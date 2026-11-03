---
name: add-lua-api
description: Add or change a public ShroudForge Lua function, including its real registration, versioned declarations, API documentation, and meaningful checks. Use for Lua surface changes in modules/mod-runtime.
---

# Add a Lua API function

Read `.agents/skills/repository/repository-working-rules.md` and `docs/development/code-ownership.en.md`. Establish the requested public name, phase, arguments/results, capability behavior, and caller before editing. Preserve existing namespace and EML compatibility.

## Find the actual owner

`modules/mod-runtime/lua/mod_runner.rs::setup_mod` calls `crate::env::register`, implemented by `lua/bindings/mod.rs::register`. Follow the relevant namespace factory rather than adding another registration list.

- `shroudforge.*`: `lua/shroudforge/v1/create_mod_api.rs` and its action owners.
- `runtime.*`: `lua/ingame/mod.rs` and the existing feature module.
- EML `loader.*`: `lua/eml/v1/create_loader_api.rs`.
- `game.*`, buffers, images, IO, hashes: the matching `lua/bindings/**` owner.
- Network operations: `modules/transport/lua/network.rs`, with the supplied runtime context.

Read the complete touched implementation and its actual callers, declarations, and test entrypoint. Reuse `lua/bindings/register_functions.rs::{add_function,add_function_with_mod}` where that namespace uses them. Preserve the existing shared namespace tables and `builtin` aliases.

## Implement the public behavior

1. Put domain behavior in its canonical owner. A binding converts/validates Lua input and calls that owner. Reuse existing checked native bridges for engine work.
2. Match neighboring argument, error, result, session, and capability conventions. Preserve phase restrictions, lifecycle ownership, and cleanup on failure. Do not expose raw addresses or infer a calling ABI from a function catalog entry.
3. If the request adds a runtime operation identifier or changes availability, also follow `extend-runtime-operation-contract`. A registered closure alone does not declare or admit a native operation.
4. Add or update the declaration in `lua/shroudforge/v1/definitions/` or `lua/eml/v1/definitions/`. Keep argument/return annotations, errors, and a valid example consistent with the implementation. Existing website parsing expects one-line `function namespace.name(args) end` declarations.

## Preserve generated consumers

`modules/mod-runtime/build.rs` automatically concatenates Lua files under `definitions`. `lua/definitions/export_lua_definitions.rs` writes cache editor definitions. `docs/website/tools/build-website-output.mjs` builds website API data from those same source declarations. Edit the source declaration rather than generated `build/website/data/api.json` or copied editor snapshots. Executable Lua helpers belong outside declaration directories.

## Verify

Use an existing relevant fixture or add a meaningful contract test when behavior warrants it. `modules/mod-runtime/Cargo.toml` has `autotests = false`, so new integration targets require explicit registration. Run `cargo test -p shroudforge-api` and `./dev/build/check_website.ps1` for declaration changes. Build the affected product when compilation/packaging requires it. For native behavior, perform the requested exact client/server live check and report actual return/effect evidence separately from test success. Do not claim gameplay verification from compilation or availability metadata.

Report the public symbol, canonical owner and caller, declaration/documentation changes, checks performed, and any unverified native behavior.
