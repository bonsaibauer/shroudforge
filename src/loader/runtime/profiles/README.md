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
| `components` | ECS component index map. Each row has the game's qualified type `name`, registry `index`, and byte `size`. Keep rows ordered by index. |
| `runtimePatches` | Explicit executable byte patches exposed as `runtime.patch.*`; these are code patches, not ECS component data. `signature` locates code, `payload` contains replacement bytes, and `function` records the owning function range. |

Numeric byte arrays use decimal values from 0 through 255. Signatures are
space-separated hexadecimal bytes; `??` marks a wildcard. RVA fields are
addresses relative to the executable image, while fields ending in `Offset`
are byte offsets within their documented structure or context.

Do not add fields to only one build profile without updating
`profile.schema.json`, the native profile reader, and the profile generator.
