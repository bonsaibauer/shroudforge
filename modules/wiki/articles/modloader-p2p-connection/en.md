# Modloader P2P connection

This page explains how the Modloader reaches a dedicated server over Steam P2P, what Steam encrypts, how client and server messages flow, and which Enshrouded user-group permissions allow server administration.

## Connect in game

1. Start Enshrouded and join a game world hosted by the dedicated server.
2. Open the Modloader and select **Server** as the target.
3. Wait for the server status to show **Connected**. This means the client recently received a Steam P2P health message. It does not verify that the sender is the intended Enshrouded server. The desktop window and the game’s main menu do not confirm an in-world P2P connection, so the server password field stays hidden there.
4. Enter the password for a qualifying <code>userGroups</code> entry. The server checks the password over the P2P connection.

## How the P2P connection works

The Network module runs inside the Enshrouded client or Dedicated Server process. The game initializes Steam, and ShroudForge reuses the Steam Networking Messages interface already loaded in that process to send directly to a Steam identity. ShroudForge does not run a relay server or place these messages inside Enshrouded gameplay packets. While the Network module is enabled, the runtime advances the P2P health and remote Modloader services. See the [runtime tick](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/mod-runtime/ingame/update_ingame_runtime.rs) and the [native Steam adapter](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/native/steam_network.cpp).

## Transport encryption and channels

Modloader requests and responses are encoded as JSON bytes. Runtime mods may send binary Lua strings. ShroudForge does not add its own TLS session, payload cipher, or end-to-end key exchange here. It relies on Steam Networking for peer authentication and transport encryption. Valve documents that <code>ISteamNetworkingMessages</code> uses the underlying <code>ISteamNetworkingSockets</code> connections and describes Steam Networking's encryption and authentication. That protects transport to a Steam peer. It does not prove that the peer is the Enshrouded server the user intended to reach, and it does not protect either endpoint. Channel numbers only route messages and do not enforce authorization. The Lua API accepts channels 0 through 65533 and rejects the two reserved channels. Internal Modloader messages request reliable delivery, which affects delivery and ordering but is not encryption or authorization. See the official [ISteamNetworkingMessages documentation](https://partner.steamgames.com/doc/api/ISteamNetworkingMessages) and [ISteamNetworkingSockets documentation](https://partner.steamgames.com/doc/api/ISteamNetworkingSockets), plus the [transport channel definitions](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lib.rs) and [Lua Network API](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lua/network.rs).

| Channel | Current use |
|---|---|
| <code>0</code> through <code>65533</code> | Runtime Lua mod messages |
| <code>65534</code> | Internal Modloader server mirroring and remote control |
| <code>65535</code> | Steam P2P health probes |

## Client and server message flow

| Direction | What the current code sends |
|---|---|
| Client to server | The client selects <code>modules/network/serverSteamId</code>, or falls back to the most recently discovered remote server Steam ID when that setting is blank. It sends a subscription about every 10 seconds. After subscribing, it can send the admin password, then a session token with settings changes, mod-list requests, and mod operations. The server forgets peers that stop refreshing for 30 seconds. |
| Server to client | A new subscription immediately receives the server’s loader configuration snapshot and the latest 64 KiB of each mirrored log. The server then sends configuration changes and raw log deltas to subscribed peers. Admin-authenticated requests also receive mod lists, server compatibility data, session results, and operation results. |
| Runtime mod to runtime mod | A Lua mod sends to an explicit Steam ID. <code>send_mod</code> adds a JSON envelope and chooses a deterministic channel from the destination mod ID. The receiver checks the envelope protocol and destination mod ID. The claimed <code>from_mod</code> value is supplied by the sending mod and is not proof of which mod authored the message. |

The client submits local UI requests through a process-local bridge, and the game runtime performs the Steam send. On the server, the sender’s Steam identity must be in the current authorized peer set. The native adapter builds this set from Dedicated Server log events for online peers and Steam authentication. The optional <code>allowedClientSteamIds</code> setting filters that set further. When it is blank, it adds no extra allowlist restriction. The server sends the subscription snapshot and log stream before admin login, so this peer check also controls access to those data. See the [remote UI service](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs), [local bridge](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/local_bridge.rs), and [Network allowlist](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/read_network_allowlist.rs).

## Required server group permissions

The entered password must exactly match a group with a non-empty <code>name</code>. That same group must set all five permission fields below to the JSON boolean <code>true</code>. A missing field, <code>false</code>, or a non-boolean value does not qualify the group for Modloader server administration.

~~~json
"userGroups": [
  {
    "name": "Admin",
    "password": "REPLACE_WITH_A_PRIVATE_PASSWORD",
    "canKickBan": true,
    "canAccessInventories": true,
    "canEditWorld": true,
    "canEditBase": true,
    "canExtendBase": true,
    "reservedSlots": 0
  }
]
~~~

<code>reservedSlots</code> controls reserved player slots and is not part of the Modloader permission check. ShroudForge sends the submitted password in a JSON request to the selected Steam peer and does not store it. The server compares it directly with the password in <code>enshrouded_server.json</code>. This code does not hash or encrypt that configuration value at rest. After a successful check, the server issues a token bound to the Steam peer and keeps it in memory on both endpoints. Admin writes recheck the token, peer, and group permissions. Five failed password attempts start a 60-second login cooldown. See the [server authorization code](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs).

## Security note

When <code>serverSteamId</code> is blank, the client can use the most recently discovered peer from a health message on channel <code>65535</code>. This setting is blank by default. A received <code>SFN1:PING:</code> message updates that discovered ID, and **Connected** reflects recent health traffic rather than a verified Enshrouded server identity. If a peer can establish a Steam message session and send that health payload, it may be selected as the destination. Steam encryption would then protect the connection to that peer, not establish that it is the user’s server. Check or pin the server Steam ID before entering the password. Also treat subscribed peers as trusted for the configuration and log data sent before admin login. Runtime mods that consume <code>send_mod</code> messages should validate the Steam peer, claimed sender mod, and payload themselves. These are source-level observations, not confirmation of a live exploit. See the [ShroudForge security policy and private reporting instructions](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/SECURITY.md).
