---
name: create-profile-server
description: Create a new Enshrouded dedicated server runtime profile from a suitable server profile, verify the exact executable image, and check the build during server startup and in server logs. Use for server profiles under modules/game-runtime/profiles/enshrouded/server/.
---

# Create an Enshrouded Server Profile

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

First read the shared [profile workflow](../profile-workflow.md). Apply it to the dedicated server target.

- The target is exclusively enshrouded_server.exe.
- The profile directory is modules/game-runtime/profiles/enshrouded/server/.
- Historical profiles are immutable. Copy the same-target baseline into a new file for the new build ID. Never edit, overwrite, rename, or delete an old profile. Record and compare its SHA-256 before and after the work.
- Use only an existing server profile as the baseline. Preserve all of its fields and behavior flags. Change only the image identity and individually verified build-dependent values.
- Capture with dev/profiles/capture/capture_runtime.ps1 -Target server. Exactly one process from the selected server directory must be running for live capture. Use -Offline only for static executable analysis.
- Start the dedicated server through its normal entry point with the captured server directory as its working directory. Check the exact process path first and avoid starting a duplicate.
- Check shroudforge-server.log, logs/enshrouded_server.log, loaded runtime DLLs, profile status, local authority, layout/gate status, and Steam/server startup status. The dedicated server skips interactive debug windows and writes to files.
- Verify server-specific ownership chains, scene/thread registration, callbacks, and guards against the server image. Do not reuse client vtables, client callback RVAs, or client context assumptions.
- For stale KFC backups, use the recovery flow already present at startup and preserve evidence of its result from the logs. Do not manually remove any .kfc.bak file.
- Test mutating server operations only in a disposable world or when the user has explicitly requested it. A running server and a ready layout do not prove that a mutating operation works.

Report separately whether the server process is running, whether this exact profile was selected, which runtime gates are ready, and which server operations actually ran. Record log paths, PID, and rollback file as described in the shared profile workflow.
