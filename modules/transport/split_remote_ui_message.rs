//! Splits a Modloader remote UI message larger than one Steam message into
//! fragments. `reassemble_remote_ui_message.rs` puts them back together on
//! arrival.

use crate::{MAX_MESSAGE_BYTES, MAX_REMOTE_UI_MESSAGE_BYTES};

/// The first bytes of a fragment. A remote UI message is JSON and never
/// starts with them.
pub(crate) const FRAGMENT_MAGIC: &[u8; 4] = b"SFRF";
/// Magic, the message's id, the fragment's index, the fragment count and the
/// whole message's length.
pub(crate) const FRAGMENT_HEADER_BYTES: usize = 4 + 16 + 4 + 4 + 4;
/// The data a fragment carries: a quarter of a Steam message, so that Steam's
/// default send buffer of 512 KiB holds several while they are sent.
pub(crate) const FRAGMENT_DATA_BYTES: usize = MAX_MESSAGE_BYTES / 4 - FRAGMENT_HEADER_BYTES;

/// The fragments of `payload`, in order. `payload` is at most
/// `MAX_REMOTE_UI_MESSAGE_BYTES`; `id` names the message among those in
/// progress between two peers.
pub(crate) fn split_remote_ui_message(payload: &[u8], id: u128) -> Vec<Vec<u8>> {
    debug_assert!(payload.len() <= MAX_REMOTE_UI_MESSAGE_BYTES);
    let count = payload.len().div_ceil(FRAGMENT_DATA_BYTES) as u32;
    payload
        .chunks(FRAGMENT_DATA_BYTES)
        .enumerate()
        .map(|(index, data)| {
            let mut fragment = Vec::with_capacity(FRAGMENT_HEADER_BYTES + data.len());
            fragment.extend_from_slice(FRAGMENT_MAGIC);
            fragment.extend_from_slice(&id.to_le_bytes());
            fragment.extend_from_slice(&(index as u32).to_le_bytes());
            fragment.extend_from_slice(&count.to_le_bytes());
            fragment.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            fragment.extend_from_slice(data);
            fragment
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_fit_one_steam_message_and_carry_the_whole_payload() {
        let payload: Vec<u8> = (0..(MAX_MESSAGE_BYTES * 2 + 17)).map(|index| index as u8).collect();
        let fragments = split_remote_ui_message(&payload, 7);
        assert_eq!(fragments.len(), payload.len().div_ceil(FRAGMENT_DATA_BYTES));
        assert!(fragments.iter().all(|fragment| fragment.len() <= MAX_MESSAGE_BYTES));
        assert!(fragments.iter().all(|fragment| fragment.starts_with(FRAGMENT_MAGIC)));
        let data: Vec<u8> = fragments
            .iter()
            .flat_map(|fragment| fragment[FRAGMENT_HEADER_BYTES..].iter().copied())
            .collect();
        assert_eq!(data, payload);
    }
}
