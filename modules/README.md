# Product modules

Each direct child of `modules/` is a product module with its own behavior and, where needed, its own `ui/` directory. `modules/transport/` is a module too. A window passes work to its owning module and presents the result instead of implementing the same behavior again.

See [code ownership](../docs/development/code-ownership.en.md) for the current repository map and action-to-file guide.

`modules/game-runtime/native/main-menu/` owns the selected-character cache alongside the existing native menu callback. It derives the original selection/name contract from the already resolved menu builder. Names are copied on the first valid selection and on a changed selection or name descriptor. The optional native ABI and `runtime.player.name()` expose that same cache. `modules/player-name/` consumes the existing runtime status publication for the Modloader and applies the configured manual/default-name policy. It does not open the game process, scan memory, start a reader thread, or maintain a persistent name cache.
