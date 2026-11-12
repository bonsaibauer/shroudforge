# Native map and overlay services

`modules/game-runtime/native/abi/map_overlay_api.h` defines optional C service
tables for engine map snapshots and owner-scoped drawing. Packages query a
table through the managed native host using their package identity and
activation generation. The host allocates the owner token.

A frame lease retains copied snapshot storage until release. Session changes
invalidate its use in a new session without freeing borrowed storage. Revoke
prevents new acquisitions and callbacks; drain waits for existing work before
releasing that generation's capture ownership. Revoked generations cannot
acquire replacement tables. Marker icon candidates are opaque engine metadata;
consumers choose their own assets and presentation.

The overlay draws rectangles and RGBA textures with rotation, tint and circle
clipping. Version 2 adds square clipping. It supplies no terrain sampler, fog
compositor, player/death effect presets or feature settings. Texture revisions
identify immutable content. The callback's encoder and spans are synchronous
and cannot be retained. The optional tables are introduced by this change;
consumers of earlier drafts must rebuild against the corrected header. The
base native runtime ABI remains version 12.

Vulkan device and swapchain creation must be observed through the prepared
startup broker. Late requests report why drawing is unavailable. Drawing
requires an observed unprotected graphics queue and color-attachment swapchain
usage. Device/swapchain destruction drains owned GPU work before forwarding the
original destructor. Unresolved completion cannot count as successful cleanup.
Textures retain their original dimensions.

Map and graphics activate independently. An overlay-only request does not
enable map hooks. `SFOverlayFrameV1.map_frame` is an optional synchronous
snapshot, valid only during the callback. The current host supplies a
`SF_MAP_DISABLED` snapshot when capture is inactive; it can also report no
session or unavailable subviews. Consumers must inspect its status and must
handle a null pointer. A combined map/overlay owner retains the ordinary map
lease contract independently of drawing callbacks.

Map capture requires a matching executable profile, complete engine-site byte
guards and a trusted assertion that the game entrypoint remains held. It only
normalizes independently verified framework-owned patches. Foreign detours
are unsupported and refused; no mod identity, version or hash can admit one.

Metadata identity is independent of generic execution-hook readiness. A
read-only metadata request does not permit component writes. Generic hooks
still require their actual readiness and original write checks. Metadata and
menu classification do not establish World incarnation or resource leases.

## Startup requirements

Packages request early preparation with `runtime-engine-hooks`, `runtime-map`
or `runtime-overlay`. Enabled managed native declarations for `shroudforge.map`
and `shroudforge.overlay` contribute the same demand. They require ordinary
DLL Manager approval and `runtime-register-dll` permission. Declarations request
preparation and do not prove readiness or grant another owner's service access.

The loader binds selected packages, parsed compatibility-profile contents,
ordered DLL entries and retained executable inputs before preparation. Authored
manifests, Lua sources, native binaries and archives are retained. Mixed settings
documents and native declarations are compared by parsed selection; package
data and logs remain writable. Changed inputs, repeated phases, incomplete
service masks and completion after the deadline stop startup.

The bootstrap prepares and activates the exact admitted mask, then commits it
on its one-shot restored-entry continuation before runtime creation. Ordinary
DLL execution combined with selected services is explicitly unsupported. The
ordinary data-driven DLL Manager path remains available for packages without
selected-service demand. Ordinary plugin initialization is not a pre-entry
provider contract. Removed capabilities, unknown plan bits and nonzero opt-ins
must fail closed rather than silently select a partial plan.

## Validation

Existing adjacent Rust tests exercise discovery, changed executable inputs,
mixed-DLL refusal and admission phases. Native CMake fixtures use synthetic
snapshots, callback owners, PE headers and GPU dispatch without game code.
The C consumer checks SDK layouts. GPU terminal-swap and terminal-device cases
run in separate processes. The bootstrap fixture exercises restored-entry
continuation, callback refusal and deadline arbitration with inert callbacks.
These checks do not establish game, driver, feature or multiplayer compatibility.
