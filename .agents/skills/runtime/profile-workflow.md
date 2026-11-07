# Shared Enshrouded Runtime Profile Workflow

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

This reference applies to the create-profile-client and create-profile-server skills. Keep the two executable targets separate. A client profile describes only enshrouded.exe. A server profile describes only enshrouded_server.exe.

## Scope and Existing State

- First check git status --short. Preserve existing user changes and profiles.
- Historical game-build profiles are immutable reference files. Never edit, overwrite, rename, delete, or bulk-migrate them, including during schema or runtime changes.
- Record the baseline path and SHA-256 before copying it. Create a separate file named for the new exact build ID in the same target directory. Do not overwrite an existing profile with that ID. Keep task-owned new drafts distinct from historical profiles.
- Recompute the baseline hash after all work and verify it matches the starting hash. Compare existing profile files with the starting working-tree state, preserving any user changes already present. Never restore or normalize old profiles as part of creating a new one.
- Read docs/development/code-ownership.en.md, modules/game-runtime/profiles/README.md, dev/profiles/README.md, and docs/development/inspect-game-build-and-create-runtime-profile.de.md as they apply to the specific profile type.
- Determine the exact target path, existing profiles for the same target, and the actual game directory. Do not use a client profile as the server baseline or vice versa.
- Use the best-matching profile for the same target as the baseline and copy it in full. Change only the build identity and addresses or offsets individually verified for the new image. Preserve every section, flag, approval, and call path from the baseline.
- Then compare both JSON files leaf by leaf. Explain every difference as an identity field or a proven build-dependent value. Do not add or remove a feature or silently change an approval.

## Image and Address Verification

- Determine the SHA-256, PE timestamp, and SizeOfImage for the correct executable. Check the target name and KFC format independently. Follow the existing build ID scheme in the target directory for the profile name.
- Verify every RVA and offset against this exact executable. Distinguish RVAs relative to the image from byte offsets within structures.
- Use the existing build scanners, disassembly, unique function fingerprints, references, and byte guards. A match from an older build or the other target is only a search hint. It does not prove the value for the new build.
- Check function prologues, calling conventions, guard bytes, and ambiguity. Finding a function alone does not authorize an engine call.
- Verify the exact image identity again after editing the file. A profile without a unique target identity or with ambiguous addresses is incomplete.

## Approvals and Profile Classes

- Preserve the existing provenance.status. read-validated and approved have different runtime meanings.
- Never set a status to approved manually. Use only the existing approval workflow, which requires a matching function scan, exact live capture, component evidence, zero unresolved mappings, and semantic checks.
- Use the validator for the actual profile format. The CLI command validate-profile may expect a different schema from a read-validated resource profile. Check the schema and readers before interpreting a failure or success.
- The shared external JSON catalog classifies read-validated profiles separately from approved execution profiles. Check the installed catalog path, exact image selection, and profile ID. Do not claim execution hooks that the profile does not contain. There is no embedded profile catalog or embedded fallback.

## Existing Tools and Build

- Use the existing tools under dev/profiles/. The current command reference is in dev/profiles/README.md and kfc-runtime-dev.exe --help.
- Inspect a generator before using it for a 1:1 copy. The existing create_runtime_profile_draft.cpp deliberately removes write layouts and resets draft gates. Do not run that destructive draft transformation as a substitute for copying the complete baseline.
- `game_inspection/compare/prepare_profile_rebase.py` prepares a complete same-target candidate outside production profiles with an exact-image leaf diff and unresolved checks. Static rankings never change its numeric values. Copied provenance remains baseline evidence, not target approval. Its optional numeric review plan must bind both images and hashed concrete evidence.
- For capture_runtime.ps1, explicitly choose -Target client or -Target server. Use -Offline only for executable analysis without a live process. Do not claim that this is a live check.
- Explicitly build any required inspection tools before capture. capture_runtime.ps1 does not build programs in the background, and live capture expects exactly one matching process to be running.
- Put capture output in a task-specific temporary directory using -OutputDirectory. Do not write investigation output to saves, settings, or profiles without being asked.
- Install the validated new JSON profile into the existing configured `runtime/profiles` catalog and verify the installed file hash and selected profile ID. Data-only profile changes do not require a provider rebuild. When native code changes, build with dev/build/build_kfc_runtime.ps1, inspect the provider with kfc-runtime-dev.exe inspect-provider, and compare its ABI and required exports with the installed bridge.
- Normal profiles must always win over `beta_*` files and IDs. The optional native `modules/game-profile-discovery/` component owns automatic experimental generation. Keep its exact cached-image and mapped-image-equivalence proof separate from developer ranking and live gameplay checks. Do not bypass its unresolved result by turning a heuristic candidate into a runtime profile.

## Live Verification

- Start only the explicitly requested target, and only if exactly one matching process is not already running. Use the normal entry point for that client or server with its game directory as the working directory. Do not start a duplicate.
- If the build provider must be placed in the installation directory for a live attempt, replace only the file required for that attempt. First preserve the previous file with a unique name and record its hash. Verify the installed hash. Do not update other runtime, proxy, updater, or configuration files without a separate request.
- Start background processes hidden. Use elevated execution only for the specifically authorized file operation or requested launch outside the sandbox.
- Read the target-specific ShroudForge log and the game server log. Default paths are shroudforge/logs/shroudforge-client.log and shroudforge/logs/shroudforge-server.log. Follow any redirected storage paths in the existing configuration. Do not clear or rotate logs yourself.
- Confirm process path and PID, loaded bootstrap and runtime DLLs, exact image identity, profile status, runtime readiness, and the last bootstrap phase. A successful process start or an opened menu alone does not prove that every profile call works.
- Interpret status messages according to the actual control path. Track separately whether the exact profile was selected, whether the layout and gate were configured, and whether a specific call was ready live.
- For incompatible KFC working copies, use the existing asset recovery flow. Do not manually delete or move .kfc.bak files.
- Do not run a mutating ECS, resource, or world operation in a real save unless the user requested that specific live change. Use a disposable world or save to verify effects.
- Do not change log levels or other user settings automatically. If existing logging omits a required status, identify the missing observation and use an existing status or diagnostic path first.

## Final Report

Name the new profile and immutable baseline paths, matching before/after baseline hashes, image hash, changed fields that were checked, build and ABI status, and static and live checks performed. List failed and unrun checks separately. Report the PID, log paths, and rollback file if a process was started or a provider was installed. Do not claim live calls when only the addresses and profile structure were verified.
