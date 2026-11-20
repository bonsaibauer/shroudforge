//! Sends Modloader remote UI messages to each peer in order, a fragmented one
//! over several updates. Steam refuses a send while a peer's send buffer
//! (512 KiB by default) is full: the rest waits for the next update
//! (`flush`), and a message for a peer with messages waiting queues behind
//! them.

use crate::MAX_REMOTE_UI_MESSAGE_BYTES;
use std::collections::{HashMap, VecDeque};

/// What Steam did with one message.
pub(crate) enum PieceSent {
    Sent,
    /// The peer's send buffer is full: the message waits.
    Full,
    Failed(String),
}

/// Bytes waiting for one peer at most; beyond, its queue is dropped.
const MAX_WAITING_BYTES_PER_PEER: usize = 2 * MAX_REMOTE_UI_MESSAGE_BYTES;

#[derive(Default)]
pub(crate) struct RemoteUiOutbox {
    waiting: HashMap<u64, VecDeque<Vec<u8>>>,
}

impl RemoteUiOutbox {
    /// Queues `pieces` (a message, or its fragments) for `peer` and sends
    /// what Steam takes now. An error drops what was waiting for that peer.
    pub(crate) fn send(
        &mut self,
        peer: u64,
        pieces: Vec<Vec<u8>>,
        send: impl FnMut(u64, &[u8]) -> PieceSent,
    ) -> Result<(), String> {
        let queue = self.waiting.entry(peer).or_default();
        let waiting = queue.iter().chain(pieces.iter()).map(Vec::len).sum::<usize>();
        if waiting > MAX_WAITING_BYTES_PER_PEER {
            self.waiting.remove(&peer);
            return Err("too many remote UI messages waiting for this peer".into());
        }
        queue.extend(pieces);
        self.flush_peer(peer, send)
    }

    /// Sends what waits, peer by peer; the peers whose send failed, with the
    /// error (what waited for them dropped).
    pub(crate) fn flush(
        &mut self,
        mut send: impl FnMut(u64, &[u8]) -> PieceSent,
    ) -> Vec<(u64, String)> {
        let peers = self.waiting.keys().copied().collect::<Vec<_>>();
        peers
            .into_iter()
            .filter_map(|peer| {
                self.flush_peer(peer, &mut send)
                    .err()
                    .map(|error| (peer, error))
            })
            .collect()
    }

    fn flush_peer(
        &mut self,
        peer: u64,
        mut send: impl FnMut(u64, &[u8]) -> PieceSent,
    ) -> Result<(), String> {
        let Some(queue) = self.waiting.get_mut(&peer) else {
            return Ok(());
        };
        while let Some(piece) = queue.front() {
            match send(peer, piece) {
                PieceSent::Sent => {
                    queue.pop_front();
                }
                PieceSent::Full => return Ok(()),
                PieceSent::Failed(error) => {
                    self.waiting.remove(&peer);
                    return Err(error);
                }
            }
        }
        self.waiting.remove(&peer);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_steam_refuses_waits_in_order_for_the_next_flush() {
        let mut outbox = RemoteUiOutbox::default();
        let mut sent = Vec::new();
        let mut room = 2;
        outbox
            .send(1, vec![vec![1], vec![2], vec![3]], |_, piece| {
                if room == 0 {
                    return PieceSent::Full;
                }
                room -= 1;
                sent.push(piece[0]);
                PieceSent::Sent
            })
            .unwrap();
        // A later message queues behind the waiting fragment.
        outbox.send(1, vec![vec![4]], |_, _| PieceSent::Full).unwrap();
        assert_eq!(sent, vec![1, 2]);
        let failures = outbox.flush(|_, piece| {
            sent.push(piece[0]);
            PieceSent::Sent
        });
        assert!(failures.is_empty());
        assert_eq!(sent, vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_failed_send_drops_what_waited_for_that_peer_only() {
        let mut outbox = RemoteUiOutbox::default();
        outbox.send(1, vec![vec![1]], |_, _| PieceSent::Full).unwrap();
        outbox.send(2, vec![vec![2]], |_, _| PieceSent::Full).unwrap();
        let failures = outbox.flush(|peer, _| {
            if peer == 1 {
                PieceSent::Failed("gone".into())
            } else {
                PieceSent::Sent
            }
        });
        assert_eq!(failures, vec![(1, "gone".to_owned())]);
        assert!(outbox.waiting.is_empty());
    }

    #[test]
    fn too_much_waiting_for_one_peer_is_refused() {
        let mut outbox = RemoteUiOutbox::default();
        let large = vec![0u8; MAX_REMOTE_UI_MESSAGE_BYTES];
        outbox.send(1, vec![large.clone(), large.clone()], |_, _| PieceSent::Full).unwrap();
        assert!(outbox.send(1, vec![vec![0]], |_, _| PieceSent::Full).is_err());
    }
}
