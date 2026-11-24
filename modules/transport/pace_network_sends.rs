//! One bounded outgoing queue for all Steam channels. A queue is kept per
//! peer and channel so a large transfer preserves its own ordering without
//! blocking unrelated channels to the same peer.

use crate::MAX_LOGICAL_MESSAGE_BYTES;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const MAX_WAITING_BYTES_PER_CHANNEL: usize = 2 * MAX_LOGICAL_MESSAGE_BYTES;
const MAX_WAITING_BYTES_TOTAL: usize = 64 * 1024 * 1024;
const OUTBOUND_MESSAGE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) enum PieceSent {
    Sent,
    Full,
    Failed(String),
}

struct PendingMessage {
    pieces: VecDeque<Vec<u8>>,
    reliable: bool,
    queued_at: Instant,
}

#[derive(Default)]
pub(crate) struct TransportOutbox {
    waiting: HashMap<(u64, i32), VecDeque<PendingMessage>>,
}

impl TransportOutbox {
    /// Adds one logical message and sends what Steam accepts immediately.
    /// A message stays intact in the queue so timeout cleanup never sends a
    /// later fragment after its earlier fragments were discarded.
    pub(crate) fn send(
        &mut self,
        peer: u64,
        channel: i32,
        reliable: bool,
        pieces: Vec<Vec<u8>>,
        now: Instant,
        mut send: impl FnMut(u64, i32, bool, &[u8]) -> PieceSent,
    ) -> Result<(), String> {
        self.expire(now);
        let key = (peer, channel);
        let current = self.waiting_bytes_for(key);
        let added = pieces.iter().map(Vec::len).sum::<usize>();
        if current.saturating_add(added) > MAX_WAITING_BYTES_PER_CHANNEL
            || self.waiting_bytes_total().saturating_add(added) > MAX_WAITING_BYTES_TOTAL
        {
            return Err("network send queue is full".into());
        }
        self.waiting
            .entry(key)
            .or_default()
            .push_back(PendingMessage {
                pieces: pieces.into(),
                reliable,
                queued_at: now,
            });
        self.flush_key(key, now, &mut send)
    }

    /// Flushes every channel queue. The caller rechecks peer authorization in
    /// `send` immediately before the native Steam call.
    pub(crate) fn flush(
        &mut self,
        now: Instant,
        mut send: impl FnMut(u64, i32, bool, &[u8]) -> PieceSent,
    ) -> Vec<(u64, i32, String)> {
        let expired = self.expire(now);
        let mut failures = expired;
        let keys = self.waiting.keys().copied().collect::<Vec<_>>();
        for key in keys {
            if let Err(error) = self.flush_key(key, now, &mut send) {
                failures.push((key.0, key.1, error));
            }
        }
        failures
    }

    /// Drops one peer/channel queue, for example when the remote UI
    /// subscription moves to a different server.
    pub(crate) fn discard(&mut self, peer: u64, channel: i32) {
        self.waiting.remove(&(peer, channel));
    }

    pub(crate) fn has_pending(&self, peer: u64, channel: i32) -> bool {
        self.waiting.contains_key(&(peer, channel))
    }

    pub(crate) fn clear(&mut self) {
        self.waiting.clear();
    }

    fn expire(&mut self, now: Instant) -> Vec<(u64, i32, String)> {
        let mut failures = Vec::new();
        self.waiting.retain(|(peer, channel), messages| {
            messages.retain(|message| {
                let keep =
                    now.saturating_duration_since(message.queued_at) < OUTBOUND_MESSAGE_TIMEOUT;
                if !keep {
                    failures.push((*peer, *channel, "queued network message expired".into()));
                }
                keep
            });
            !messages.is_empty()
        });
        failures
    }

    fn flush_key(
        &mut self,
        key: (u64, i32),
        now: Instant,
        mut send: impl FnMut(u64, i32, bool, &[u8]) -> PieceSent,
    ) -> Result<(), String> {
        loop {
            let Some((piece, reliable, queued_at)) = self
                .waiting
                .get(&key)
                .and_then(|messages| messages.front())
                .map(|message| {
                    (
                        message.pieces.front().cloned(),
                        message.reliable,
                        message.queued_at,
                    )
                })
            else {
                self.waiting.remove(&key);
                return Ok(());
            };
            if now.saturating_duration_since(queued_at) >= OUTBOUND_MESSAGE_TIMEOUT {
                self.waiting.remove(&key);
                return Err("queued network message expired".into());
            }
            let Some(piece) = piece else {
                if let Some(messages) = self.waiting.get_mut(&key) {
                    messages.pop_front();
                }
                continue;
            };
            match send(key.0, key.1, reliable, &piece) {
                PieceSent::Sent => {
                    let message_finished = if let Some(message) =
                        self.waiting.get_mut(&key).and_then(VecDeque::front_mut)
                    {
                        message.pieces.pop_front();
                        message.pieces.is_empty()
                    } else {
                        false
                    };
                    if message_finished {
                        if let Some(messages) = self.waiting.get_mut(&key) {
                            messages.pop_front();
                        }
                    }
                }
                PieceSent::Full => return Ok(()),
                PieceSent::Failed(error) => {
                    self.waiting.remove(&key);
                    return Err(error);
                }
            }
        }
    }

    fn waiting_bytes_for(&self, key: (u64, i32)) -> usize {
        self.waiting
            .get(&key)
            .into_iter()
            .flatten()
            .flat_map(|message| message.pieces.iter())
            .map(Vec::len)
            .sum()
    }

    fn waiting_bytes_total(&self) -> usize {
        self.waiting
            .values()
            .flatten()
            .flat_map(|message| message.pieces.iter())
            .map(Vec::len)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_buffer_waits_in_order_per_channel() {
        let mut outbox = TransportOutbox::default();
        let mut sent = Vec::new();
        let now = Instant::now();
        let mut room = 2;
        outbox
            .send(
                1,
                42,
                true,
                vec![vec![1], vec![2], vec![3]],
                now,
                |_, _, _, piece| {
                    if room == 0 {
                        return PieceSent::Full;
                    }
                    room -= 1;
                    sent.push(piece[0]);
                    PieceSent::Sent
                },
            )
            .unwrap();
        outbox
            .send(1, 42, true, vec![vec![4]], now, |_, _, _, _| {
                PieceSent::Full
            })
            .unwrap();
        outbox
            .send(1, 43, true, vec![vec![5]], now, |_, _, _, piece| {
                sent.push(piece[0]);
                PieceSent::Sent
            })
            .unwrap();
        assert_eq!(sent, vec![1, 2, 5]);
        let failures = outbox.flush(now, |_, _, _, piece| {
            sent.push(piece[0]);
            PieceSent::Sent
        });
        assert!(failures.is_empty());
        assert_eq!(sent, vec![1, 2, 5, 3, 4]);
    }

    #[test]
    fn expired_message_is_removed_as_a_whole() {
        let mut outbox = TransportOutbox::default();
        let start = Instant::now();
        outbox
            .send(1, 42, true, vec![vec![1], vec![2]], start, |_, _, _, _| {
                PieceSent::Full
            })
            .unwrap();
        let later = start + OUTBOUND_MESSAGE_TIMEOUT;
        let failures = outbox.flush(later, |_, _, _, _| panic!("expired message was sent"));
        assert_eq!(failures.len(), 1);
        assert!(outbox.waiting.is_empty());
    }

    #[test]
    fn peer_channel_queue_can_be_discarded_without_affecting_another_channel() {
        let mut outbox = TransportOutbox::default();
        let now = Instant::now();
        outbox
            .send(1, 42, true, vec![vec![1]], now, |_, _, _, _| {
                PieceSent::Full
            })
            .unwrap();
        outbox
            .send(1, 43, true, vec![vec![2]], now, |_, _, _, _| {
                PieceSent::Full
            })
            .unwrap();
        outbox.discard(1, 42);
        assert_eq!(outbox.waiting.len(), 1);
        assert!(outbox.waiting.contains_key(&(1, 43)));
    }
}
