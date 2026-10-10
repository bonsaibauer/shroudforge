# Built-in Steam Networking module

This crate supplies the Windows runtime binding for Steam Networking Messages. The game initializes Steam. this module only uses the client or dedicated-server interface already loaded in that process. It ships with ShroudForge and is not a `mod.json` package.

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

The calling mod must have the `runtime` capability. `runtime.network.send` accepts a binary Lua string up to 512 KiB. Reliable messages are ordered per peer and channel. A successful `send` means Steam accepted the send request. add an application-level reply if the sender needs a delivery acknowledgement. Use `runtime.network.accept(peer_steam_id)` on the receiving process before its first `receive` call for an incoming peer session. `local_steam_id` is available from `runtime.network.status()` on each process and is returned as a decimal string so Lua does not lose uint64 precision.

The client and server must both run ShroudForge. This is a separate Steam P2P session and does not inject data into Enshrouded's native packets or forward messages through its server. Mods must arrange peer Steam IDs and authorization through their own configuration or application protocol.
