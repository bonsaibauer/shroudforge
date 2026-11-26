# Runtime profile format

Each JSON file describes one game executable build and process target. Keep the
sections in this order so the file reads from identity, through runtime layout
and ECS metadata, to executable patch definitions.

This directory is the source of the shipped external JSON catalog. Packaging
copies it to the installation's configured `runtime/profiles` directory, which
is shared by execution and resource/read-owner selection. The provider embeds
no profiles and has no embedded fallback. Updating profile data does not require
rebuilding the provider. Exact image identity, provenance, guards and live
ownership checks still apply.

The optional `game-profile-discovery` native module publishes experimental
`beta_*` profiles into this same installed catalog after comparing the exact
old and current executable images. It can verify mapped-image equality or
derive individual addresses and layout evidence from original code and data.
Normal profiles always have selection priority. It preserves the complete
baseline shape and existing gates. Generated files have a `beta_` filename and
ID, with proof in the existing cache. A missing source image, ambiguous address
or unproved layout field leaves the complete candidate unresolved.

Historical game-build profiles are immutable references. Create a separate
file for each new build and verify the baseline file's SHA-256 before and after
the work. Do not migrate or normalize historical profile files when extending
the format.

New client profiles use `<SteamBuildId>.json` and the ID
`enshrouded-client-<SteamBuildId>`. The name covers the complete validated data
connection. Individual read and write capabilities are specified by the profile.

An additive contract for an already shipped exact image is a separate complete
`<SteamBuildId>.revision-<N>.json` file with the same stable `id` and a positive
`profileRevision`. The historical file is revision zero and stays byte-identical.
The shared installed-catalog reader selects the unique highest revision before
execution/resource selection and discovery. Schema, target, KFC format and the
entire image identity must agree within that ID. Files are never merged.
Missing old sections are not inherited. Duplicate highest revisions remain
ambiguous. Invalid revisions or conflicting identities exclude that ID and
block discovery for that read, preventing fallback to an older baseline.
Unversioned duplicates retain their existing ambiguity. Launcher IDs do not
change. Resource status `profileSelection.selectedSource` reports the selected
file path and revision. Revisions grant no additional engine permissions.

The developer capture action is an explicit review input, not a runtime profile
selector. If several complete files match its executable, choose one with its
existing `-ClientBaselineProfile` or `-ServerBaselineProfile` option. For exact
matches, the supplied file must be one of those exact matches. It does not
duplicate the native catalog's automatic revision policy. Unknown-image review
continues to use the existing explicit baseline or unambiguous newest baseline.

| Section | Meaning |
| --- | --- |
| `schemaVersion`, `id`, `target` | Profile format version, stable launcher selection ID, and executable name. |
| `kfcFormat` | KFC directory format expected for this build, currently `3` or `4`. The parser confirms the file's own header marker. |
| `image` | Executable identity: PE timestamp, image size in bytes, and optional SHA-256. |
| `allowStructuralRevalidation` | Whether automatic selection may try this execution profile when executable identity differs within the same KFC format. Manual selection also requires the same KFC format. |
| `ownerLayout` | Exact-image original singleton, client and controller ownership shared by ResourceService and ECS readers. |
| `hudCapture` | Optional `hud-observation-v1` description for the existing engine HUD observer. Exact-image UI chain, renderer CALL, guards and observation limits. Does not enable Map, Overlay or arbitrary DLL hooks. |
| `mapCapture` | Optional `map-image-admission-v1` dependency descriptions shared by map admission and renderer-only admission. Static image checks, never activation or permission to invoke game code. |
| `ecsReadLayout` | Validated original execution-owner, registry-owner and entity-manager relation plus entity storage offsets. Enables stable reads independently of hooks, with `provenance.ecsReadValidated=true`. |
| `resourceLayout` | Original ItemRegistry pointer, ResourceService callbacks used only for validation, resource index and chunk layouts. |
| `resourceWriteLayout` | Exact-image, separately validated content-worker lifetime and private bulk scalar CAS. Requires resourceAtomicScalarWriteValidated and its evidence. Uses the shared thread registry fields in ecsWriteLayout. |
| `ecsWriteLayout` | Original registered local-authority thread lifetime and Scene execution gates. The current untracked-components policy admits only registrations with both original tracking bits clear. |
| `entityCommandQueueLayout` | Exact-image manager offsets for the original `ecs::Tasks` state, task records, queue records, entity-ID counter and active tracking contexts, plus task stride and command-buffer capacity. Used only by admitted original-operation adapters after the existing authority and Scene gates. |
| `buildingPropSpawnLayout` | Exact-image query source entity-ID offset and the allocation-failure marker used by the complete original building-prop helper contract. |
| `ecsLayout` | Byte offsets used to read the entity manager, entities, and component storage. These are offsets, not absolute addresses. All field names use `camelCase`. `entityDefinitionPointer` is the entity record's pointer to its definition. |
| `entityDefinitionLayout` | Offsets of the entity template UUID and name fields. |
| `hooks` | Signature and original instruction bytes for runtime entry hooks. `captureOffset` is only used by the cursor hook. |
| `worldContexts` | Offsets between the live game services used by world operations. |
| `worldGrids` | Per-build grid IDs, world origins, cell sizes, and maximum dimensions exposed by the native world API. |
| `worldOperations` | Guarded game functions or globals used by `runtime.world.*`. `*Rva` values are relative virtual addresses. |
| `componentResolution` | Shipped profiles use `live-registration`. the provider derives component identities/indices/sizes from the engine. Do not duplicate them in `components`. |
| `components` | Optional legacy fallback only, mutually exclusive with `componentResolution`. |
| `attributeCalculationModel` | Exact-build authorization for the independently tested owned attribute interpreter. Removed when generating a new-build draft. |
| `runtimePatches` | Guarded executable interventions, now attached to `runtime.functions` through `modifier`. legacy `runtime.patch.*` keys remain backend aliases. `function` identifies the unwind fragment. `modifier.function_rva` identifies the chained primary root. |

Numeric byte arrays use decimal values from 0 through 255. Signatures are
space-separated hexadecimal bytes. `??` marks a wildcard. RVA fields are
addresses relative to the executable image, while fields ending in `Offset`
are byte offsets within their documented structure or context.

Read profiles use `provenance.status="read-validated"`, an exact SHA-256 image
identity and `ownerLayout` with `resourceLayout`. They do not contain execution
hooks or native action definitions. Optional `ecsReadLayout` has its own live
validation and owner-chain evidence. Original component registration records
remain the only component name/index/size source for this path.

The optional write sections carry their own exact-image evidence. Resource
writes are limited to one aligned 32-bit scalar in the original private bulk,
with restored data-page protection. They do not mutate executable pages or
publish engine notifications. Component writes preserve complete snapshot CAS
and the original component tracking policy. The profile generator removes both
write sections, `entityCommandQueueLayout` and `buildingPropSpawnLayout` when
producing a version-2 draft for another image. Original consumer dataflow and
live effect checks remain separate from structural authorization.

`entityCommandQueueLayout` uses `id="ecs-entity-command-queue-v1"`. Its seven
`*Offset` fields are byte offsets from the selected live entity manager, not
RVAs. `taskStateOffset` identifies the state addressed by the original
`ecs::Tasks` service. `taskRecordsOffset` and `taskCountOffset` locate the task
array pointer and count. `queueRecordsOffset` and `queueCountOffset` locate the
queue array pointer and count. `entityIdCounterOffset` identifies the shared
entity-ID counter, and `activeTrackingContextsOffset` identifies the original
active-context count that must be zero. `taskRecordStride` is the byte distance
between task records. `bufferCapacity` is the exact byte capacity required of
each original command buffer. It must cover the admitted adapters' largest
160-byte command snapshot. All nine numbers are required integers. The shared
tracker continues to use the canonical
`ecsWriteLayout.changeTrackerContextOffset`.

This section belongs to the one exact executable profile selected for the
current image and process target. It requires KFC4, SHA-256 identity,
`ecsReadLayout`, enabled `ecsWriteLayout` and the existing validated ECS read,
authority, write-gate and change-tracking evidence. It adds no approval flag,
build-ID dispatch or structural fallback. Original bindings, ownership, memory
ranges, task/queue links and stable readback remain runtime checks. A layout
alone does not authorize arbitrary queue writes or native calls. Profile
generation for another image removes the complete section so its values must
be established again for that build.

`buildingPropSpawnLayout` uses `id="building-prop-spawn-v1"` and requires
`sourceEntityIdsOffset` and `allocationFailureMarkerValue`. The source offset
is the original query-context field used by the source helper. The unsigned
32-bit marker is zero for the former helper without the extra fatal-tail
store, or the exact observed store value for a helper with that instruction.
The adapter validates the complete selected instruction contract, including
the failure tails. These values neither select a function address nor admit
an arbitrary helper.

New KFC4 write profiles use `schemaVersion: 2` and provide both contracts
explicitly. Version 2 never inherits missing layouts. Version-1 profiles keep
their original JSON bytes and their existing native constants. Only version 2
reads the new queue and spawn values from the selected build profile. The new
sections belong exclusively to version 2. Neither version uses structural
revalidation for another image on this exact-image path.

The optional active-scene fields in `ecsReadLayout` additionally require
`provenance.ecsAuthorityReadValidated=true`. They select exactly one ready local
authority scene before falling back to the client replica when no local scene
exists. A transitioning or ambiguous scene is unavailable. Authority selection
does not authorize writes or task submission.

`modifier.engine_name`, when present, must match an original loaded execution
descriptor. `modifier.id` is explicitly a ShroudForge intervention name.
`attribute_requirements` checks stored KFC IDs, names, root IDs and element
indices before Lua can bind an attribute intervention. Static caller paths are
evidence of shared-helper reachability, not complete native signatures.

`inlineReferences` describes displacement/next-instruction/data offsets inside
a trampoline. The provider derives these relative references after copying the
payload. it never reuses an original EXE-relative displacement for inline data.

World operations with `validated: false` are always disabled, including when a
candidate address happens to match an instruction guard. `unresolvedReason`
states the missing evidence. Such entries may omit addresses entirely. a
candidate `functionRva` only supports an inspectable partial function binding.

Do not add fields to only one build profile without updating
`profile.schema.json`, the native profile reader, and the profile generator.

`hudCapture` is parsed only from the exact-image owner profile selected through
the existing resource/read-owner path. Its `callRva` must have an original E8
relative CALL guard targeting `rendererRva`. `contextGlobalRva` is the root
pointer. The observer follows `context + holderOffset`, dereferences the holder,
reads `state + playStateOffset`, then reads one byte at `play + screenOffset`.
`minimumScreen`, `maximumScreen`, `gameplayScreen` and `maximumAgeMs` bound the
observation. No pointer is exported. The render callback retains an immutable
copy of the selected contract across forwarding and logical shutdown.

Each guard has `guardRva` and lowercase hexadecimal `bytes` (1–64 bytes).
There must be 2–32 non-overlapping, image-bounded guards, including the renderer
entry and complete original CALL. All guards remain mandatory. `guardPolicy`
is either `exact`, or `runtime-owned-prefix` to use the existing independently
verified runtime prefix proofs. It does not accept a mod's ownership claim.
Only an independently verified framework-owned CALL may normalize its first
five bytes. Following instructions remain guarded. An absent or malformed
optional HUD description makes HUD capture unavailable without disabling
unrelated resource or ECS operations. Hook publication still uses the existing
shared engine-hook writer and lifecycle gates.

The revision-one client files for builds 26082781 and 26430625 migrate the
previous PR #116 HUD descriptions and all nine guard spans verbatim. Their
source attribution remains in `native/map/SOURCE-NOTICE.txt`. This migration
adds no new live validation or semantic approval flags. Other map execution data are described below in mapCapture.execution.

`mapCapture` uses the same exact-image owner selection and immutable control-plane
snapshot as `hudCapture`. It describes nine views: `player`, `renderer`, `markers`,
`clock`, `weather`, `fog`, `world`, `deaths` and `discovery`. Every view contains
`guards`, `systems` and `pointers`. Guards have `guardRva` and lowercase hex
`bytes`. Systems have `descriptorRva`, `name` and `callbackRva`. Pointers have
`addressRva` and `targetRva` and must equal the actual loaded image base plus
the target. They are not wildcarded. The weather view may additionally specify
`enumTable` with `tableRva`, `entryStride`, `namePointerOffset`, `valueOffset`
and ordered `names` whose original numeric values must equal their indices.

`requiredSystems` preserves the map-wide dependencies. `readiness` is either
`all-views` or `player-only`, retaining the previous build-specific completeness
policy. `deathsRequireWorld` retains the previous death-view dependency.
`guardChunkBytes` preserves either 256-byte chunks or the legacy whole-guard
1024-byte maximum. Differences larger than 512 bytes cannot use runtime prefix
normalization. All original bytes and original named-system identity checks
remain mandatory for the requested view. Per-view guards are limited to 4096
spans, each 1–65536 bytes, with a 4 MiB aggregate byte budget. Legacy whole guards
remain limited to 1024 bytes. Contradictory overlapping guards are rejected.
Malformed optional descriptions disable only the affected image service.
The existing resource status reports parsing under
`profileSelection.mapCaptureProfile` (`parsed` and `status`). This is static
profile information, not map/renderer activation, World readiness or permission.

Renderer-only admission checks exact identity and the renderer's own dependencies,
independently of map-wide named systems and map activation. It uses the same
matcher and profile. This does not create a second graphics profile catalog.
The two new, unpublished revision-one files in PR #116 retain all their HUD and
previous profile data and now include all 188 former map guard spans (41191 bytes),
81 image pointer relations and the existing system/enum checks. Source attribution
remains in `native/map/SOURCE-NOTICE.txt`. Historical profile files stay unchanged.
The optional mapCapture.execution describes the known map-capture-checked-v1 ABI.
It includes seven guarded hook sites, original bytes and CALL targets, guarded
iterator/component lookup function RVAs, build-dependent record offsets and the
native-iterator-extent-v1 control/copy extents. Readers validate all offsets and
original/target relations against the same image guards. Generic machine-code
calling conventions and known record decoding stay in reviewed native code.
This is a bounded data description, not a programmable hook script.

Execution may instead contain unavailableReason. The 26082781 candidate uses
legacy-iterator-extent-unproved. Malformed or missing execution disables map
capture without disabling renderer-only image validation. The 26430625 candidate
explicitly marks discovery-position-layout-unproved under unavailableViews.
No heuristic coordinate scanner or late installer remains. The diagnostic
profileSelection.mapCaptureProfile.executionStatus reports admission separately
from the overall profile, world presence and actual effect. These data migrate
the existing PR contracts and do not create new live validation evidence.

Discovery preserves `hudCapture` and `mapCapture` only when complete mapped-image equivalence
has been established. A changed mapped image reports
`hud-capture-rebase-not-supported` or `map-capture-rebase-not-supported`, publishes no beta and cannot be approved by
the ECS live-layout prover. `generate-profile` likewise rejects such a baseline
instead of copying or dropping its HUD contract. A complete HUD rebaser remains
required before those profiles support automatic discovery across code changes.

Server `runtime.world.context.active` uses ABI `actor-world-context` with an
instruction guard and store-validation offset. It intentionally has no client
global RVA. The cursor hook is optional (the headless server has none).
World operation guards establish native code availability. live context checks
and the game-thread dispatcher still gate actual calls. Zero in the original
finish event slot is a valid argument.
