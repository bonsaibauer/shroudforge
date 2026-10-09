--- @meta

--- The execution-facing part of the single ShroudForge API.
--- Its shape is identical before and inside the game; `phase` and `has()` report availability.
--- @class RuntimeApi
--- @field schema_version integer
--- @field types TypeRegistry Same registry object as game.types; includes all reflection entries.
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

--- @class RuntimeValuesApi
runtime.values = {}
--- Decode an owned KFC byte string. This does not dereference engine pointers.
---@param type TypeSelector
---@param bytes string
---@return any? value
---@return string? reason
function runtime.values.decode(type, bytes) end
--- Validate and serialize a Lua value using the current type layout.
---@param type TypeSelector
---@param value any
---@return string? bytes
---@return string? reason
function runtime.values.encode(type, value) end
--- Decode the exact reflected default. Never invent zero defaults when none are present.
--- DS containers requiring engine allocation return nil with an explicit reason.
---@param type TypeSelector
---@return any? value
---@return string? reason
function runtime.values.new(type) end

runtime.functions = {}
---@class RuntimeNativeBinding
---@field id string native:<decimal RVA>
---@field name string Original engine descriptor name when unique; otherwise unclear_<hex RVA>.
---@field name_provisional boolean False only for an original engine name.
---@field engine_descriptors table[] Original names and raw dependency-slot offsets from the executable.
---@field modifiers table[] Checked interventions attached to their owning native function.
---@field operations table[] Existing checked runtime wrappers attached to the same function.
---@field key string Executable SHA256/name; rejects use with another executable.
---@field rva integer
---@field provisional boolean
---@field callable boolean True only when this descriptor has a verified owned-buffer adapter.
---@field native_callable boolean Raw engine calls are not exposed by provisional bindings.
---@field execution string? Adapter implementation, when available.
---@field owners table[] Registration names/hashes, slot offsets and pointer origins.
---@field signature table Known adapter contract plus explicit unresolved native contract.
---@field validation table Separate address, type ownership, signature, context and effect evidence.
---@field buffer_transform table? Minimum source/destination lengths, exact copies, optional constant result.
---@field reason string?
---@field call fun(source_bytes:string,destination_bytes:string):string?,string?,integer? Uses owned bytes; unknown bindings return nil,reason.

--- Pages of all observed native code candidates: unwind groups, image code pointers,
--- and live registration callbacks, including provisional/uncallable entries.
--- Code pointers can target internal labels. Unreferenced leaf/inlined code may be absent.
---@param offset integer? Zero-based, default 0.
---@param limit integer? 1..4096, default 256.
---@return table? page Descriptors contain evidence; use get() to obtain a binding with call().
---@return string? reason
function runtime.functions.list_native(offset, limit) end
--- Alias of list_native.
---@param offset integer?
---@param limit integer?
---@return table? page
---@return string? reason
function runtime.functions.list(offset, limit) end
---@param selector integer|string Original engine name, checked operation ID, RVA, native:<decimal>, unclear_<hex>, or SHA256/name.
---@return RuntimeNativeBinding? binding Includes unresolved bindings and their explicit failure reason.
---@return string? reason
function runtime.functions.get(selector) end
--- Native descriptor/owned-buffer adapter only; does not attach typed world-operation wrappers.
---@param rva integer|string
---@return RuntimeNativeBinding? binding
---@return string? reason
function runtime.functions.get_native(rva) end
--- Return a checked operation wrapper or a proven owned-buffer adapter.
--- Use get() to inspect incomplete bindings; bind() returns nil,reason for those.
---@param operation string Operation ID or provisional binding name/key.
---@return function? binding
---@return string? reason
function runtime.functions.bind(operation) end
---@return table<string, RuntimeFeatureStatus>
function runtime.functions.get_operations() end

---@class RuntimeFunctionModifier
---@field id string ShroudForge intervention ID, not an original engine name.
---@field owner RuntimeNativeBinding Exact owning function, original name when evidenced.
---@field effect string Instruction-level effect and scope.
---@field scope string Shared helpers affect all callers of the code site.
---@field callers table[] Static call paths to named engine systems; not a complete dynamic call graph.
---@field available fun():boolean Rechecks the existing capability and native byte guards.
---@field set_enabled fun(enabled:boolean):boolean,string? Same checked backend as legacy runtime.patch.
--- Resolve an intervention through its verified function and current KFC attribute definitions.
---@param id string
---@return RuntimeFunctionModifier? modifier
---@return string? reason
function runtime.functions.bind_modifier(id) end

--- Known features: `game.assets.write`, `export`, `runtime.lifecycle`, ECS operations,
--- and build-verified `runtime.world.*` operations.
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

--- Enumerate declared runtime operations and current phase, mod and provider availability.
--- Unknown native candidates in development catalogs are not executable operations.
--- @return table<string, RuntimeFeatureStatus>
function runtime.get_operations() end

--- @class RuntimeEcsApi
runtime.ecs = {}

---@class RuntimeComponentRegistration
---@field index integer Engine registration index, distinct from Type.index.
---@field qualified_name string Original engine registration name.
---@field runtime_type Type? Actual entity storage layout, possibly Dynamic*.
---@field template_type Type? Configuration layout, separate from entity bytes.
---@field runtime_size integer Zero for template-only registrations.
---@field storage 'entity'|'template-only'
---@field flags_bits integer Raw engine flags, semantics not assumed.
---@field storage_flags_bits integer Raw packed flags adjacent to the 16-bit storage size.
---@field callbacks table[] Code ownership evidence with origin/slot_offset/function_rva; resolve via runtime.functions.get(callback.function_rva).
---@field read_available boolean
---@field write_available boolean
---@field partial_value boolean Some nested values require an unsupported native container codec.
---@field value_reason string?
---@field reason string?

--- All registrations from the validated live engine registry, including template-only entries.
--- Does not require a per-build component hash/index profile. Engine hook/layout compatibility is still required.
---@return {schema_version:integer,layout_version:integer,source:string,count:integer,runtime_type_count:integer,entries:RuntimeComponentRegistration[]}? registry
---@return string? reason
function runtime.ecs.get_registry() end

--- Resolve an original registration or its runtime storage type by qualified name or Type.
---@param selector TypeSelector
---@return RuntimeComponentRegistration? component
---@return string? reason
function runtime.ecs.get_component(selector) end

---@class RuntimeAttribute
---@field name string Original KFC debugNames entry.
---@field hash integer Stored engine attribute ID; not a type hash or FNV(name).
---@field root_hash integer
---@field index integer Zero-based element index within the root's storage.
---@field scalar_type string? KFC calculation scalar type.
---@field components table[] Definition GUID, reflected storage scalar type, size and offsets.
---@field structure table Original parent/child/sibling links and calculation words.
---@field calculation_words integer[] Original program words.
---@field calculation table Decoded AttributeOps instructions, attribute references and Push literals.
---@field storage_writable boolean Static layout agreement, independent of current entity/permissions.
---@field storage_reason string? Missing ownership or contradictory layout evidence.
---@field calculation_scalar_type string Effective model scalar; unresolved paths retain the KFC declaration.
---@field calculation_writable boolean Root program has a consistent or sign-independent scalar interpretation.
---@field calculation_reason string? Why a root update is blocked.
---@field write_value_domain string Scalar range restriction, including common integer range for signedness conflicts.
---@return {version:string,count:integer,entries:RuntimeAttribute[]}? attributes
---@return string? reason
function runtime.ecs.get_attributes() end
---@param selector string|integer Original attribute name or stored ID.
---@return RuntimeAttribute? attribute
---@return string? reason
function runtime.ecs.get_attribute(selector) end
---@param entity integer Entity handle from runtime.ecs.query.
---@param selector string|integer
---@return number? value Interpreted according to the reflected storage scalar type.
---@return string? reason
function runtime.ecs.read_attribute(entity, selector) end
--- Write exactly one checked storage element on the existing ECS dispatcher.
--- Does not execute attribute calculation programs, publish events or recalculate dependencies.
--- Conflicting integer signedness permits only the common nonnegative range; incompatible scalar representations reject writes.
---@param entity integer
---@param selector string|integer
---@param value number
---@return boolean ok
---@return string? reason
function runtime.ecs.write_attribute_storage(entity, selector, value) end

---Read the entire related attribute root as {root_hash, scalar_type, values={original_name=value}}.
---@param entity integer
---@param selector string|integer Original attribute name or stored attribute ID.
---@return table? result
---@return string? reason
function runtime.ecs.read_attributes(entity, selector) end

---Evaluate a detached root in native descending order. values must contain every original name in the root.
---Returns named values and a per-program trace with before_bits/after_bits in root index order.
---Does not write game state. Unsupported programs and non-finite values are rejected.
---@param selector string|integer
---@param values table<string, number>
---@return table? result
---@return string? reason
function runtime.ecs.evaluate_attributes(selector, values) end

---Set one attribute and recalculate its complete root. Requires an exact verified executable profile.
---The game-thread write rejects a changed snapshot and checks the entity generation and component layout.
---Applies locally; the engine owns replication. Does not forward client calls to the server.
---Signedness conflicts permit only common nonnegative input values and type-independent calculations.
---@param entity integer
---@param selector string|integer
---@param value number
---@return table? result Updated values and calculation trace, or nil on failure.
---@return string? reason
function runtime.ecs.update_attribute(entity, selector, value) end

--- Return every type that is registered as a component in the live Keen ECS.
--- The returned Type objects expose their exact names, sizes and fields through `game.types`.
--- Reflection-only ECS helpers, enums and resources are intentionally not included.
--- @return Type[]? components
--- @return string? reason
function runtime.ecs.get_components() end

--- List every reflected component candidate, including unresolved types.
--- Availability also depends on the current phase, mod capabilities and provider.
--- A resolved entry does not imply that a matching entity exists or every field is writable.
--- @return table catalog {version, entries={type, qualified_name, size, resolved, read_available, write_available, reason, read_reason, write_reason}[]}
function runtime.ecs.get_catalog() end

--- Query live entities by real `keen::ecs::*` component type names or Type objects.
--- Returns `nil, reason` until the current game build has a verified ECS provider.
--- @param ... string|Type Qualified type name or Type from the current registry.
--- @return integer[]? entities Opaque, generation-checked ShroudForge entity handles.
--- @return string? reason
function runtime.ecs.query(...) end
--- Query live entities whose CurrentTransform pivots overlap bounds expanded by padding * max(abs(scale)).
function runtime.ecs.query_bounds(bounds, padding, ...) end

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
--- @param component string|Type Qualified type name or Type from the current registry.
--- @return table? value
--- @return string? reason
function runtime.ecs.read(entity, component) end
--- Read an owned snapshot of the complete entity storage bytes, including opaque DS descriptors.
--- Does not follow pointers. The same runtime.ecs.read capability and entity checks apply.
---@param entity integer
---@param component TypeSelector
---@return string? bytes
---@return string? reason
function runtime.ecs.read_bytes(entity, component) end

--- Write changed reflected fields from a value previously returned by `read`.
--- Padding and concurrently updated fields are retained; the write is verified
--- by readback and restored on failure.
--- Returns `false, reason` until the current game build has a verified ECS provider.
--- @param entity integer
--- @param component string|Type Qualified type name or Type from the current registry.
--- @param value table
--- @return boolean ok
--- @return string? reason
function runtime.ecs.write(entity, component, value) end

--- @class RuntimeWorldApi
runtime.world = {}
--- @class RuntimeWorldCursorApi
runtime.world.cursor = {}
--- Read the most recent live building cursor captured by the current build's verified native hook.
--- The hook copies the ClientCursor layout before returning to the game and the snapshot is served without an ECS scan.
--- @return table? snapshot with `sequence` and reflected-layout `value.primaryTransform` fields.
--- @return string? reason
function runtime.world.cursor.get() end

--- Ordinary local player building input; availability: runtime.world.building.input.
--- Exact client build only; follows the existing game input path. The server
--- still checks permissions/resources/range. No server mod RPC is created.
runtime.world.building = {}
--- @class BuildingInput
--- @field player integer Process-local ClientPlayerInput entity handle.
--- @field action 'select'|'place'|'remove'|'dismantle'|'undo' `remove` is the secondary building action for voxels; `dismantle` holds the contextual action for props.
--- @field targetEntityId integer? Exact live ECS entity ID required for `dismantle`.
--- @field itemId integer? Required for select; current KFC ItemInfo item ID.
--- @field materialItemId integer? Optional stock cycle material item ID.
--- @field slot integer? Current actionbar slot for select.
--- @field position number[]? Required for place/remove/dismantle, world units.
--- @field rotation number[]? Quaternion x,y,z,w, normalized by the provider.
--- @field scale number[]? Positive x,y,z, defaults to 1,1,1.
--- @param input BuildingInput
--- The first accepted request reserves the shared input sequence for this Lua
--- API instance. Call cancel(latest_id) when the whole sequence ends, including
--- after successful observation, so another mod can use the building tool.
--- @return integer? request_id
--- @return string? reason
function runtime.world.building.submit(input) end
--- Only the latest request owned by this Lua API instance can be queried.
--- 'dispatched' means input emitted, NOT accepted/replicated/persisted.
--- @param request_id integer
--- @return 'unknown'|'queued'|'pressed'|'dispatched'|'timeout'|'cancelled'
function runtime.world.building.status(request_id) end
--- Stops pending input. A pressed action is released on the next input tick;
--- already dispatched changes cannot be undone by cancelling.
--- @param request_id integer
--- @return boolean
function runtime.world.building.cancel(request_id) end

--- @class RuntimeWorldVoxelApi
runtime.world.voxel = {}

--- Return metadata for a native world grid. The active backend currently exposes the `voxel` grid.
--- @param grid_id string Grid identifier, currently "voxel".
--- @return table? spec {id, origin, cellSize, maximum}
--- @return string? reason
function runtime.world.voxel.get_grid_spec(grid_id) end

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
--- Conservative process-local world/ECS lifetime marker; zero when unavailable.
--- Not a persistent world ID or a singleplayer/multiplayer indicator.
---@return integer
function runtime.world.session_id() end

--- @class RuntimeWorldEntityApi
runtime.world.entity = {}

--- Query placeable props using the native, build-profiled ECS layout. Returns
--- transforms, item IDs, and the native entity template UUID directly, without requiring Lua reflection reads.
--- @param bounds number[] World-space min xyz followed by max xyz.
--- @param padding number Conservative pivot margin, scaled by each prop's maximum absolute scale.
--- @return table[]? props
--- @return string? reason
function runtime.world.entity.query_props(bounds, padding) end

--- Register ItemInfo placement AABBs for native prop-bound filtering.
--- @param recipes table[] Entries with itemId, bounds[6], and feedback.
--- @return boolean? registered
--- @return string? reason
function runtime.world.entity.register_prop_recipes(recipes) end

--- Query props whose native, rotated and scaled ItemInfo placement AABBs intersect bounds.
--- Each result includes entityId and templateUuidHighHex/templateUuidLowHex from the live native entity definition.
--- Requires placement recipes registered with register_prop_recipes.
--- @param bounds number[] World-space min xyz followed by max xyz.
--- @return table[]? props
--- @return string? reason
function runtime.world.entity.query_props_in_bounds(bounds) end

--- Resolve the current transform, ECS entity ID, item ID, and native template UUID for an opaque live prop handle.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @return table? prop
--- @return string? reason
function runtime.world.entity.get_transform(entity_handle) end

--- Update a live prop's three scale values through the build-profiled native API.
--- The game-thread write is read back and verified; a failed write is restored when possible.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @param scale number[] Three finite scale values for x, y, and z.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.set_scale(entity_handle, scale) end

--- Spawn a native entity in the live prop-update context and wait for its live ECS record.
--- UUIDs are hexadecimal qwords. Returns the new opaque entity handle, not an engine queue token.
--- @param template_uuid_high_hex string
--- @param template_uuid_low_hex string
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param tracking_id integer
--- @param flags integer
--- @return integer? entity_handle
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

--- Remove the prop identified by its current live handle. The native operation resolves its
--- transform and registered ItemInfo placement recipe, dispatches removal, and verifies that
--- this exact handle disappeared, matching Shroudtopia's WorldApi handle-only contract.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.destroy(entity_handle) end

--- Legacy spatial destroy overload. Prefer the handle overload for editor operations.
--- This operation does not establish save persistence or collision updates.
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
function log.trace(...) end
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
