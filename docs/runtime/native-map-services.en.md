# Native map and overlay services

`modules/game-runtime/native/abi/map_overlay_api.h` defines the C service tables
for map snapshots and overlay rendering. Packages query a table through the
managed native host using their package identity and activation generation.
The host allocates the owner token; a package cannot choose another owner.

A frame lease keeps its copied snapshot storage alive until release. Session
changes invalidate the frame for further use in a new session without freeing
borrowed storage. Revocation prevents new acquisitions and callbacks. Drain
waits for existing work before releasing that generation's capture ownership.
A revoked generation cannot acquire a replacement service table.

The overlay uses observed Vulkan device and swapchain creation. Its startup
broker must be prepared before those calls occur. Late requests report the
reason rendering is unavailable. Device/swapchain destruction drains owned GPU
work before forwarding to the original destructor. Drawing requires an observed
unprotected graphics queue and color-attachment swapchain usage. Renderer
resources are bound to that queue family; unsupported calls forward unchanged. An unresolved completion
cannot be treated as successful cleanup. Texture and terrain uploads preserve
their original dimensions; this layer does not reduce generated map assets.

Map capture and First Person preparation require the matching executable
profile and a trusted assertion that the game entrypoint remains held. First
Person has one active owner and retains disabled forwarding code after shutdown.
An owner release restores the default distance and prevents stale owners from
changing the new generation's controls.

Metadata can be identified independently of generic execution hooks. The
selected metadata path requires a complete executable identity and recognized
KFC header. Requesting read-only metadata does not permit component writes,
even if another feature has installed hooks. Requesting generic hooks still
requires their actual readiness and all original write checks.

Original Creative/Find/Teleport hook chains have bounded identity and lifetime
checks. Their presence is not a general promise of compatibility: startup must
complete the original owners' synchronous initialization and finalize the same
owner cohort before publishing service access. Unknown chains are refused.

## Integration requirements

The complete bootstrap selection, original-mod loading order and current-asset
schema admission must accompany these provider services. Optional private
Flight/Vein implementations remain separate dependencies. A missing provider
operation must remain unavailable; it cannot be replaced with a successful stub.
Metadata and menu classification do not establish an authenticated World
incarnation, a World retirement fence or an active-menu resource lease.

## Tests

The native CMake tests use synthetic snapshots, callback owners, PE headers and
GPU dispatch. They do not load game code. The C consumer layout assertions are
shared with the native plugin ABI check. Run the GPU test's `--terminal-swap`
and `--terminal-device` cases in separate processes. Game/driver and original
publisher compatibility still require their own qualification.
