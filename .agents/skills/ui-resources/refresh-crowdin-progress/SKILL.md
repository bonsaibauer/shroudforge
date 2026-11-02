---
name: refresh-crowdin-progress
description: Refresh or diagnose the generated Crowdin translation progress snapshot consumed by the shared UI build. Use for progress metadata and branch/locale matching, not translating catalog values or registering a locale.
---

# Refresh Crowdin Progress Metadata

Read `.agents/skills/repository/repository-working-rules.md`, `dev/build/capture_crowdin_progress.mjs`, `modules/modloader/ui/package.json`, `dev/build/build_modloader_frontend.ps1`, `modules/modloader/ui/vite.config.ts` and `modules/notifications/ui/show-crowdin-progress.tsx`. The source locale catalog is `modules/ui-resources/locales/en-GB.json`; the generated progress artifact is `build/ui/modloader-inputs/crowdin-progress.json`.

Use the existing capture/build action. It reads CROWDIN_PERSONAL_TOKEN, CROWDIN_PROJECT_ID and CROWDIN_BRANCH from the environment, with current defaults and VERSION fallback. Do not print the token or hardcode it in files. A missing token intentionally produces unavailable progress entries for every local target locale, not fabricated zero-percent progress.

Preserve exact requested branch selection and latest earlier semantic-version fallback. No suitable branch produces unavailable entries. Branch fallback must be reported. Preserve paginated requests and language matching by distinct Crowdin identifiers/filename codes. Do not match the generic English language through a locale fallback that collides with the separate en-US target. Duplicate mappings are errors.

Inspect schemaVersion, actual branch, updatedAt, local target-locale coverage, null/unavailable entries, normalized percentages and word totals. These are build progress metadata, not translation text or generated flags. Do not edit generated output manually or add another locale registry. Use `add-ui-translation` for catalog changes.

For code changes, verify controlled no-token, missing/exact/fallback branch and duplicate-language cases. For a requested refresh, run the canonical action and inspect the snapshot and relevant UI build result. Report whether the Crowdin API was contacted and which branch actually supplied the data.
