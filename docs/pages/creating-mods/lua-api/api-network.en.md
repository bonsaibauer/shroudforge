---
{
  "id": "api-network",
  "locale": "en",
  "title": "Steam P2P messages between mod processes",
  "summary": "runtime.network provides a separate Steam messaging channel. It does not send normal Enshrouded game packets.",
  "navigation": {
    "path": [
      "Lua API guides",
      "Feature guides"
    ]
  },
  "apiPrefixes": [
    "runtime.network.",
    "RuntimeNetworkStatus.",
    "RuntimeNetworkSendOptions.",
    "RuntimeNetworkReceiveOptions.",
    "RuntimeNetworkMessage.",
    "RuntimeModNetworkMessage."
  ]
}
---

# Steam P2P messages between mod processes

`runtime.network` sends messages between mod runtimes on a client and a Dedicated Server. It is a separate Steam Networking Messages transport, not an Enshrouded game packet. See [targets](#doc-targets) as well.

## Messages for a specific mod

`send_mod` sends UTF-8 text to one SteamID64 and mod ID. The receiving mod reads it with `receive_mod`. Use this when both sides install the same mod. There is no broadcast.

## Raw channels

`send` and `receive` use binary-safe Lua strings. Public channels are 0 through 65533. Channel 65534 is reserved for Modloader file mirroring and channel 65535 for the internal health probe. Both sides must use the same channel.

## Server authorization and SteamID64

The Dedicated Server accepts only players Enshrouded has currently authenticated and connected. An optional allowlist under Modloader Settings → Network can further restrict this set. An empty list means all currently authenticated players. The client learns the server SteamID64 from live world context and the health probe. `serverSteamId` is a fallback for remote servers.

## Check the result

A successful `send` call means Steam accepted the send request. It does not confirm that the other process received or handled the message. Validate the protocol, sender, mod ID, and permissions on the receiving side. The API index documents payload limits and return values.
