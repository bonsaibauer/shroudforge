//! Splits one logical network message into Steam-sized fragments.
//! `reassemble_network_message.rs` puts them back together on arrival.

use crate::{MAX_LOGICAL_MESSAGE_BYTES, MAX_MESSAGE_BYTES};

/// The first bytes of a transport fragment. These bytes are reserved by the
/// transport framing format.
pub(crate) const FRAGMENT_MAGIC: &[u8; 4] = b"SFNT";
/// Magic, the message's id, the fragment's index, the fragment count and the
/// whole message's length. A checksum follows each fragment's data.
pub(crate) const FRAGMENT_HEADER_BYTES: usize = 4 + 16 + 4 + 4 + 4;
pub(crate) const FRAGMENT_CHECKSUM_BYTES: usize = 8;
/// The data a fragment carries: a quarter of a Steam message, so that Steam's
/// default send buffer of 512 KiB holds several while they are sent.
pub(crate) const FRAGMENT_DATA_BYTES: usize =
    MAX_MESSAGE_BYTES / 4 - FRAGMENT_HEADER_BYTES - FRAGMENT_CHECKSUM_BYTES;

/// The fragments of `payload`, in order. `payload` is at most
/// `MAX_LOGICAL_MESSAGE_BYTES`; `id` names the message among those in
/// progress between a peer and channel.
pub(crate) fn split_network_message(payload: &[u8], id: u128) -> Vec<Vec<u8>> {
    debug_assert!(payload.len() <= MAX_LOGICAL_MESSAGE_BYTES);
    let count = payload.len().div_ceil(FRAGMENT_DATA_BYTES) as u32;
    payload
        .chunks(FRAGMENT_DATA_BYTES)
        .enumerate()
        .map(|(index, data)| {
            let mut fragment =
                Vec::with_capacity(FRAGMENT_HEADER_BYTES + data.len() + FRAGMENT_CHECKSUM_BYTES);
            fragment.extend_from_slice(FRAGMENT_MAGIC);
            fragment.extend_from_slice(&id.to_le_bytes());
            fragment.extend_from_slice(&(index as u32).to_le_bytes());
            fragment.extend_from_slice(&count.to_le_bytes());
            fragment.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            fragment.extend_from_slice(data);
            let checksum = fragment_checksum(&fragment);
            fragment.extend_from_slice(&checksum.to_le_bytes());
            fragment
        })
        .collect()
}

pub(crate) fn fragment_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_fit_one_steam_message_and_carry_the_whole_payload() {
        let payload: Vec<u8> = (0..(MAX_MESSAGE_BYTES * 2 + 17))
            .map(|index| index as u8)
            .collect();
        let fragments = split_network_message(&payload, 7);
        assert_eq!(fragments.len(), payload.len().div_ceil(FRAGMENT_DATA_BYTES));
        assert!(
            fragments
                .iter()
                .all(|fragment| fragment.len() <= MAX_MESSAGE_BYTES)
        );
        assert!(
            fragments
                .iter()
                .all(|fragment| fragment.starts_with(FRAGMENT_MAGIC))
        );
        let data: Vec<u8> = fragments
            .iter()
            .flat_map(|fragment| {
                fragment[FRAGMENT_HEADER_BYTES..fragment.len() - FRAGMENT_CHECKSUM_BYTES]
                    .iter()
                    .copied()
            })
            .collect();
        assert_eq!(data, payload);
    }
}
