//! Steam Networking Messages P2P channel exposed to active runtime mods.

use super::{AppState, loader};
use mlua::{Lua, Table};
use mod_loader::Mod;
use shroudforge_steam_networking as steam_network;

fn steam_id(value: &str) -> Result<u64, String> {
    let id = value
        .parse::<u64>()
        .map_err(|_| "peer Steam ID must be a decimal uint64 string".to_owned())?;
    if id == 0 {
        return Err("peer Steam ID must be nonzero".into());
    }
    Ok(id)
}

fn channel(options: &Option<Table>) -> mlua::Result<i32> {
    let channel = match options {
        Some(options) => options.raw_get::<Option<i32>>("channel")?.unwrap_or(0),
        None => 0,
    };
    if !(0..=65_535).contains(&channel) {
        return Err(mlua::Error::runtime("channel must be between 0 and 65535"));
    }
    Ok(channel)
}

fn denied(lua: &Lua, owner: &Mod, operation: &str) -> Option<String> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    loader::runtime_denial_reason(&state, owner, operation)
}

// Give every destination mod a deterministic Steam channel. The envelope still
// carries the full mod ID so a hash collision cannot deliver a message to the
// wrong mod.
fn mod_channel(id: &str) -> i32 {
    let hash = id.bytes().fold(0x811c9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    });
    ((hash % 65_535) + 1) as i32
}

pub(crate) fn create(lua: &Lua, owner: &Mod) -> mlua::Result<Table> {
    let network = lua.create_table()?;

    let status_owner = owner.clone();
    network.set(
        "status",
        lua.create_function(move |lua, ()| {
            let reason = denied(lua, &status_owner, "runtime.network.status");
            let native = if reason.is_none() {
                steam_network::status()
            } else {
                steam_network::Status {
                    available: false,
                    local_steam_id: None,
                    local_dedicated_server_steam_id: None,
                }
            };
            let state = lua.app_data_ref::<AppState>().unwrap();
            let result = lua.create_table()?;
            let available = reason.is_none() && native.available;
            result.raw_set("available", available)?;
            result.raw_set(
                "role",
                if state.is_server() {
                    "server"
                } else {
                    "client"
                },
            )?;
            result.raw_set("local_steam_id", native.local_steam_id)?;
            result.raw_set(
                "local_dedicated_server_steam_id",
                native.local_dedicated_server_steam_id,
            )?;
            result.raw_set(
                "reason",
                reason.or_else(|| {
                    (!native.available).then(|| {
                        "Steam Networking Messages is not initialized in this process".to_owned()
                    })
                }),
            )?;
            Ok(result)
        })?,
    )?;

    let send_owner = owner.clone();
    network.set(
        "send",
        lua.create_function(
            move |lua, (peer, payload, options): (String, mlua::String, Option<Table>)| {
                if let Some(reason) = denied(lua, &send_owner, "runtime.network.send") {
                    return Ok((false, Some(reason)));
                }
                let peer = match steam_id(&peer) {
                    Ok(peer) => peer,
                    Err(reason) => return Ok((false, Some(reason))),
                };
                let channel = channel(&options)?;
                let reliable = match options {
                    Some(options) => options.raw_get::<Option<bool>>("reliable")?.unwrap_or(true),
                    None => true,
                };
                let payload = payload.as_bytes().to_vec();
                Ok(
                    match steam_network::send(peer, &payload, channel, reliable) {
                        Ok(()) => (true, None),
                        Err(reason) => (false, Some(reason)),
                    },
                )
            },
        )?,
    )?;

    let send_mod_owner = owner.clone();
    network.set(
        "send_mod",
        lua.create_function(move |lua, (peer, target_mod, payload, options): (String, String, String, Option<Table>)| {
            if let Some(reason) = denied(lua, &send_mod_owner, "runtime.network.send") {
                return Ok((false, Some(reason)));
            }
            let peer = match steam_id(&peer) {
                Ok(peer) => peer,
                Err(reason) => return Ok((false, Some(reason))),
            };
            if target_mod.is_empty() || target_mod.len() > 96 {
                return Ok((false, Some("target mod ID must contain 1 to 96 bytes".into())));
            }
            let reliable = match options {
                Some(options) => options.raw_get::<Option<bool>>("reliable")?.unwrap_or(true),
                None => true,
            };
            let channel = mod_channel(&target_mod);
            let envelope = serde_json::json!({
                "protocol": "shroudforge.mod-message.v1",
                "from_mod": send_mod_owner.info().id,
                "to_mod": target_mod,
                "payload": payload,
            });
            let bytes = serde_json::to_vec(&envelope).map_err(mlua::Error::external)?;
            Ok(match steam_network::send(peer, &bytes, channel, reliable) {
                Ok(()) => (true, None),
                Err(reason) => (false, Some(reason)),
            })
        })?,
    )?;

    let accept_owner = owner.clone();
    network.set(
        "accept",
        lua.create_function(move |lua, peer: String| {
            if let Some(reason) = denied(lua, &accept_owner, "runtime.network.accept") {
                return Ok((false, Some(reason)));
            }
            let peer = match steam_id(&peer) {
                Ok(peer) => peer,
                Err(reason) => return Ok((false, Some(reason))),
            };
            Ok(match steam_network::accept(peer) {
                Ok(true) => (true, None),
                Ok(false) => (
                    false,
                    Some("no pending Steam P2P session for this peer".into()),
                ),
                Err(reason) => (false, Some(reason)),
            })
        })?,
    )?;

    let peers_owner = owner.clone();
    network.set(
        "connected_peers",
        lua.create_function(move |lua, ()| {
            if let Some(reason) = denied(lua, &peers_owner, "runtime.network.connected_peers") {
                return Ok((None::<Table>, Some(reason)));
            }
            let state = lua.app_data_ref::<AppState>().unwrap();
            if !state.is_server() {
                return Ok((
                    None,
                    Some(
                        "connected_peers is available only in the Dedicated Server runtime".into(),
                    ),
                ));
            }
            let peers = match steam_network::connected_peers() {
                Ok(peers) => peers,
                Err(reason) => return Ok((None, Some(reason))),
            };
            let result = lua.create_table()?;
            for (index, peer) in peers.iter().enumerate() {
                result.raw_set(index + 1, peer.as_str())?;
            }
            Ok((Some(result), None))
        })?,
    )?;

    let receive_owner = owner.clone();
    network.set(
        "receive",
        lua.create_function(move |lua, options: Option<Table>| {
            if let Some(reason) = denied(lua, &receive_owner, "runtime.network.receive") {
                return Ok((None, Some(reason)));
            }
            let channel = channel(&options)?;
            let limit = match &options {
                Some(options) => options.raw_get::<Option<usize>>("limit")?.unwrap_or(16),
                None => 16,
            };
            if !(1..=32).contains(&limit) {
                return Err(mlua::Error::runtime(
                    "receive limit must be between 1 and 32",
                ));
            }
            let messages = lua.create_table()?;
            for index in 1..=limit {
                match steam_network::receive(channel) {
                    Ok(Some(message)) => {
                        let value = lua.create_table()?;
                        value.raw_set("peer_steam_id", message.peer_steam_id)?;
                        value.raw_set("payload", lua.create_string(&message.payload)?)?;
                        value.raw_set("reliable", message.reliable)?;
                        messages.raw_set(index, value)?;
                    }
                    Ok(None) => break,
                    Err(reason) => return Ok((Some(messages), Some(reason))),
                }
            }
            Ok((Some(messages), None))
        })?,
    )?;

    let receive_mod_owner = owner.clone();
    network.set(
        "receive_mod",
        lua.create_function(move |lua, limit: Option<usize>| {
            if let Some(reason) = denied(lua, &receive_mod_owner, "runtime.network.receive") {
                return Ok((None, Some(reason)));
            }
            let limit = limit.unwrap_or(16);
            if !(1..=32).contains(&limit) {
                return Err(mlua::Error::runtime(
                    "receive limit must be between 1 and 32",
                ));
            }
            let messages = lua.create_table()?;
            for index in 1..=limit {
                match steam_network::receive(mod_channel(&receive_mod_owner.info().id)) {
                    Ok(Some(message)) => {
                        let Ok(envelope) =
                            serde_json::from_slice::<serde_json::Value>(&message.payload)
                        else {
                            continue;
                        };
                        if envelope["protocol"] != "shroudforge.mod-message.v1"
                            || envelope["to_mod"].as_str() != Some(&receive_mod_owner.info().id)
                        {
                            continue;
                        }
                        let Some(from_mod) = envelope["from_mod"].as_str() else {
                            continue;
                        };
                        let Some(payload) = envelope["payload"].as_str() else {
                            continue;
                        };
                        let value = lua.create_table()?;
                        value.raw_set("peer_steam_id", message.peer_steam_id)?;
                        value.raw_set("from_mod", from_mod)?;
                        value.raw_set("to_mod", receive_mod_owner.info().id.as_str())?;
                        value.raw_set("payload", payload)?;
                        value.raw_set("reliable", message.reliable)?;
                        messages.raw_set(index, value)?;
                    }
                    Ok(None) => break,
                    Err(reason) => return Ok((Some(messages), Some(reason))),
                }
            }
            Ok((Some(messages), None))
        })?,
    )?;

    Ok(network)
}
