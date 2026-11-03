# Repository Workflow Skill Audit

Date: 2026-11-02.

The requested work dissolved the root AGENTS.md into task-specific English skills, audited the first-party code for reusable workflows, and preserved the historical game-build profile rule.

## Delivered Instructions

Fifty new task skills and the two existing client/server profile skills are present under .agents/skills/. Each has a concrete trigger, canonical source owners, real callers and data/build boundaries, and appropriate verification guidance. The two shared references contain cross-task working rules and the client/server profile procedure.

All 27 root instruction statements have destinations recorded in [the root-rule migration](2026-11-02-root-agents-rule-migration.en.md). The root AGENTS.md was removed after destination checks. Nested module instructions remain in place.

| Skill group | Count | Tasks |
| --- | ---: | --- |
| build | 2 | build-release-package, inspect-release-security-report |
| catalog | 1 | export-nexus-mod-catalog |
| debug-console | 1 | change-debug-console-behavior |
| diagnostics | 1 | add-diagnostic-job |
| entrypoints | 1 | change-bootstrap-entrypoint |
| game-files | 1 | change-startup-asset-preparation |
| loader-config | 1 | add-loader-setting |
| modding | 10 | add-mod-keybind, add-mod-locale, add-mod-setting, add-mod-ui-action, add-reversible-runtime-mod-behavior, create-lua-mod, create-mod-extension, extend-mod-extension-schema, extend-mod-manifest-schema, integrate-native-mod-dll |
| modloader | 4 | add-mod-conflict-rule, add-mod-link-provider, add-native-mod-adapter, add-ui-host-command |
| notifications | 1 | add-news-notice |
| parser | 1 | update-parser-integration |
| repository | 2 | audit-repository-code, refactor-repository-module |
| runtime | 11 | add-lua-api, add-native-engine-operation, capture-runtime-evidence, change-runtime-mod-lifecycle, create-profile-client, create-profile-server, evolve-runtime-compatibility-contract, extend-native-runtime-abi, extend-runtime-operation-contract, extend-runtime-provider-adapter, extend-typed-resource-values |
| transport | 1 | extend-runtime-network-protocol |
| ui-resources | 2 | add-ui-translation, refresh-crowdin-progress |
| updater | 7 | add-mod-download-provider, add-updater-migration, change-mod-package-installation, change-system-installation-transaction, change-system-update-check, change-updater-queue-operation, change-updater-worker-lifecycle |
| website | 2 | add-page-integration, add-website-page |
| wiki | 1 | add-modloader-wiki-article |
| world-editor | 2 | add-world-editor-action, change-blueprint-persistence |

## Source Reading Coverage

The inventory includes executable first-party source, build/CI files, manifests, schemas/contracts, tests and authored code examples. Every included path was content-read in full at least once. Large files were read in bounded chunks. Hashing, file enumeration, searching, and truncated output were not counted as full reading.

| Inventory partition | Full content reads | Included paths |
| --- | ---: | ---: |
| loader-updater | 331 | 331 |
| lua-mods | 260 | 260 |
| native-entrypoints | 294 | 294 |
| root-features | 238 | 238 |
| Total | 1123 | 1123 |

Path reading coverage is 100% for the 1,123-path inventory. [The source ledger](2026-11-02-skill-audit-source-inventory.csv) records each path, its inventory hash, coverage evidence, report-time hash and any final-delta read hash.

This was an evolving shared working tree, not an audit of one immutable Git revision. Concurrent native/profile work changed files during review. The final delta identified 22 paths. Eight delta snapshots were read in full, three changed again and one was removed at that checkpoint. Fourteen delta files were not fully reread before the user's instruction to finish and commit. Report-time hashes expose later differences. The path-coverage statement must not be interpreted as certification of every final concurrent byte or a completed current product review.

At report generation, 20 inventoried source paths differ from their inventory hashes. These source/profile changes are outside this task's writes and commit.

## Explicit Scope Boundaries

| Category | Inventory entries |
| --- | ---: |
| asset-or-content | 77 |
| development-planning-data | 9 |
| documentation | 421 |
| external-submodule | 3 |
| generated-game-snapshot | 4 |
| generated-lockfile | 3 |
| historical-evidence | 88 |
| translation-content | 91 |

External submodules are kfc-parser, modules/game-runtime/native/json and modules/hotkeys/SDL. Their complete contents were excluded. First-party integration code and the recorded parser integration patch were reviewed. Generated captured game snapshots, lockfiles, historical evidence, translations, prose documentation and media were classified separately. Relevant documentation, catalogs and article examples were consulted without claiming every content file was source-audited.

## Preserved Distinctions

- EML mod.json, extended.mod.json authoring, their schema evolution, package settings and package locales have separate tasks.
- Public Lua bindings, operation admission, native ABI, provider adapters, native implementations and evidence capture have separate contracts.
- Native mod compatibility adapters differ from immutable game-build profiles. New builds require separate full same-target copies. Old profiles are never overwritten, renamed, normalized, migrated or deleted.
- Offline Modloader Wiki articles and published website pages have separate sources, metadata, discovery and builds.
- Mod actions, native UI host commands, World Editor actions, diagnostics jobs and updater migrations use their existing distinct dispatch/lifecycle owners.
- System check coordination, metadata/checksum requests, mod provider downloads, installation transactions and worker lifecycle retain their separate owners.

## Checks and Limits

The official skill-creator quick_validate.py passed for all 52 installed skills. Names are unique. Repository path checks, relative Markdown references, English-prose checks and stale-root-reference checks passed. The staged diff was checked for whitespace. PyYAML used by the official validator was isolated under generated build output, without changing system Python.

No product test suite, native/product/website build, client/server launch, mutation, scanner upload or deployment was run for this documentation task. Verification commands in skills describe future task checks and are not execution claims.

The audit also identified existing independent verification boundaries, including the link SVG glob/layout mismatch, manifest playground ZIPs without a Lua entrypoint, and capture/system-dependency producer-consumer shape differences. No product fixes were made as part of creating instructions.

This dated audit is evidence, not a replacement ownership map or a second skill registration system.
