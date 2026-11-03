---
name: create-profile-client
description: Create a new Enshrouded client runtime profile from a suitable client profile, verify the exact executable image, and check the build in the normal client process. Use for client profiles under modules/game-runtime/profiles/enshrouded/client/.
---

# Create an Enshrouded Client Profile

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

First read the shared [profile workflow](../profile-workflow.md). Apply it to the client target.

- The target is exclusively enshrouded.exe.
- The profile directory is modules/game-runtime/profiles/enshrouded/client/.
- Historical profiles are immutable. Copy the same-target baseline into a new file for the new build ID. Never edit, overwrite, rename, or delete an old profile. Record and compare its SHA-256 before and after the work.
- Use only an existing client profile as the baseline. Preserve its fields and behavior flags. Change only the image identity and individually verified build-dependent values.
- Capture with dev/profiles/capture/capture_runtime.ps1 -Target client. Exactly one process from the selected client directory must be running for live capture. Use -Offline only for static executable analysis.
- Start the normal client through its existing client entry point or Steam, using the exact captured game directory. Check shroudforge-client.log, the process path, loaded runtime DLLs, the exact profile ID, and live status.
- Verify client-specific hooks and cursor/world contexts against the client image only. Server values are not substitutes.
- Review the changes, build the KFC runtime, and verify the provider ABI and embedded profile as described in the shared workflow.

Report which calls are supported by static evidence, which are ready according to client status, and which were actually called live. Starting the client alone does not confirm that a write took effect.
