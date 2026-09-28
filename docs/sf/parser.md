# ShroudForge Parser

The `src/parser/` crate converts Enshrouded data into snapshots consumed by the
loader API and compatibility modules. It also owns parser status and the KFC
asset transaction adapter.

The upstream KFC parser remains a pinned submodule directly inside this module
at `src/parser/kfc-parser/`. ShroudForge parser changes belong alongside it in
`src/parser/`; the upstream source is kept unchanged.

Cargo workspace membership and the root build are defined in `Cargo.toml` and
`build.ps1`.
