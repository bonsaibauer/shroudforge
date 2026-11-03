---
name: add-mod-download-provider
description: Add a mod catalog or package-download provider through the updater's provider contract and canonical installation pipeline. Use for a new remote provider, not a mod link badge or system release-check policy.
---

# Add a Mod Download Provider

Read `.agents/skills/repository/repository-working-rules.md`, `modules/updater/provider_config.rs`, `modules/updater/mod-installation/catalog.rs`, `download_mod_package.rs`, `verify_mod_checksum.rs`, the mod requests and queue executor, and existing configured provider fields in `modules/loader-config/loader.default.json` and `loader.schema.json`.

Separate catalog search, project/version selection, mod update detection, archive URL resolution and archive download. Implement provider-specific metadata/response mapping inside the updater owner and normalize to the existing catalog item/package structures. Preserve recorded project/mod/version identity and the catalog install registry used for future update checks. Do not infer loader capabilities or package provenance from the remote display name.

Extend the current provider contract, configuration validation, save allowlists and owner UI snapshots only when required. Keep system GitHub-release provider semantics separate. Respect enabled/manual/automatic check coordination already used by catalog checks, existing maximum sizes, cancellation and request timeouts. Measure actual endpoint requests for changes to repeated checks. Never log tokens or forward an API credential to an unrelated CDN host.

Pass downloaded archives through the existing checksum, bounded extraction, package selection, validation, same-volume staging and recoverable commit flow. The provider must not install directly into the live mods directory, bypass `mod.json` validation or add archive reads to normal running mod discovery. Native compatibility import rules are owned by `modules/modloader/mod_profiles.rs`.

Verify with controlled provider responses and temporary directories, including malformed metadata, missing checksum/version, identity mismatch, unsupported package and cancellation. Existing body-download tests in `modules/updater/download_response_body.rs` cover stalled and failed response handling. Use the relevant `shroudforge-updater` checks and `shroudforge-package` reader checks. Explicitly distinguish mock checks from a real authorized provider lookup. Do not download or install into a user's live installation merely to author the provider.
