# Root AGENTS Rule Migration

Date: 2026-11-02.

The user requested dissolution of the root AGENTS.md into task skills. Each instruction statement was assigned to an existing skill or the short shared repository rules. The root file was removed after verifying every destination. Nested module instructions remain in place.

Reviewed pre-deletion SHA-256: a7b30eb0b5435172aa813ab044e29f98405f44abac8340a43f2728532c9fa309

| Source line | Preserved rule | Skill destination |
| --- | --- | --- |
| 5 | Keep the active agent responsible for the task. | .agents/skills/repository/repository-working-rules.md |
| 7 | Instruction files describe workflows and do not create UI agents or CI workflows. | .agents/skills/repository/repository-working-rules.md |
| 11 | Preserve the starting working tree. | .agents/skills/repository/repository-working-rules.md |
| 12 | Read implementations and trace callers, data, persistence, builds, packages and discovery. | .agents/skills/repository/repository-working-rules.md |
| 13 | Use current ownership documentation and distinguish dated evidence. | .agents/skills/repository/repository-working-rules.md |
| 14 | Separate observed behavior, user intent, proposals and unresolved facts. | .agents/skills/repository/repository-working-rules.md |
| 18 | Justify every new or moved file by action, owner, path, name, callers and existing ownership. | .agents/skills/repository/repository-working-rules.md, .agents/skills/repository/refactor-repository-module/SKILL.md |
| 20 | Keep paired website Markdown, metadata, ordering, automatic discovery and generated output. | .agents/skills/website/add-website-page/SKILL.md |
| 22 | Keep page integrations with their page and use automatic mount-page.js discovery. | .agents/skills/website/add-page-integration/SKILL.md |
| 24 | Keep canonical actions, thin callers and meaningful files without arbitrary splitting. | .agents/skills/repository/repository-working-rules.md, .agents/skills/repository/refactor-repository-module/SKILL.md |
| 26 | Keep feature modules and their UI together, including transport. | .agents/skills/repository/repository-working-rules.md |
| 28 | Respect demonstrated repository boundaries. | .agents/skills/repository/refactor-repository-module/SKILL.md |
| 30 | Keep mod packages directly under mods. | .agents/skills/repository/refactor-repository-module/SKILL.md |
| 31 | Keep parser code/crates in the direct submodule without nested copies or vendoring. | .agents/skills/repository/refactor-repository-module/SKILL.md |
| 32 | Add src levels only when required by the actual build system. | .agents/skills/repository/refactor-repository-module/SKILL.md |
| 33 | Assign native/runtime files from actual responsibilities and callers. | .agents/skills/repository/refactor-repository-module/SKILL.md |
| 34 | Keep development actions concrete and in existing dev entrypoints. | .agents/skills/repository/repository-working-rules.md, .agents/skills/repository/refactor-repository-module/SKILL.md |
| 38 | Historical game-build profiles are immutable and a new build gets a new file. | .agents/skills/runtime/profile-workflow.md, .agents/skills/repository/repository-working-rules.md |
| 40 | Trace configuration readers/writers and preserve the canonical storage contract. | .agents/skills/repository/repository-working-rules.md, .agents/skills/loader-config/add-loader-setting/SKILL.md |
| 41 | Separate EML extension creation from revision-checked configuration persistence. | .agents/skills/modding/create-mod-extension/SKILL.md, .agents/skills/modding/add-mod-setting/SKILL.md |
| 42 | Keep installed mods unpacked and archive extraction in installation. | .agents/skills/modloader/add-native-mod-adapter/SKILL.md, .agents/skills/repository/refactor-repository-module/SKILL.md |
| 43 | Measure actual updater network stages and request counts. | .agents/skills/updater/change-system-update-check/SKILL.md |
| 44 | Preserve automatic/manual check coordination, cooldown and saved result. | .agents/skills/updater/change-system-update-check/SKILL.md |
| 46 | Preserve refactoring contracts, dynamic uses and owner-bound local data. | .agents/skills/repository/refactor-repository-module/SKILL.md, .agents/skills/repository/repository-working-rules.md |
| 50 | Use appropriate existing checks and report actual owners, callers, results and limits. | .agents/skills/repository/repository-working-rules.md |
| 52 | Inventory a complete audit and prove full-content coverage without overclaiming. | .agents/skills/repository/audit-repository-code/SKILL.md |
| 54 | Use direct German umlauts and no semicolons in German prose. | .agents/skills/repository/repository-working-rules.md |

Mapped instruction statements: 27. Unmapped: 0.

This is a dated migration record, not an alternative code ownership map or a skill registration list.
