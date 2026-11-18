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

## Programmable overlay v3

Query `shroudforge.overlay` version 3 using the existing `SFOverlayServiceV1`
table. The existing callback signature stays the same. Inside a v3 callback,
check the base size/version before casting the frame to `SFOverlayFrameV3`
and the encoder to `SFOverlayEncoderV3`. The v3 encoder extends v2; both its
base size/version and the frame's base size/version describe the full v3 value.
On x64, ShaderV1 is 40 bytes, ProgramCommandV1 is 80, and EncoderV3/FrameV3
are 64. V1/v2 layouts and behavior are unchanged.

`draw_programs` draws the host's fixed six-vertex quad. Its positive destination
rectangle must fit entirely inside the observed framebuffer, with each side
at most 16384 pixels. The viewport and scissor enforce this rectangle. UV
location 0 is `(0,0)` at the top-left and `(1,1)` at the bottom-right. The fixed
vertex stage has no descriptors or push constants. Fragment programs own all
rotation, clipping, compositing and other rendering behavior.

The supported fragment contract is intentionally narrow:

- Vulkan 1.0 / SPIR-V 1.0, exactly one Fragment entry named `main` and the
  `OriginUpperLeft` execution mode. Location 0 output is one float32 vec4,
  with straight alpha. Optional location 0 input is a smoothly interpolated
  float32 vec2. Optional `FragCoord` input is supported; other inputs/outputs
  and interpolation modes are refused.
- Set 0 bindings 0 through 3 are combined float sampler2D values. Arrays,
  storage images/buffers, uniform buffers and other descriptor types are
  unsupported. Missing bindings sample transparent black `(0,0,0,0)` from a
  1x1 image. Textures use normalized UV, linear filtering, clamp-to-edge and
  a single mip level. The existing RGBA8/coverage-mask contract applies.
- An optional push-constant block starts at offset 0 and fits in 128 bytes.
  Explicit layouts may contain 32-bit scalar/vector/matrix and fixed-array
  members, with nesting at most 16. Constants are copied synchronously; their
  byte count is a multiple of four and at most 128. Remaining bytes are zero.
  All 128 bytes are fragment-only; the host consumes none of them.
- SPIR-V is at most 256 KiB, word-sized, with ID bound at most 65536. Supported
  capabilities are Shader, Matrix, ImageQuery and DerivativeControl. The only
  extended instruction import is `GLSL.std.450`; SPIR-V extensions and grouped
  decorations are refused. Khronos SPIRV-Tools performs complete Vulkan
  validation before a separate restricted-interface check. Only validated
  host-owned words reach shader/pipeline creation.

Set `shader.size=sizeof(SFOverlayShaderV1)` and `shader.version=1`. Owner,
program ID, revision and device generation identify an immutable program.
Changing textures or destination geometry does not create a new pipeline.
The shader span is copied only when that program revision is first cached;
cached revisions must retain the same byte count. Use a new revision to change
shader contents. Each image caches at most 32 programs per owner and 128 total.
Cache pressure returns `BUSY`; changed revisions can replace older revisions
only when no command in the current image frame references them.

Each v3 owner has a shared budget of 4096 primitive commands from the start
of every callback, counting rectangles, v1/v2 textures and programs together.
Requests beyond the budget return `TOO_LARGE`. V1/v2 owners retain their old
limits. The existing global frame bound, 512 texture entries and 256 MiB
per-image texture budget remain; individual textures retain the 4096-pixel-side
and 64 MiB limits. A failed callback prevents submission of its partial batch.
Propagate drawing failures through the callback result.

Pipeline caches, sampled images and per-draw descriptor sets belong to a
specific device generation and swapchain image. Only a confirmed image fence
permits descriptor recycling or cache eviction. Retiring or uncertain renderers
retain these resources through the same completion/destruction rules as v1/v2.
No Vulkan handle crosses the service API.

V3 performs no implicit color conversion. `target_encoding` describes the
existing renderer's output convention; all reserved fields are zero:

| Encoding | Required shader RGB output |
| --- | --- |
| `SRGB_VALUES` | Existing encoded RGB values, unchanged, matching the legacy SDR path. |
| `SCRGB_LINEAR` | Decode sRGB to linear Rec.709, then multiply by `diffuse_white_nits / linear_unit_nits` (203/80). |
| `HDR10_PQ` | Decode sRGB to linear Rec.709, convert to Rec.2020, then apply ST.2084 PQ using diffuse white 203 nits and peak 10000 nits. |

The scRGB and PQ modes correspond to the observed renderer modes previously
used by the generic texture shader. Other observed modes retain the legacy
encoded-value convention. Alpha remains straight coverage. The Rec.709-to-2020
rows are `(0.627404,0.329283,0.043313)`,
`(0.069097,0.919540,0.011362)`, and `(0.016391,0.088013,0.895595)`.
For linear Rec.2020 channel `c`, set `y=max(c,0)*203/10000`,
`p=y^(2610/16384)`, then output
`((3424/4096+(2413/128)*p)/(1+(2392/128)*p))^(2523/32)`.
Frame metadata supplies 203-nit white, 80-nit linear units only in scRGB mode,
and 10000-nit peak only in PQ mode. Shaders may recompile against fixed UV;
binary compatibility with earlier custom vertex stages is not provided.

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
