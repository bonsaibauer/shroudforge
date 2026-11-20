//! Puts back together the fragments of a Modloader remote UI message split by
//! `split_remote_ui_message.rs`, peer by peer. A payload that is not a
//! fragment is a whole message and passes through.

use crate::MAX_REMOTE_UI_MESSAGE_BYTES;
use crate::split_remote_ui_message::{FRAGMENT_DATA_BYTES, FRAGMENT_HEADER_BYTES, FRAGMENT_MAGIC};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A message whose fragments stop coming is dropped after this long.
const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(30);
/// Messages in progress from one peer at most.
const MAX_MESSAGES_IN_PROGRESS_PER_PEER: usize = 4;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Reassembled {
    /// A whole message.
    Message(Vec<u8>),
    /// A fragment kept until the others arrive.
    Pending,
    /// A fragment that cannot be part of a message, dropped.
    Invalid(&'static str),
}

struct Partial {
    length: usize,
    parts: Vec<Option<Vec<u8>>>,
    received: usize,
    started: Instant,
}

#[derive(Default)]
pub(crate) struct RemoteUiReassembly {
    partial: HashMap<(u64, u128), Partial>,
}

impl RemoteUiReassembly {
    /// The message `payload` from `peer` completes, if any.
    pub(crate) fn accept(&mut self, peer: u64, payload: Vec<u8>, now: Instant) -> Reassembled {
        if !payload.starts_with(FRAGMENT_MAGIC) {
            return Reassembled::Message(payload);
        }
        self.partial
            .retain(|_, partial| now.saturating_duration_since(partial.started) < FRAGMENT_TIMEOUT);
        if payload.len() < FRAGMENT_HEADER_BYTES {
            return Reassembled::Invalid("remote UI fragment shorter than its header");
        }
        let id = u128::from_le_bytes(payload[4..20].try_into().unwrap_or_default());
        let field = |at: usize| {
            u32::from_le_bytes(payload[at..at + 4].try_into().unwrap_or_default()) as usize
        };
        let (index, count, length) = (field(20), field(24), field(28));
        if length == 0
            || length > MAX_REMOTE_UI_MESSAGE_BYTES
            || count != length.div_ceil(FRAGMENT_DATA_BYTES)
        {
            return Reassembled::Invalid("remote UI fragment with an invalid length or count");
        }
        if index >= count {
            return Reassembled::Invalid("remote UI fragment index out of range");
        }
        let expected = if index + 1 == count {
            length - index * FRAGMENT_DATA_BYTES
        } else {
            FRAGMENT_DATA_BYTES
        };
        let data = &payload[FRAGMENT_HEADER_BYTES..];
        if data.len() != expected {
            return Reassembled::Invalid("remote UI fragment of an unexpected size");
        }
        let key = (peer, id);
        if !self.partial.contains_key(&key) {
            let in_progress = self.partial.keys().filter(|(from, _)| *from == peer).count();
            if in_progress >= MAX_MESSAGES_IN_PROGRESS_PER_PEER {
                return Reassembled::Invalid("too many remote UI messages in progress from a peer");
            }
            self.partial.insert(
                key,
                Partial {
                    length,
                    parts: vec![None; count],
                    received: 0,
                    started: now,
                },
            );
        }
        let Some(partial) = self.partial.get_mut(&key) else {
            return Reassembled::Pending;
        };
        if partial.length != length || partial.parts.len() != count {
            self.partial.remove(&key);
            return Reassembled::Invalid("remote UI fragments disagree on their message");
        }
        if partial.parts[index].is_none() {
            partial.parts[index] = Some(data.to_vec());
            partial.received += 1;
        }
        if partial.received < count {
            return Reassembled::Pending;
        }
        let Some(partial) = self.partial.remove(&key) else {
            return Reassembled::Pending;
        };
        let mut message = Vec::with_capacity(partial.length);
        for part in partial.parts.into_iter().flatten() {
            message.extend_from_slice(&part);
        }
        Reassembled::Message(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split_remote_ui_message::split_remote_ui_message;

    fn large_payload() -> Vec<u8> {
        (0..(crate::MAX_MESSAGE_BYTES + 1000)).map(|index| (index % 251) as u8).collect()
    }

    #[test]
    fn a_whole_message_passes_through() {
        let mut reassembly = RemoteUiReassembly::default();
        let message = br#"{"type":"subscribe"}"#.to_vec();
        assert_eq!(
            reassembly.accept(1, message.clone(), Instant::now()),
            Reassembled::Message(message)
        );
    }

    #[test]
    fn fragments_in_any_order_give_the_message_back() {
        let payload = large_payload();
        let mut fragments = split_remote_ui_message(&payload, 42);
        fragments.reverse();
        let last = fragments.pop().unwrap();
        let mut reassembly = RemoteUiReassembly::default();
        let now = Instant::now();
        for fragment in fragments {
            assert_eq!(reassembly.accept(9, fragment, now), Reassembled::Pending);
        }
        assert_eq!(reassembly.accept(9, last, now), Reassembled::Message(payload));
    }

    #[test]
    fn two_peers_do_not_mix_their_fragments() {
        let payload = large_payload();
        let fragments = split_remote_ui_message(&payload, 5);
        let mut reassembly = RemoteUiReassembly::default();
        let now = Instant::now();
        for fragment in &fragments[..fragments.len() - 1] {
            assert_eq!(reassembly.accept(1, fragment.clone(), now), Reassembled::Pending);
            assert_eq!(reassembly.accept(2, fragment.clone(), now), Reassembled::Pending);
        }
        let last = fragments.last().unwrap().clone();
        assert_eq!(reassembly.accept(1, last.clone(), now), Reassembled::Message(payload.clone()));
        assert_eq!(reassembly.accept(2, last, now), Reassembled::Message(payload));
    }

    #[test]
    fn an_unfinished_message_expires() {
        let payload = large_payload();
        let fragments = split_remote_ui_message(&payload, 3);
        let mut reassembly = RemoteUiReassembly::default();
        let start = Instant::now();
        assert_eq!(reassembly.accept(1, fragments[0].clone(), start), Reassembled::Pending);
        let later = start + FRAGMENT_TIMEOUT + Duration::from_secs(1);
        for fragment in &fragments[1..fragments.len() - 1] {
            reassembly.accept(1, fragment.clone(), later);
        }
        // The first fragment was dropped with its expired message: no message.
        assert_eq!(
            reassembly.accept(1, fragments.last().unwrap().clone(), later),
            Reassembled::Pending
        );
    }

    #[test]
    fn a_forged_fragment_is_refused() {
        let payload = large_payload();
        let mut fragment = split_remote_ui_message(&payload, 1).remove(0);
        fragment[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut reassembly = RemoteUiReassembly::default();
        assert!(matches!(
            reassembly.accept(1, fragment, Instant::now()),
            Reassembled::Invalid(_)
        ));
    }
}
