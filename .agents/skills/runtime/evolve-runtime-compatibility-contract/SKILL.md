---
name: evolve-runtime-compatibility-contract
description: Change runtime profile schemas, readers, embedding, or capability contracts while preserving historical profile files and distinguishing static compatibility from live readiness and semantic approval.
---

# Evolve a Runtime Compatibility Contract

Read `.agents/skills/repository/repository-working-rules.md`. Use this workflow when the format or interpretation changes. Use the existing client/server profile skill when only a new executable build needs its own addresses and identity.

## Trace Every Reader and Gate

Read `dev/profiles/schemas/profile.schema.json`, relevant component/function catalog schemas, CLI generation/validation/approval, and `modules/game-runtime/native/CMakeLists.txt`. Follow both production paths:

- Execution profiles: `native/profiles/select_game_profile.cpp`, `read_game_profile.cpp`, and `game_profile.h`.
- Resource/read-owner profiles: `native/loaded-resources/initialize_resource_access.cpp`, `parse_resource_layout.cpp`, `native/engine-context/configure_engine_layout.cpp`, and the owner/Scene/lifetime resolvers.

Trace any version-specific metadata through the affected reader and its build inputs. Keep one canonical definition without assuming a new directory or rewriting old profiles. Trace `modules/game-runtime/compatibility/lib.rs` and its actual callers for operation availability. Its `GameContract` is internal and does not grant live readiness. A missing unrelated parser type must not disable unrelated provider operations.

The CLI execution-profile validator and production resource-profile reader do not accept identical shapes. Test the applicable contract instead of treating one validator's rejection as universal proof. CMake classifies and embeds profiles by their actual provenance. Draft files do not belong in its production profile glob.

## Preserve Historical Interpretation

Historical profiles must never be modified, renamed, deleted, reformatted, or bulk-rewritten. Hash them before and after any format/reader work. Maintain compatible interpretation through the canonical reader and narrowly versioned metadata when appropriate. New required semantics need their own explicit format/version handling. Do not make a new field retroactively required in every old build file.

For a new build, create a separate file by fully copying a same-target baseline. Retain every existing operation and behavioral contract, then change only proved image identity and build-dependent addresses. A schema change is not permission to add/remove operations from the user's requested address-only copy.

Keep exact executable identity, target, and KFC format separate. Read-validated resource selection requires the exact SHA-256, timestamp, and image size. Preserve the execution selector's established precedence and historical behavior without inventing a new fallback. Client replica reads must not become authority writes. Registered metadata discovery must not become entity access authorization.

## Evidence and Approval

Capture reports, unique static matches, live ownership, calling conventions, and gameplay effects prove different facts. Preserve those distinctions in provenance and status. Never mark a semantic verification flag true merely to satisfy a schema or make startup proceed.

`kfc-runtime-dev approve-profile <new-draft.json> --function-scan <scan.json> --components <catalog.json>` is the existing interactive approval action for its generated execution draft. It checks exact identity, unique candidates, unresolved mappings, and equality with the original live component capture, then requests two `VERIFY` attestations. Complete the underlying live-layout and semantic checks before those attestations. Never use this action on a historical file or invent a bypass.

The current `generate-profile` implementation resets selected derived write/model metadata. Read it before use. It is not the canonical 1:1 copy procedure for an address-only profile update.

## Verify

Build the native provider with `pwsh -NoProfile -File dev/build/build_kfc_runtime.ps1`. Build CLI tools explicitly, for example `cmake --build build/native-runtime --config Release --target kfc-runtime-dev`. Run `cargo test --release -p shroudforge-compatibility` when its contract changes.

Extend the existing private reader fixtures with historical and new format cases, malformed fields, bounds, ambiguity, authority, and required provenance as relevant. Preserve existing historical fixtures. Run affected explicit native targets such as `kfc-runtime-test-ecs-read`, `kfc-runtime-test-resource`, `kfc-runtime-test-service`, and `kfc-runtime-test-entity-command-queue`.

Verify historical file hashes are unchanged, inspect the generated embedding output, and report static validation separately from actual client/server startup and live calls.
