# Game assets, ECS, and reflected data types

The Lua API has several areas with different timing and access behavior. The [API reference](#api) lists all functions and types.

## Prepare assets

`game.assets` reads and changes supported game resources during preparation. Changes are prepared for a later game start. They do not update an already running world immediately. Declare the matching `patch` or `export` capability and check resource types available in the active profile.

## Runtime and ECS

`runtime.ecs` works with supported components while the game is running. The loader checks the profile, types, and execution phase. Use documented query and write functions and inspect their result status. A confirmed memory operation is not independent proof of the visible gameplay effect.

## Types and values

`game.types` and `runtime.types` expose reflected type information from the active game profile. `runtime.values` can read or create owned byte strings according to a known type. Dereferencing arbitrary engine pointers is not part of this safe value access.

## Why game updates matter

Available types and operations depend on the installed Enshrouded build and profile. A mod can be present and loaded while one native access is unsupported. Check `runtime.has(feature)` and the status of the specific function.
