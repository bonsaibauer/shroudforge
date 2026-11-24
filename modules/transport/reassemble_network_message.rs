//! Puts back together fragments from the shared Steam transport. A payload
//! that is not a fragment is a whole message and passes through.

use crate::MAX_LOGICAL_MESSAGE_BYTES;
use crate::split_network_message::{
    FRAGMENT_CHECKSUM_BYTES, FRAGMENT_DATA_BYTES, FRAGMENT_HEADER_BYTES, FRAGMENT_MAGIC,
    fragment_checksum,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A message whose fragments stop coming is dropped after this long.
const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(30);
/// Messages in progress from one peer at most.
const MAX_MESSAGES_IN_PROGRESS_PER_PEER: usize = 4;
/// Declared storage held for incomplete messages from one peer at most.
const MAX_REASSEMBLY_BYTES_PER_PEER: usize = 32 * 1024 * 1024;
/// Declared storage held for all incomplete messages at most.
const MAX_REASSEMBLY_BYTES_TOTAL: usize = 64 * 1024 * 1024;

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
pub(crate) struct NetworkReassembly {
    partial: HashMap<(u64, i32, u128), Partial>,
}

impl NetworkReassembly {
    /// The message `payload` from `peer` on `channel` completes, if any.
    pub(crate) fn accept(
        &mut self,
        peer: u64,
        channel: i32,
        payload: Vec<u8>,
        now: Instant,
    ) -> Reassembled {
        self.expire(now);
        if !payload.starts_with(FRAGMENT_MAGIC)
            || payload.len() < FRAGMENT_HEADER_BYTES + FRAGMENT_CHECKSUM_BYTES
        {
            return Reassembled::Message(payload);
        }
        let data_end = payload.len() - FRAGMENT_CHECKSUM_BYTES;
        let checksum = u64::from_le_bytes(payload[data_end..].try_into().unwrap_or_default());
        if fragment_checksum(&payload[..data_end]) != checksum {
            // Ordinary binary messages may begin with the framing marker.
            // Only consume a packet as a fragment when its checksum matches.
            return Reassembled::Message(payload);
        }
        let id = u128::from_le_bytes(payload[4..20].try_into().unwrap_or_default());
        let field = |at: usize| {
            u32::from_le_bytes(payload[at..at + 4].try_into().unwrap_or_default()) as usize
        };
        let (index, count, length) = (field(20), field(24), field(28));
        if length == 0
            || length > MAX_LOGICAL_MESSAGE_BYTES
            || count != length.div_ceil(FRAGMENT_DATA_BYTES)
        {
            return Reassembled::Invalid("network fragment with an invalid length or count");
        }
        if index >= count {
            return Reassembled::Invalid("network fragment index out of range");
        }
        let expected = if index + 1 == count {
            length - index * FRAGMENT_DATA_BYTES
        } else {
            FRAGMENT_DATA_BYTES
        };
        let data = &payload[FRAGMENT_HEADER_BYTES..data_end];
        if data.len() != expected {
            return Reassembled::Invalid("network fragment of an unexpected size");
        }
        let key = (peer, channel, id);
        if !self.partial.contains_key(&key) {
            let from_peer = self
                .partial
                .iter()
                .filter(|((from, _, _), _)| *from == peer)
                .collect::<Vec<_>>();
            let in_progress = from_peer.len();
            let peer_bytes = from_peer
                .iter()
                .map(|(_, partial)| partial.length)
                .sum::<usize>();
            let total_bytes = self
                .partial
                .values()
                .map(|partial| partial.length)
                .sum::<usize>();
            if in_progress >= MAX_MESSAGES_IN_PROGRESS_PER_PEER {
                return Reassembled::Invalid("too many network messages in progress from a peer");
            }
            if peer_bytes.saturating_add(length) > MAX_REASSEMBLY_BYTES_PER_PEER
                || total_bytes.saturating_add(length) > MAX_REASSEMBLY_BYTES_TOTAL
            {
                return Reassembled::Invalid("network reassembly memory limit exceeded");
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
            return Reassembled::Invalid("network fragments disagree on their message");
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

    /// Drops incomplete messages that have not received fragments recently.
    pub(crate) fn expire(&mut self, now: Instant) {
        self.partial
            .retain(|_, partial| now.saturating_duration_since(partial.started) < FRAGMENT_TIMEOUT);
    }

    pub(crate) fn clear(&mut self) {
        self.partial.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split_network_message::split_network_message;

    fn large_payload() -> Vec<u8> {
        (0..(crate::MAX_MESSAGE_BYTES + 1000))
            .map(|index| (index % 251) as u8)
            .collect()
    }

    #[test]
    fn a_whole_message_passes_through() {
        let mut reassembly = NetworkReassembly::default();
        let message = br#"{"type":"subscribe"}"#.to_vec();
        assert_eq!(
            reassembly.accept(1, 42, message.clone(), Instant::now()),
            Reassembled::Message(message)
        );
    }

    #[test]
    fn binary_payload_with_the_transport_prefix_passes_through() {
        let mut reassembly = NetworkReassembly::default();
        let mut payload = b"SFNT".to_vec();
        payload.extend(0u8..128u8);
        assert_eq!(
            reassembly.accept(1, 42, payload.clone(), Instant::now()),
            Reassembled::Message(payload)
        );
    }

    #[test]
    fn fragments_in_any_order_give_the_message_back() {
        let payload = large_payload();
        let mut fragments = split_network_message(&payload, 42);
        fragments.reverse();
        let last = fragments.pop().unwrap();
        let mut reassembly = NetworkReassembly::default();
        let now = Instant::now();
        for fragment in fragments {
            assert_eq!(
                reassembly.accept(9, 42, fragment, now),
                Reassembled::Pending
            );
        }
        assert_eq!(
            reassembly.accept(9, 42, last, now),
            Reassembled::Message(payload)
        );
    }

    #[test]
    fn two_peers_do_not_mix_their_fragments() {
        let payload = large_payload();
        let fragments = split_network_message(&payload, 5);
        let mut reassembly = NetworkReassembly::default();
        let now = Instant::now();
        for fragment in &fragments[..fragments.len() - 1] {
            assert_eq!(
                reassembly.accept(1, 42, fragment.clone(), now),
                Reassembled::Pending
            );
            assert_eq!(
                reassembly.accept(2, 42, fragment.clone(), now),
                Reassembled::Pending
            );
        }
        let last = fragments.last().unwrap().clone();
        assert_eq!(
            reassembly.accept(1, 42, last.clone(), now),
            Reassembled::Message(payload.clone())
        );
        assert_eq!(
            reassembly.accept(2, 42, last, now),
            Reassembled::Message(payload)
        );
    }

    #[test]
    fn channels_do_not_mix_fragments_from_the_same_peer() {
        let payload = large_payload();
        let fragments = split_network_message(&payload, 12);
        let mut reassembly = NetworkReassembly::default();
        let now = Instant::now();
        for fragment in &fragments[..fragments.len() - 1] {
            assert_eq!(
                reassembly.accept(1, 42, fragment.clone(), now),
                Reassembled::Pending
            );
            assert_eq!(
                reassembly.accept(1, 43, fragment.clone(), now),
                Reassembled::Pending
            );
        }
        let last = fragments.last().unwrap().clone();
        assert_eq!(
            reassembly.accept(1, 42, last.clone(), now),
            Reassembled::Message(payload.clone())
        );
        assert_eq!(
            reassembly.accept(1, 43, last, now),
            Reassembled::Message(payload)
        );
    }

    #[test]
    fn an_unfinished_message_expires() {
        let payload = large_payload();
        let fragments = split_network_message(&payload, 3);
        let mut reassembly = NetworkReassembly::default();
        let start = Instant::now();
        assert_eq!(
            reassembly.accept(1, 42, fragments[0].clone(), start),
            Reassembled::Pending
        );
        let later = start + FRAGMENT_TIMEOUT + Duration::from_secs(1);
        for fragment in &fragments[1..fragments.len() - 1] {
            reassembly.accept(1, 42, fragment.clone(), later);
        }
        // The first fragment was dropped with its expired message: no message.
        assert_eq!(
            reassembly.accept(1, 42, fragments.last().unwrap().clone(), later),
            Reassembled::Pending
        );
    }

    #[test]
    fn a_forged_fragment_is_refused() {
        let payload = large_payload();
        let mut fragment = split_network_message(&payload, 1).remove(0);
        fragment[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
        let data_end = fragment.len() - FRAGMENT_CHECKSUM_BYTES;
        let checksum = fragment_checksum(&fragment[..data_end]);
        fragment[data_end..].copy_from_slice(&checksum.to_le_bytes());
        let mut reassembly = NetworkReassembly::default();
        assert!(matches!(
            reassembly.accept(1, 42, fragment, Instant::now()),
            Reassembled::Invalid(_)
        ));
    }
}
