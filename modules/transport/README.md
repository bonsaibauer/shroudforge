# Built-in Steam Networking module

This module supplies the Windows runtime binding for Steam Networking Messages. The game initializes Steam. The module only uses the client or dedicated-server interface already loaded in that process. It ships with ShroudForge and is not a `mod.json` package. Channel 65534 is reserved internally for Modloader file mirroring; Lua mods can use channels 0 through 65533. Channel 65535 remains reserved for Network health checks.

The Modloader mirrors server settings over that same channel. Server logs are sent only after the server validates the password of a `userGroups` group with all five admin permission flags, and only while that group's server log-sync setting is enabled. The setting lives in the existing Modloader server settings. A settings change uses the current mirror as its expected version. The server checks it under the existing installation write lock before saving, then returns the canonical settings. The password is sent only to the server, never returned to the client UI, and never persisted by ShroudForge.

The Servermanagement rights view is assembled by the server. It lists authenticated Steam peers from Enshrouded's live peer list, then joins those IDs with names and effective permissions read from the server's own `logs/enshrouded_server.log`. Group permissions are read from `enshrouded_server.json`. A name reported by a client is not used to identify a player or determine permissions. The server never sends group passwords to the UI.

Runtime mods use `runtime.network`:

```lua
local status = runtime.network.status()
if status.available then
    print("This process Steam ID:", status.local_steam_id)
end

-- Send to the other process's decimal SteamID64.
local ok, reason = runtime.network.send(server_steam_id, "hello", {
    channel = 42,
    reliable = true,
})

-- Read from the same local channel during on_update.
local messages, receive_error = runtime.network.receive({ channel = 42, limit = 16 })
```

For mod-to-mod traffic, use the routed API. The caller's mod ID is attached automatically, and the receiving mod ID selects a dedicated deterministic Steam channel. The SteamID64 is always explicit, so a server mod can address a particular client rather than broadcasting to an ambiguous player name:

```lua
-- server-side mod: configure/store this client's SteamID64 after authorization
runtime.network.accept(client_steam_id)
local ok, reason = runtime.network.send_mod(client_steam_id, "client-addon", "hello")

-- client-addon, from its runtime on_update:
local messages, reason = runtime.network.receive_mod(16)
for _, message in ipairs(messages or {}) do
    print(message.peer_steam_id, message.from_mod, message.payload)
end
```

Both peers must have ShroudForge and Steam Networking initialized. This is peer-to-peer: it does not relay through the game server, map Steam IDs to character names, or prove that a claimed sender mod ID is trustworthy. Validate allowed peer IDs and payloads in the receiving mod. Use application-level acknowledgements when delivery confirmation is needed.

The calling mod must have the `runtime` capability. `runtime.network.send` accepts a binary Lua string up to 512 KiB. Reliable messages are ordered per peer and channel. A successful `send` means Steam accepted the send request. Add an application-level reply if the sender needs a delivery acknowledgement. Use `runtime.network.accept(peer_steam_id)` on the receiving process before its first `receive` call for an incoming peer session. `local_steam_id` is available from `runtime.network.status()` on each process and is returned as a decimal string so Lua does not lose uint64 precision.

The client and server must both run ShroudForge. This is a separate Steam P2P session and does not inject data into Enshrouded's native packets or forward messages through its server. Mods must arrange peer Steam IDs and authorization through their own configuration or application protocol. Two values come from the game's own session log (`enshrouded.log`, see `read_session_peers.rs`): a client that joined a game reads its host in `runtime.network.status().session_host_steam_id`, a player's game or a Dedicated Server, and a player's game that hosts lists the players of its session with `runtime.network.connected_peers()`. In the game that hosts, a Dedicated Server or a player's game, `runtime.network.player_permissions(peer)` gives the permissions each player logged in with, from the same log.
