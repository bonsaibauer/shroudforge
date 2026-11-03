# Integrate the KFC parser

`kfc-parser/` is the Git submodule itself. Rust consumers use its crates through local Cargo paths. There is no additional parser wrapper.

Format access and resource resolution live in `crates/kfc-base`. Existing codecs live in `crates/kfc-content` and `crates/kfc-resource`. `crates/kfc/src/schema.rs` and `snapshot.rs` contain schema projection and snapshot export. The submodule is pinned to the commit behind the current `sf-v0.2.0` tag, `db1580a98dcd679cb9f309b8e26fac97f0734de4`. Git submodules store commit IDs, so the parent repository records that commit rather than the tag name. It is a ShroudForge commit based on the integrated `2cf45e36` revision and includes the parser and loader bridge, the `mod-loader-bridge` crate, and the DirectInput proxy routing used by the DLL Manager.

After a fresh checkout, run `dev/parser/prepare_parser.ps1`. Both `dev/build/dev_build.ps1` and `dev/build/release_build.ps1` perform this preparation step too. The script accepts the current `db1580a9` pin and the previous integrated revisions `2cf45e36` and `742b1cb7` without applying a patch. For the legacy revision `34a156f6`, it retains support for `dev/parser/parser-integration.patch` and detects changes that have already been applied.

Game-file selection belongs to `modules/entrypoints/loader/game_files.rs`. Original files, parser state, and the persistent file transaction belong to `modules/game-files/`. The selected mod configuration fingerprint is calculated in `modules/modloader/calculate_preparation_fingerprint.rs`. Live game access and the native ABI belong to `modules/game-runtime/`.

The workspace is defined in `Cargo.toml` and the release composition is in `dev/build/release_build.ps1`. This integration does not introduce new parser or mod formats.
