--- @meta

--- The execution-facing part of the single ShroudForge API.
--- Its shape is identical before and inside the game; `phase` and `has()` report availability.
--- @class RuntimeApi
--- @field phase "pregame"|"ingame"
--- @field is_client boolean
--- @field is_server boolean
--- Report observed status from a runtime mod's ECS work. A confirmed write reports a typed-memory write, not an independently verified gameplay effect.
--- @field report_effect fun(state:'waiting'|'no-target'|'no-change'|'write-confirmed'|'write-failed', detail:string?):boolean,string?

--- @class RuntimeLifecycle
--- @field on_load fun()?
--- @field on_update fun(delta_seconds:number)?
--- @field on_unload fun()?
--- @field update_interval_ms integer? Minimum 8, maximum 1000, default 50. Missed intervals are not replayed.
runtime = {}

--- @param mod_id string
--- @return boolean
function runtime.has_mod(mod_id) end

--- Known features: `game.assets.write`, `export`, `runtime.lifecycle`,
--- `runtime.ecs.query`, `runtime.ecs.resolve`, `runtime.ecs.read`, `runtime.ecs.write`.
--- @param feature string
--- @return boolean
function runtime.has(feature) end

--- @param feature string
function runtime.require(feature) end

--- @class RuntimeFeatureStatus
--- @field available boolean
--- @field reason string?

--- Returns the availability and, if unavailable, the reason for a runtime operation.
--- @param feature string
--- @return RuntimeFeatureStatus
function runtime.status(feature) end

--- @class RuntimeEcsApi
runtime.ecs = {}

--- Return every type that is registered as a component in the live Keen ECS.
--- The returned Type objects expose their exact names, sizes and fields through `game.types`.
--- Reflection-only ECS helpers, enums and resources are intentionally not included.
--- @return Type[]? components
--- @return string? reason
function runtime.ecs.get_components() end

--- Query live entities by real `keen::ecs::*` component type names or Type objects.
--- Returns `nil, reason` until the current game build has a verified ECS provider.
--- @param ... string|Type
--- @return integer[]? entities Opaque, generation-checked ShroudForge entity handles.
--- @return string? reason
function runtime.ecs.query(...) end

--- Resolve a real `keen::EntityId.id` obtained from a reflected component to
--- the current generation-checked ShroudForge handle. The id is looked up in
--- the live entity manager; it is never treated as a pointer or cached handle.
--- @param keen_entity_id integer
--- @return integer? entity
--- @return string? reason
function runtime.ecs.resolve(keen_entity_id) end

--- Read one live ECS component by an opaque entity handle and real `keen::ecs::*` type.
--- Returns `nil, reason` until the current game build has a verified ECS provider.
--- @param entity integer
--- @param component string|Type
--- @return table? value
--- @return string? reason
function runtime.ecs.read(entity, component) end

--- Write changed reflected fields from a value previously returned by `read`.
--- Padding and concurrently updated fields are retained; the write is verified
--- by readback and restored on failure.
--- Returns `false, reason` until the current game build has a verified ECS provider.
--- @param entity integer
--- @param component string|Type
--- @param value table
--- @return boolean ok
--- @return string? reason
function runtime.ecs.write(entity, component, value) end

--- @class ShroudForgeLogApi
local log = {}
--- @param ... unknown
function log.debug(...) end
--- @param ... unknown
function log.info(...) end
--- @param ... unknown
function log.warn(...) end
--- @param ... unknown
function log.error(...) end

--- @class ShroudForgeApi
--- @field version string
--- @field mod_id string
--- @field mod_kind "lua"
--- @field log ShroudForgeLogApi
shroudforge = {}
