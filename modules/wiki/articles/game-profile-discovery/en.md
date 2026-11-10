# Automatic beta game profiles

Optional beta discovery compares a new game build with a saved, supported version. It creates a complete beta profile only when the required evidence is available. Matching regular game profiles take priority.

## Prepare before a game update

The module is enabled by default and can be switched off. An explicitly saved disabled setting stays disabled. Its switch is under **Settings → General → Game profile**, beside the profile selector.

1. Check that **Dynamic game profile creation (Beta)** is enabled. Switch it on if needed.
2. Fully restart the game or dedicated server while a matching regular profile is still available.
3. ShroudForge verifies the exact current executable and saves it as a comparison baseline in the cache. You do not need to download a historical executable.
4. After a later game update, the new game process starts the comparison automatically and uses the saved baseline.

The client and dedicated server keep separate baselines. A client cannot replace a server baseline. If the module is first enabled after an update and no matching baseline was saved, it cannot reconstruct the previous version.

## What happens at game startup

Analysis runs during game startup. The game waits for the result before continuing startup. No loaded world is needed for this step.

ShroudForge first looks for a regular profile matching the exact running executable. Automatic selection uses that profile when one exists. With discovery enabled, it also caches the supported executable for future updates.

If no matching regular profile exists, the module checks the latest suitable regular baseline for the same process target and KFC format. The previously cached executable must match that baseline exactly. It compares code, references, registered types and the required structure contracts from the comparison images. Target addresses must be established from the new image.

The result preserves the complete previous profile contract, including its feature flags. Unresolved or conflicting evidence prevents publication of the entire beta profile. Diagnostics state the reason. Waiting longer or entering a world does not resolve missing static evidence.

## Storage and selection

Regular and generated profiles share the configured profile directory. The default paths are:

| Content | Location |
| --- | --- |
| Client profiles | `shroudforge/runtime/profiles/enshrouded/client/` |
| Server profiles | `shroudforge/runtime/profiles/enshrouded/server/` |
| Saved comparison images and evidence | `shroudforge/cache/game-profile-discovery/` |

Generated profiles start with `beta_`. **Automatic** selection can use a completely verified matching beta profile when no matching regular profile is available. The profile selector lists available profiles. A matching regular profile added later takes priority.

Changing the switch or profile selection requires a full restart of the affected game or server. Reopening the Modloader window does not restart the runtime.

## Distinguish discovery, profiles and a running world

The blue **Dynamic profile search in progress** notice describes an active profile search. **Workflow and help** opens this guide. If the search ends without a matching profile, the problem is shown in red. The orange **Experimental game profile active** notice identifies the beta profile actually selected. Actual runtime errors continue to be displayed as errors.

Structural executable checks do not require a loaded world. World operations additionally need their live contexts, registrations and runtime safety checks. A generated profile does not establish that game commands were executed or produced visible effects. Functional checks must also test the client and server separately.

If no profile is generated, read the reported reason and technical details. A missing saved baseline, unsuitable source profile or unresolved code and layout evidence each requires the relevant input or further analysis. A matching regular profile or a suitable ShroudForge update may provide support.

## Detailed logging

The Debug Console shows discovery messages from the main log under the `game-profile-discovery` source. To see every comparison and rebase field, set **Settings → General → Logging → Minimum level** to **Diagnostic**. **Advanced** shows additional summaries.

The red runtime banner and the log entry `[compatibility-check] check=runtime` come from the same runtime check. The existing UI refresh logs the state change before sending the same status to the Modloader. The Debug Console applies its existing popup setting to warning and error lines from that main log.

On failure, the main log gives the path to the current `*.v2.unresolved.json` or `*.v2.error.json` report. On success, it gives the path to the `*.v2.proof.json` evidence and the full save path of the generated `beta_*.json` profile. Reports are stored in the discovery cache.

## Linux and Proton

Discovery runs inside the native runtime module and requires no external PowerShell or Python programs. This workflow has not yet been live-tested under Proton.

For runtime, mod API and parser guidance, see [Engine](#wiki/engine).
