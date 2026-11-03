---
name: export-nexus-mod-catalog
description: Export Enshrouded Nexus mod archive preview inventories with the existing metadata-only developer action. Use for inspecting available package file layouts without downloading or installing mod archives.
---

# Export Nexus Mod Archive Metadata

Read `.agents/skills/repository/repository-working-rules.md` and the complete `dev/export_enshrouded_mod_catalog.ps1`. Determine the requested category, bounded/full scope and output path. Reuse this concrete developer action rather than adding a second catalog crawler or general job framework.

The script uses NEXUS_API_KEY from the environment, GraphQL v2 to list Enshrouded mods, REST v1 file metadata for current MAIN/UPDATE/OPTIONAL/MISCELLANEOUS files, then metadata CDN previews for archive paths. It writes a timestamped tree report under build/nexusmods by default. It does not download mod archives. Preserve pagination, PageSize bounds, delay and MaxMods behavior, and report whether the export is partial.

Keep API headers limited to API endpoints. Preview requests intentionally omit the API key and accept only the recognized HTTPS metadata hosts. Preserve conversion of legacy preview links, traversal/absolute-path rejection and per-file preview-error reporting. Never expose the key in command text, output, logs or generated artifacts.

Run the existing script with the requested parameters when the credential is already configured. If a required credential is absent, report that precise blocker without asking the user to paste it into chat. Inspect the report header, mod/file counts, category selection, preview failures and sampled tree paths. Report metadata coverage and exact output path. Do not treat these previews as proof of package compatibility or live runtime behavior.
