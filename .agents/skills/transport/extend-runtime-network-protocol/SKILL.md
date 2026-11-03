---
name: extend-runtime-network-protocol
description: Change Steam runtime transport or add a bounded authenticated mod request protocol, including framing, authorization, queueing, retries, and uncertain results.
---

# Extend runtime networking or a mod protocol

Read `.agents/skills/repository/repository-working-rules.md`. Identify whether the request changes shared Steam transport, a public Lua networking operation, or one mod's message protocol. Keep the implementation in its actual owner.

## Shared transport

`modules/transport/{lib.rs,lua/network.rs,native/steam_network.cpp}` owns the runtime transport. `read_network_allowlist.rs`, `report_network_health.rs`, and `ui/` own authorization configuration and health presentation. The mod runtime provides the current root, role, and denial context.

Read the full affected Rust/Lua/native path and its ABI/build caller. Preserve these current contracts:

- Enshrouded initializes Steam and pumps callbacks. This module reuses the existing interfaces and must not create a second callback pump.
- Public channel limits, reserved health channel, 512 KiB message bound, bounded receive counts, and bounded batches remain enforced.
- Mod messages carry a full destination/source envelope on a deterministic channel. Validate the destination despite possible channel collisions. Envelope `from_mod` is not an authenticated user identity.
- Server admission uses actual authenticated connected-player evidence and intersects the optional allowlist. A malformed nonempty restriction must not become unrestricted. Recheck receive/accept/queued dispatch authorization.
- Automatic remote discovery, local dedicated-server identity freshness, health probes, and cooldowns remain module-owned. Do not copy discovery or health polling into every mod.

A new public Lua operation also needs its canonical declaration and availability contract. Preserve native ABI/export compatibility.

## Mod request protocols

A concrete existing example is `mods/world-editor/src/p2p.lua`, assembled by `operations.lua::create_p2p_bridge`. It frames the existing SFBP V7 body with sequential 384 KiB chunks, checksums, a 32 MiB online transfer limit, bounded peer-specific uploads/FIFO queue/receipts, one active world action, and peer/request-scoped idempotency. `operations.lua` and `history.lua` own server behavior and journals, not the framing layer.

For a protocol change, trace both sender and receiver, all command/result/progress variants, size/count/time bounds, authentication/session identity, and duplicate/late/cancel behavior. Reuse the canonical domain action after validation. Do not duplicate world validation in a transport wrapper. Preserve unknown outcomes after a sent mutation times out. Missing replies must not trigger a blind mutation retry or clear unresolved journals. Read-only capture cancellation and mutation abandon have different semantics.

## Verify

Use meaningful transport tests and the real shipped Lua fixture (`modules/mod-runtime/tests/world_editor_lifecycle.lua` for editor protocols). Cover relevant malformed/incomplete/corrupt/spoofed/stale/duplicate/concurrent messages and authorization changes. Run the affected transport/runtime test targets. Rebuild native transport when changed. For requested live verification, test the intended client and dedicated server with actual authenticated peers and retained logs. Separate transport availability, delivery, server dispatch, readback, and confirmed effect in the report.
