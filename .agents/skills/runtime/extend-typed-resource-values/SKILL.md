---
name: extend-typed-resource-values
description: Extend ShroudForge reflected resource or owned-value conversion, encoding, field paths, or checked loaded-resource access. Use for typed adapter and storage support, rather than an ordinary mod setting.
---

# Extend typed resource and value support

Read `.agents/skills/repository/repository-working-rules.md`. Identify the actual value kind, reflected layout, storage owner, and operation first. The direct `kfc-parser/` submodule owns KFC formats and parser storage codecs. Keep parser changes there and integration changes in their existing module.

## Select the existing path

- KFC mapped/owned values exposed to Lua: `modules/mod-runtime/lua/resource_values/{convert_resource_values,mapped_values,owned_values,validate_resource_values,value_path}.rs`.
- Owned construction/encode/decode/normalization and typed paths: `lua/values/`.
- Type selector/registry identity: `lua/bindings/game/resolve_type.rs` and `types.rs`.
- Loaded snapshots/indexes: `lua/ingame/resources/` and `session/{loaded_resources,loaded_resource_indexes}.rs`.
- Reflected inheritance and generated Lua types: `reflection/traverse_reflected_types.rs`, `lua/definitions/generate_game_type_definitions.rs`.

Read the complete touched owners and both their reads and writes. Follow session creation and native bridge consumers when live data is involved.

## Preserve representation and ownership

1. Add the missing kind or field behavior at the canonical codec/resolver. Keep original signedness, width, enum/flag meaning, GUID representation, array indexing, native padding, and float32 normalization.
2. Preserve bounded field paths, cycle/depth and overlap checks, registry identity, and exact layout validation. Struct/typedef inheritance is not array element inheritance.
3. Owned values cannot allocate native DS storage. Inline paths must reject external descriptors. Resource paths may follow only already validated mapped BlobArray/BlobVariant data and an admitted aligned scalar. Do not turn a snapshot adapter into arbitrary native pointer traversal.
4. For live writes, preserve phase/capability/authoritative scope, session generation, expected current value and dependent-field comparisons, fault/rollback semantics, and a fresh returned snapshot.
5. Use the shared incremental loaded-resource index owner. Preserve complete-scan uniqueness, ambiguity, work/memory limits, negative-result expiry, key-overlap invalidation, and a fresh final snapshot. Do not add a per-mod scan cache to hide an incomplete lookup.
6. Update canonical Lua/type declarations only if public behavior changes. Use the existing type-definition/export build rather than generated-output edits.

## Verify

Use meaningful fixtures in `lua/values/tests.rs`, `lua/ingame/resources/index_scalar_tests.rs`, `session/loaded_resource_indexes_tests.rs`, or the closest existing owner. Cover the new representation and relevant malformed/stale/overlapping input. Run `cargo test -p shroudforge-api`. Corpus tests requiring installed captures/cache are explicitly separate and often ignored. Run them only with the required evidence and report that prerequisite. Ordinary tests do not establish native write correctness. Rebuild the parser/product through the existing developer actions if those owners changed.
