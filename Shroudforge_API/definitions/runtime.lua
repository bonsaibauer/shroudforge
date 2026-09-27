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

--- Known features: `game.assets.write`, `export`, `runtime.lifecycle`, ECS operations,
--- build-verified `runtime.world.*` operations, and build-profile runtime patches.
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

--- @class RuntimeWorldApi
runtime.world = {}
--- @class RuntimeWorldVoxelApi
runtime.world.voxel = {}

--- Read a bounded, x-fastest voxel region. Each number packs material in the low byte and density in the high byte.
--- @param x integer
--- @param y integer
--- @param z integer
--- @param size_x integer
--- @param size_y integer
--- @param size_z integer
--- @return integer[]? cells
--- @return string? reason
function runtime.world.voxel.read(x, y, z, size_x, size_y, size_z) end

--- Write a bounded, x-fastest voxel region and verify the native readback. Each cell is a packed 16-bit material/density value.
--- @param x integer
--- @param y integer
--- @param z integer
--- @param size_x integer
--- @param size_y integer
--- @param size_z integer
--- @param cells integer[]
--- @return boolean ok
--- @return string? reason
function runtime.world.voxel.write(x, y, z, size_x, size_y, size_z, cells) end

--- Report whether the current build profile resolves this named world operation.
--- @param operation string
--- @return boolean available
function runtime.world.operation_available(operation) end

--- Report whether a validated live voxel-world context is available.
--- @return boolean active
--- @return string? reason
function runtime.world.context_active() end

--- @class RuntimeWorldEntityApi
runtime.world.entity = {}

--- Queue a native entity spawn in the live prop-update context and wait for a matching
--- CurrentTransform + UsedItem record in the live ECS. UUIDs are hexadecimal qwords.
--- The returned integer is still the engine command queue token, not an entity handle.
--- @param template_uuid_high_hex string
--- @param template_uuid_low_hex string
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param tracking_id integer
--- @param flags integer
--- @return integer? queue_token
--- @return string? reason
function runtime.world.entity.spawn(template_uuid_high_hex, template_uuid_low_hex, position, rotation, tracking_id, flags) end

--- Dispatch an engine placement and finish-building call in a validated live actor placement context.
--- A successful return confirms dispatch without independently proving entity materialization, persistence, or collision updates.
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param bounds number[] AABB min xyz followed by max xyz.
--- @param tracking_id integer
--- @param feedback_id integer
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.place(position, rotation, bounds, tracking_id, feedback_id) end

--- Dispatch an engine destroy call by transform and bounds, then wait for a matching CurrentTransform + UsedItem count to decrease.
--- This spatial operation does not identify an exact entity handle and does not establish save persistence or collision updates.
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param bounds number[] AABB min xyz followed by max xyz.
--- @param tracking_id integer
--- @param feedback_id integer Ignored by the native destroy call; retained for the shared place/destroy signature.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.destroy(position, rotation, bounds, tracking_id, feedback_id) end

--- Dispatch the profile's finish-building operation in a validated live actor placement context.
--- @param complete boolean
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.finish_building(complete) end

--- @class RuntimePatchApi
runtime.patch = {}
--- Report whether the active build profile validated a named runtime patch.
--- @param name string Profile operation name such as runtime.patch.no_fall_damage.
--- @return boolean available
function runtime.patch.available(name) end
--- Safely enable or restore a profile-defined runtime patch.
--- @param name string Profile operation name such as runtime.patch.no_fall_damage.
--- @param enabled boolean
--- @return boolean ok
--- @return string? reason
function runtime.patch.set_enabled(name, enabled) end

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
