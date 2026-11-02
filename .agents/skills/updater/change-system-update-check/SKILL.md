---
name: change-system-update-check
description: Change or diagnose automatic/manual system release checks while preserving per-installation coordination, cooldown and persisted results. Use for system update detection and request counts, not mod catalog checks or release installation.
---

# Change System Update Checks

Read `.agents/skills/repository/repository-working-rules.md`, `modules/updater/control/check_due_system_update.rs`, `modules/updater/system-updates/check_lock.rs`, `modules/updater/system-updates/headless_check.rs`, `modules/updater/system-updates/fetch_system_release.rs`, `modules/updater/release-feed/lib.rs`, and all actual callers before editing.

Trace the automatic headless controller and manual host/request paths to the same installation-scoped check coordination. Preserve the automatic cooldown from `updates.checkMinutes` and persisted attempt/check timestamps, concurrent-check exclusion, manual-check behavior and cached result consumed by UI and headless processes. A manual request bypasses the automatic interval but still coordinates with a running check. Do not introduce a separate UI cache or another polling loop.

Count network accesses at the actual call sites. Release metadata, the GitHub SHA-256 asset, archive download, staging and installation are different operations. `release-feed::fetch_latest` performs a release-metadata request and conditionally a checksum-file request when its caller requests one. Read the caller's checksum argument before stating its access count. The daily news metadata fallback has a separate policy and does not justify caching a hash file for the same duration. Installer paths require verified checksum data and must retain that requirement.

Preserve stable/prerelease filtering, release comparison, unavailable-provider/error states and the saved latest-release result. Follow result snapshots in both `modules/modloader/ui/host/read_release_status.rs` and `modules/updater/ui/host/window.rs` where relevant. Use relevant existing updater checks and controlled provider/lock fixtures to verify simultaneous automatic/manual checks, cooldown, failure/retry and cached result visibility. Do not repeatedly query a real endpoint as a substitute for measuring calls.

Report the exact previous and resulting request counts for each affected trigger, the persisted state and the checks executed. Keep mod provider/catalog checks and installation downloads outside the system-check policy unless the requested change explicitly includes them.
