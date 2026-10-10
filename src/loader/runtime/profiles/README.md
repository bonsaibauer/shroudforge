# Runtime profile format

Each JSON file describes one game executable build and process target. Keep the
sections in this order so the file reads from identity, through runtime layout
and ECS metadata, to executable patch definitions.

| Section | Meaning |
| --- | --- |
| `schemaVersion`, `id`, `target` | Profile format version, stable launcher selection ID, and executable name. |
| `image` | Executable identity: PE timestamp, image size in bytes, and optional SHA-256. |
| `allowStructuralRevalidation` | Whether automatic selection may try this profile when executable identity differs. A manually selected profile is tried regardless. |
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

Server `runtime.world.context.active` uses ABI `actor-world-context` with an
instruction guard and store-validation offset. It intentionally has no client
global RVA. The cursor hook is optional (the headless server has none).
World operation guards establish native code availability. live context checks
and the game-thread dispatcher still gate actual calls. Zero in the original
finish event slot is a valid argument.
