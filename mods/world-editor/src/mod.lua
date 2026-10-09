-- The same editor module runs headless on the dedicated server. UI actions and
-- cursor input stay client-side; P2P requests enter the existing world API here.
if type(runtime) ~= "table" then return {} end
runtime.require("runtime.lifecycle")
local editor_is_server = runtime.is_server == true
local editor_ui_available = not editor_is_server and type(shroudforge) == "table" and type(shroudforge.ui) == "table"
if type(shroudforge) ~= "table" or (not editor_is_server and not editor_ui_available) then return {} end

local remote_setting_override = nil
local function setting(name)
    if remote_setting_override and remote_setting_override[name] ~= nil then
        return remote_setting_override[name]
    end
    local value = shroudforge.settings.get(name)
    if value == nil then return "" end
    return value
end

local clipboard = nil
local placement_preview = nil
local undo_state = nil
local feature
local selection_a = nil
local selection_b = nil
local selection_target = nil
local active_blueprint_name = nil
local live_prop_cache = {}
local placeable_items = nil
local prop_recipes_registered = false
local pending_cursor_action = nil
local pending_prop_capture = nil
local pending_world_action = nil
local readiness_logged = false
local readiness_wait_logged = false
local maximum_blueprint_bytes = 256 * 1024 * 1024
local maximum_supported_props = 1000000
local default_maximum_copyable_props = 60000
local save_blueprint_named
local undo_voxels
local rollback_partial_paste
local refresh_blueprint_library
local publish_editor_state
local set_editor_message
local previous_key_state = {}
local observed_rotation_setting = nil
local active_rotation_turns = nil
local voxel_grid_spec = nil
local editor_stage = "need_a"
local editor_hint = "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press F5."
local last_published_editor_state = nil
local last_execution_backend = nil
local p2p_bridge = nil
local remote_undo = nil
local remote_request_pending = false
local server_remote_paste = nil
local server_remote_transaction = nil
local server_remote_undo = nil
local paste_remote
local undo_remote
local observed_session = nil
local retired_sessions = {}
local world_wait_timeout = 15
local function current_session()
    return runtime.world and runtime.world.session_id and runtime.world.session_id() or 0
end
local function execution_backend()
    if editor_is_server then return "direct" end
    -- Route only from the native, build-profiled context source. The client EXE
    -- can be local or joined to a dedicated server, so process role and general
    -- runtime readiness are not sufficient evidence for selecting a writer.
    if not runtime.world or not runtime.world.context_kind then return "unknown" end
    local session = current_session()
    if not session or session == 0 then return "unknown" end
    local kind = runtime.world.context_kind()
    if kind == "direct" then return "direct" end
    if kind == "client-read-only" then return "p2p" end
    return "unknown"
end

local function execution_backend_label(backend)
    if backend == "direct" then return "native-direct" end
    if backend == "p2p" then return "server-p2p" end
    return "detecting"
end

local function require_execution_backend(backend, operation)
    if backend ~= "unknown" then return true end
    local message = "Automatic world target detection is waiting for a validated world session. " ..
        tostring(operation or "No world change was sent.")
    set_editor_message(nil, message)
    shroudforge.log.warn("World Editor blocked " .. tostring(operation or "world change") ..
        ": native world target is unknown; no local write or P2P request was sent")
    return false
end
-- Overall save milestones, not a prop count or an estimated byte percentage.
local save_progress = {state = "idle", completed = 0, phase = "", sequence = 0}
local function save_step(completed, phase, state)
    if completed == 0 then save_progress.sequence = save_progress.sequence + 1 end
    save_progress.completed, save_progress.phase = completed, phase
    save_progress.state = state or "saving"
    if publish_editor_state then publish_editor_state() end
end
local function finish_save_progress()
    if refresh_blueprint_library and refresh_blueprint_library() then
        save_step(4, "Blueprint saved", "complete")
    else
        save_step(3, "Saved; library refresh failed", "error")
    end
end

local function key_pressed(key)
    local down = shroudforge.input.is_key_down(key)
    local pressed = down and not previous_key_state[key]
    previous_key_state[key] = down
    return pressed
end

local function rotation_turns()
    local configured = math.floor(tonumber(setting("rotationQuarterTurns")) or 0) % 4
    if active_rotation_turns == nil or configured ~= observed_rotation_setting then
        active_rotation_turns = configured
        observed_rotation_setting = configured
    end
    return active_rotation_turns
end

local function rotate_blueprint()
    if not clipboard then
        set_editor_message("need_a", "No blueprint is active. Press F5 to start a capture or select a saved blueprint.")
        shroudforge.log.warn("World Editor: capture or load a blueprint before rotating it")
        return
    end
    active_rotation_turns = (rotation_turns() + 1) % 4
    placement_preview = nil
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local degrees = active_rotation_turns * 90
    shroudforge.log.info(string.format(
        "World Editor rotation: %d/3 quarter turns (%d degrees) around %s. F3 rotates again, F7 places it, and Preview checks the cursor target.",
        active_rotation_turns, degrees, string.upper(axis)))
    editor_stage = "selected"
    set_editor_message("selected", string.format("Rotation set to %d°. Aim at the target and press F7 to place the blueprint.", degrees))
end

local function finite_number(value)
    return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function maximum_copyable_props()
    local configured = tonumber(setting("maximumCopyableProps"))
    if not finite_number(configured) then configured = default_maximum_copyable_props end
    return math.min(maximum_supported_props, math.max(1, math.floor(configured)))
end

publish_editor_state = function()
    if not editor_ui_available then return true end
    if save_progress.state == "saving" and editor_stage == "ready" then
        save_progress.state, save_progress.phase = "error", "Save stopped"
    end
    local axis = clipboard and clipboard.region and clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local panel_width = tonumber(setting("panelWidth")) or 1180
    local panel_height = tonumber(setting("panelHeight")) or 190
    local panel_position_x = tonumber(setting("panelPositionX")) or -1
    local panel_position_y = tonumber(setting("panelPositionY")) or 92
    if panel_width ~= panel_width then panel_width = 1180 end
    if panel_height ~= panel_height then panel_height = 190 end
    if not finite_number(panel_position_x) then panel_position_x = -1 end
    if not finite_number(panel_position_y) then panel_position_y = 92 end
    panel_width = math.floor(math.min(1920, math.max(760, panel_width)))
    panel_height = math.floor(math.min(400, math.max(140, panel_height)))
    panel_position_x = math.floor(math.min(8192, math.max(-1, panel_position_x)))
    panel_position_y = math.floor(math.min(8192, math.max(0, panel_position_y)))
    local text = table.concat({
        "saveState=" .. save_progress.state,
        "saveCompleted=" .. tostring(save_progress.completed),
        "savePhase=" .. save_progress.phase,
        "saveSequence=" .. tostring(save_progress.sequence),
        "stage=" .. tostring(editor_stage),
        "hint=" .. tostring(editor_hint),
        "selected=" .. tostring(active_blueprint_name or ""),
        "axis=" .. axis,
        "turns=" .. tostring(rotation_turns()),
        "panelWidth=" .. tostring(panel_width),
        "panelHeight=" .. tostring(panel_height),
        "panelPositionX=" .. tostring(panel_position_x),
        "panelPositionY=" .. tostring(panel_position_y),
        "processRole=" .. (runtime.is_server and "dedicated-server" or "client"),
        "executionBackend=" .. execution_backend_label(execution_backend()),
        "sessionGeneration=" .. tostring(current_session()),
        "archivedSessions=" .. tostring(#retired_sessions),
    }, "\n") .. "\n"
    if text == last_published_editor_state then return true end
    local ok, reason = pcall(io.export, "world-editor/editor-state.txt", text)
    if not ok then
        shroudforge.log.warn("World Editor could not publish its on-screen helper state: " .. tostring(reason))
        return false
    end
    last_published_editor_state = text
    return true
end

set_editor_message = function(stage, message)
    if stage then editor_stage = stage end
    if save_progress.state == "saving" and (stage == "ready" or stage == "need_a") then
        save_progress.state, save_progress.phase = "error", "Save stopped"
    end
    editor_hint = message
    if publish_editor_state then publish_editor_state() end
end

local function get_voxel_grid_spec()
    if voxel_grid_spec then return voxel_grid_spec end
    if not runtime.has("runtime.world.voxel.grid_spec") then
        local status = runtime.status("runtime.world.voxel.grid_spec")
        return nil, status and status.reason or "native voxel grid metadata is unavailable"
    end
    local spec, reason = runtime.world.voxel.get_grid_spec("voxel")
    if not spec then return nil, reason end
    voxel_grid_spec = spec
    return spec
end

local function grid_to_world(index, axis, spec)
    return spec.origin[axis] + index * spec.cellSize[axis]
end

local function world_to_grid(position, axis, spec)
    return math.floor((position - spec.origin[axis]) / spec.cellSize[axis])
end

local function rotate_vector_quarters(x, y, z, axis, turns)
    for _ = 1, turns % 4 do
        if axis == "x" then x, y, z = x, -z, y
        elseif axis == "y" then x, y, z = z, y, -x
        else x, y, z = -y, x, z end
    end
    return x, y, z
end

local function rotated_box_bounds(minimum, maximum, turns, axis)
    local low = {math.huge, math.huge, math.huge}
    local high = {-math.huge, -math.huge, -math.huge}
    for corner = 0, 7 do
        local x = corner % 2 == 0 and minimum[1] or maximum[1]
        local y = math.floor(corner / 2) % 2 == 0 and minimum[2] or maximum[2]
        local z = math.floor(corner / 4) % 2 == 0 and minimum[3] or maximum[3]
        local rx, ry, rz = rotate_vector_quarters(x, y, z, axis, turns)
        low[1], low[2], low[3] = math.min(low[1], rx), math.min(low[2], ry), math.min(low[3], rz)
        high[1], high[2], high[3] = math.max(high[1], rx), math.max(high[2], ry), math.max(high[3], rz)
    end
    return low, high
end

local function unavailable_world_reason(operation)
    if runtime.has(operation) then return nil end
    local status = runtime.status(operation)
    return status and status.reason or (operation .. " is unavailable")
end

local function world_position(position)
    if not position then return nil end
    local x, y, z = tonumber(position.x), tonumber(position.y), tonumber(position.z)
    if not finite_number(x) or not finite_number(y) or not finite_number(z) then return nil end
    -- Prop-query/get-transform Lua API records are already decoded to world units.
    return {x = x, y = y, z = z}
end

local function cursor_world_position(position)
    if not position then return nil end
    local scale = 4294967296
    local x, y, z = tonumber(position.x), tonumber(position.y), tonumber(position.z)
    if not finite_number(x) or not finite_number(y) or not finite_number(z) then return nil end
    -- The cursor hook exposes the raw Q32.32 fields from the game structure.
    return {x = x / scale, y = y / scale, z = z / scale}
end

local function rotated_blueprint(source, quarter_turns)
    local region = source.region
    local sx, sy, sz = region.sx or 0, region.sy or 0, region.sz or 0
    local axis = region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local cells, coverage, props = source.cells, source.coverage, {}
    local extent = {source.extent[1], source.extent[2], source.extent[3]}
    local voxel_offset = source.voxelOffset and {source.voxelOffset[1], source.voxelOffset[2], source.voxelOffset[3]} or nil
    local cell_size = source.hasVoxels and {source.cellSize[1], source.cellSize[2], source.cellSize[3]} or nil
    local voxel_extent = source.hasVoxels and {
        sx * cell_size[1], sy * cell_size[2], sz * cell_size[3]
    } or nil
    for index, prop in ipairs(source.props or {}) do
        props[index] = {
            itemId = prop.itemId, entityHandle = prop.entityHandle,
            templateUuidHighHex = prop.templateUuidHighHex, templateUuidLowHex = prop.templateUuidLowHex,
            x = prop.x, y = prop.y, z = prop.z,
            qx = prop.qx, qy = prop.qy, qz = prop.qz, qw = prop.qw,
            sx = prop.sx, sy = prop.sy, sz = prop.sz,
        }
    end
    local half_sqrt_two = math.sqrt(0.5)
    for _ = 1, quarter_turns % 4 do
        local next_sx, next_sy, next_sz = sx, sy, sz
        if axis == "x" then next_sy, next_sz = sz, sy
        elseif axis == "y" then next_sx, next_sz = sz, sx
        else next_sx, next_sy = sy, sx end
        if source.hasVoxels then
            local next_cells, next_coverage = {}, coverage and {} or nil
            for z = 0, sz - 1 do
                for y = 0, sy - 1 do
                    for x = 0, sx - 1 do
                        local source_index = x + sx * (y + sy * z) + 1
                        local next_x, next_y, next_z
                        if axis == "x" then next_x, next_y, next_z = x, sz - 1 - z, y
                        elseif axis == "y" then next_x, next_y, next_z = z, y, sx - 1 - x
                        else next_x, next_y, next_z = sy - 1 - y, x, z end
                        local target_index = next_x + next_sx * (next_y + next_sy * next_z) + 1
                        next_cells[target_index] = cells[source_index]
                        if coverage then next_coverage[target_index] = coverage[source_index] end
                    end
                end
            end
            cells = next_cells
            if coverage then coverage = next_coverage end
        end
        for _, prop in ipairs(props) do
            local x, y, z = prop.x, prop.y, prop.z
            prop.x, prop.y, prop.z = rotate_vector_quarters(x, y, z, axis, 1)
            local qx, qy, qz, qw = prop.qx, prop.qy, prop.qz, prop.qw
            if axis == "x" then
                prop.qx, prop.qy, prop.qz, prop.qw = half_sqrt_two * (qx + qw),
                    half_sqrt_two * (qy - qz), half_sqrt_two * (qz + qy), half_sqrt_two * (qw - qx)
            elseif axis == "y" then
                prop.qx, prop.qy, prop.qz, prop.qw = half_sqrt_two * (qx + qz),
                    half_sqrt_two * (qy + qw), half_sqrt_two * (qz - qx), half_sqrt_two * (qw - qy)
            else
                prop.qx, prop.qy, prop.qz, prop.qw = half_sqrt_two * (qx - qy),
                    half_sqrt_two * (qy + qx), half_sqrt_two * (qz + qw), half_sqrt_two * (qw - qz)
            end
        end
        if voxel_offset then
            local channel_low = voxel_offset
            local channel_high = {channel_low[1] + voxel_extent[1], channel_low[2] + voxel_extent[2], channel_low[3] + voxel_extent[3]}
            voxel_offset = rotated_box_bounds(channel_low, channel_high, 1, axis)
            voxel_extent = {voxel_extent[1], voxel_extent[2], voxel_extent[3]}
            if axis == "x" then voxel_extent[2], voxel_extent[3] = voxel_extent[3], voxel_extent[2]
            elseif axis == "y" then voxel_extent[1], voxel_extent[3] = voxel_extent[3], voxel_extent[1]
            else voxel_extent[1], voxel_extent[2] = voxel_extent[2], voxel_extent[1] end
            if axis == "x" then cell_size[2], cell_size[3] = cell_size[3], cell_size[2]
            elseif axis == "y" then cell_size[1], cell_size[3] = cell_size[3], cell_size[1]
            else cell_size[1], cell_size[2] = cell_size[2], cell_size[1] end
        end
        local bounds_low, bounds_high = rotated_box_bounds({0, 0, 0}, extent, 1, axis)
        extent = {bounds_high[1] - bounds_low[1], bounds_high[2] - bounds_low[2], bounds_high[3] - bounds_low[3]}
        sx, sy, sz = next_sx, next_sy, next_sz
    end
    local bounds_low, bounds_high = rotated_box_bounds({0, 0, 0}, source.extent, quarter_turns, axis)
    return {sx = sx, sy = sy, sz = sz, cells = cells, props = props,
        hasVoxels = source.hasVoxels, coverage = coverage, voxelOffset = voxel_offset,
        extent = extent, cellSize = cell_size, boundsMin = bounds_low, boundsMax = bounds_high}
end

local function cursor_point()
    local reason = unavailable_world_reason("runtime.world.cursor.get")
    if reason then return nil, reason end
    local snapshot, reason = runtime.world.cursor.get()
    if not snapshot then return nil, reason end
    local value = snapshot.value
    local transform = value and value.primaryTransform
    local position = transform and cursor_world_position(transform.position)
    if position then return position end
    return nil, "native cursor snapshot has no readable primary transform"
end

local function query_is_pending(reason)
    return type(reason) == "string" and (
        reason:find("still scanning", 1, true) ~= nil or
        reason:find("has not published a live sample", 1, true) ~= nil or
        reason:find("waiting for the live Keen ECS world", 1, true) ~= nil or
        reason:find("KFC Runtime is not ready", 1, true) ~= nil or
        reason:find("world operation is not ready", 1, true) ~= nil)
end

local function world_feature_pending(reason)
    return type(reason) == "string" and (
        reason:find("world operation is not ready", 1, true) ~= nil or
        reason:find("active voxel world context is not available", 1, true) ~= nil)
end

local function require_world_feature(operation, action)
    if runtime.has(operation) then return true end
    local status = runtime.status(operation)
    local reason = status and status.reason or (operation .. " is unavailable")
    if world_feature_pending(reason) then
        if pending_world_action then
            set_editor_message(nil, "A world operation is already running. Wait for it to finish, then try again.")
            shroudforge.log.warn("World Editor is already waiting for the live world context")
            return false
        end
        local session = current_session()
        if session == 0 then
            set_editor_message("ready", "The world session is unavailable. Wait for loading to finish, then retry the action.")
            return false
        end
        pending_world_action = {operation = operation, action = action, session = session, elapsed = 0}
        set_editor_message(nil, "Waiting for the live world to become ready. The requested action will continue automatically.")
        shroudforge.log.debug("World Editor is waiting for the validated world context. The requested action will continue automatically.")
        return false
    end
    feature(operation)
    return false
end

local function request_cursor_action(action, waiting_hint)
    if pending_cursor_action then
        set_editor_message(nil, "A cursor request is already running. Wait for its instruction to update, then try again.")
        shroudforge.log.warn("World Editor is already waiting for its live cursor query")
        return
    end
    local point, reason = cursor_point()
    if point then action(point); return end
    if query_is_pending(reason) then
        pending_cursor_action = action
        set_editor_message(nil, waiting_hint or "Reading the aimed cursor position…")
        shroudforge.log.debug("World Editor is resolving the live cursor. The requested action will continue automatically.")
    else
        set_editor_message(nil, "Could not read the aimed cursor position. Aim at a surface and try again.")
        shroudforge.log.warn("World Editor cursor is unavailable: " .. tostring(reason))
    end
end

local function guid_halves(reference)
    local function find_guid(value, depth)
        if depth > 4 or value == nil then return nil end
        if type(value) == "string" then
            local compact = value:gsub("[^%x]", "")
            if #compact == 32 then return compact end
            return nil
        end
        if type(value) ~= "table" then
            local text = tostring(value)
            local compact = text:gsub("[^%x]", "")
            if #compact == 32 then return compact end
            return nil
        end
        for _, key in ipairs({"value", "guid", "id", "reference", "objectId"}) do
            local found = find_guid(value[key], depth + 1)
            if found then return found end
        end
        return nil
    end
    local value = find_guid(reference, 0)
    if not value then return nil end
    -- KFC GUID strings use the Windows GUID field order; the engine spawn ABI
    -- consumes the two qwords in native little-endian memory order.
    local tail = value:sub(17, 32)
    local low = tail:sub(15, 16) .. tail:sub(13, 14) .. tail:sub(11, 12) .. tail:sub(9, 10) ..
        tail:sub(7, 8) .. tail:sub(5, 6) .. tail:sub(3, 4) .. tail:sub(1, 2)
    local high = value:sub(13, 16) .. value:sub(9, 12) .. value:sub(1, 8)
    return high, low
end

local function item_id(value)
    local kind = type(value)
    if kind == "userdata" then
        value = value.value
    elseif kind == "table" then
        value = value.value or value.id
    end
    return tonumber(value)
end

local function resolve_placeable_items()
    if placeable_items and prop_recipes_registered then return placeable_items end
    local resolved = {}
    local ok, resources = pcall(game.assets.get_resources_by_type, "keen::ItemInfo")
    if not ok or type(resources) ~= "table" then
        shroudforge.log.warn("World Editor could not read ItemInfo assets: " .. tostring(resources))
        return nil
    end
    local item_ids, placed_entities, valid_bounds_count, feedback_count = 0, 0, 0, 0
    for _, resource in ipairs(resources) do
        local data = resource.data
        local equipment = data and data.equipment
        local id = data and item_id(data.itemId)
        local reference = equipment and equipment.placedEntity
        local high, low = guid_halves(reference)
        local valid_id = id and id > 0 and id % 1 == 0
        if valid_id then item_ids = item_ids + 1 end
        if valid_id and reference ~= nil then placed_entities = placed_entities + 1 end
        local minimum, maximum = equipment and equipment.placementAABBmin, equipment and equipment.placementAABBmax
        local bounds = minimum and maximum and {
            tonumber(minimum.x), tonumber(minimum.y), tonumber(minimum.z),
            tonumber(maximum.x), tonumber(maximum.y), tonumber(maximum.z),
        }
        local valid_bounds = bounds ~= nil
        if valid_bounds then
            for index = 1, 6 do
                if not finite_number(bounds[index]) then valid_bounds = false; break end
            end
        end
        if valid_bounds then
            for axis = 1, 3 do if bounds[axis] > bounds[axis + 3] then valid_bounds = false end end
        end
        if valid_id and reference ~= nil and valid_bounds then
            valid_bounds_count = valid_bounds_count + 1
            local feedback
            for _, collider in ipairs(equipment.placementColliders or {}) do
                for _, entry in ipairs(collider.dataArray or {}) do
                    -- KFC variants expose their contained typed value as `.value`.
                    -- `$value` exists only in the raw JSON representation.
                    local candidate = entry.value.materialFeedbackId
                    feedback = item_id(candidate)
                    if feedback and feedback ~= 0 then break end
                end
                if feedback and feedback ~= 0 then break end
            end
            if feedback and feedback ~= 0 then feedback_count = feedback_count + 1 end
            if feedback and feedback ~= 0 then
                resolved[id] = {
                    id = id, uuidHigh = high, uuidLow = low, feedback = feedback,
                    bounds = bounds,
                    name = data.debugName or tostring(id),
                }
            end
        end
    end
    local count = 0
    for _ in pairs(resolved) do count = count + 1 end
    if count == 0 then
        shroudforge.log.warn(string.format(
            "World Editor found no supported ItemInfo placement recipes (resources=%d validItemIds=%d withPlacedEntity=%d withValidBounds=%d withFeedback=%d)",
            #resources, item_ids, placed_entities, valid_bounds_count, feedback_count))
        return nil
    end
    local native_recipes = {}
    for id, recipe in pairs(resolved) do
        native_recipes[#native_recipes + 1] = {itemId = id, bounds = recipe.bounds, feedback = recipe.feedback}
    end
    local registered, reason = runtime.world.entity.register_prop_recipes(native_recipes)
    if not registered then
        shroudforge.log.warn("World Editor could not register native ItemInfo placement bounds: " .. tostring(reason))
        return nil
    end
    placeable_items = resolved
    prop_recipes_registered = true
    shroudforge.log.debug("World Editor registered " .. count .. " native ItemInfo placement recipes")
    return resolved
end

local function voxel_region(source, use_current_cursor, cursor_override)
    if not source then return nil, "paste targets use world-space anchors" end
    local grid, grid_reason = get_voxel_grid_spec()
    if not grid then return nil, grid_reason end
    local names = source and {"sourceX", "sourceY", "sourceZ"} or {"targetX", "targetY", "targetZ"}
    local x, y, z
    local sx, sy, sz
    if source and (selection_a or selection_b) then
        if not selection_a or not selection_b then
            return nil, "mark both selection corners at the cursor, or clear the cursor selection"
        end
        local ax, ay, az = world_to_grid(selection_a.x, 1, grid), world_to_grid(selection_a.y, 2, grid), world_to_grid(selection_a.z, 3, grid)
        local bx, by, bz = world_to_grid(selection_b.x, 1, grid), world_to_grid(selection_b.y, 2, grid), world_to_grid(selection_b.z, 3, grid)
        x, y, z = math.min(ax, bx), math.min(ay, by), math.min(az, bz)
        sx, sy, sz = math.abs(bx - ax) + 1, math.abs(by - ay) + 1, math.abs(bz - az) + 1
    else
        local point, cursor_reason
        if not source then
            if use_current_cursor then
                if cursor_override then point = cursor_override
                else point, cursor_reason = cursor_point() end
            else point = selection_target end
        end
        if not source and use_current_cursor and not point then
            return nil, cursor_reason or "the live cursor position is unavailable"
        end
        if point then
            x, y, z = world_to_grid(point.x, 1, grid), world_to_grid(point.y, 2, grid), world_to_grid(point.z, 3, grid)
        else
            x, y, z = tonumber(setting(names[1])), tonumber(setting(names[2])), tonumber(setting(names[3]))
        end
    end
    if not sx then sx, sy, sz = tonumber(setting("sizeX")), tonumber(setting("sizeY")), tonumber(setting("sizeZ")) end
    if not x or not y or not z or not sx or not sy or not sz or
       x % 1 ~= 0 or y % 1 ~= 0 or z % 1 ~= 0 or
       sx % 1 ~= 0 or sy % 1 ~= 0 or sz % 1 ~= 0 or
       sx < 1 or sy < 1 or sz < 1 or
       sx * sy * sz > 65536 then
        return nil, "coordinates must be integers and the region must contain at most 65,536 cells"
    end
    return {x = x, y = y, z = z, sx = sx, sy = sy, sz = sz}
end

local function target_anchor(use_current_cursor, cursor_override)
    if use_current_cursor then
        if cursor_override then return {cursor_override.x, cursor_override.y, cursor_override.z} end
        local point, reason = cursor_point()
        if not point then return nil, reason end
        return {point.x, point.y, point.z}
    end
    if selection_target then return {selection_target.x, selection_target.y, selection_target.z} end
    if clipboard and not clipboard.hasVoxels then
        local x, y, z = tonumber(setting("targetWorldX")), tonumber(setting("targetWorldY")), tonumber(setting("targetWorldZ"))
        if not finite_number(x) or not finite_number(y) or not finite_number(z) then
            return nil, "props-only world target coordinates must be finite numbers"
        end
        return {x, y, z}
    end
    local grid, reason = get_voxel_grid_spec()
    if not grid then return nil, reason end
    local x, y, z = tonumber(setting("targetX")), tonumber(setting("targetY")), tonumber(setting("targetZ"))
    if not x or not y or not z or x % 1 ~= 0 or y % 1 ~= 0 or z % 1 ~= 0 then
        return nil, "manual target cell coordinates must be integers"
    end
    return {grid_to_world(x, 1, grid), grid_to_world(y, 2, grid), grid_to_world(z, 3, grid)}
end

local function plan_world_bounds(plan, anchor)
    local low, high = plan.boundsMin, plan.boundsMax
    low = {low[1], low[2], low[3]}
    high = {high[1], high[2], high[3]}
    if plan.hasVoxels and plan.voxelOffset then
        local channel_high = {plan.voxelOffset[1] + plan.sx * plan.cellSize[1],
            plan.voxelOffset[2] + plan.sy * plan.cellSize[2], plan.voxelOffset[3] + plan.sz * plan.cellSize[3]}
        for axis = 1, 3 do
            low[axis] = math.min(low[axis], plan.voxelOffset[axis])
            high[axis] = math.max(high[axis], channel_high[axis])
        end
    end
    return {anchor[1] + low[1], anchor[2] + low[2], anchor[3] + low[3],
        anchor[1] + high[1], anchor[2] + high[2], anchor[3] + high[3]}
end

local function voxel_target_region(plan, anchor)
    if not plan.hasVoxels then return nil end
    local grid, reason = get_voxel_grid_spec()
    if not grid then return nil, reason end
    for axis = 1, 3 do
        if math.abs(plan.cellSize[axis] - grid.cellSize[axis]) > 1e-6 then
            return nil, "blueprint voxel cell size does not match the active world grid"
        end
    end
    local region = {sx = plan.sx, sy = plan.sy, sz = plan.sz}
    local low = {anchor[1] + plan.voxelOffset[1], anchor[2] + plan.voxelOffset[2], anchor[3] + plan.voxelOffset[3]}
    local sizes = {plan.sx, plan.sy, plan.sz}
    for axis = 1, 3 do
        local coordinate = (low[axis] - grid.origin[axis]) / grid.cellSize[axis]
        local nearest = math.floor(coordinate + 0.5)
        if not finite_number(coordinate) or math.abs(coordinate - nearest) > 1e-6 then
            return nil, "paste anchor would place the voxel channel off-grid. Choose a cursor point aligned to the voxel grid"
        end
        region[({"x", "y", "z"})[axis]] = nearest
    end
    region.queryBounds = {low[1], low[2], low[3],
        low[1] + sizes[1] * grid.cellSize[1], low[2] + sizes[2] * grid.cellSize[2], low[3] + sizes[3] * grid.cellSize[3]}
    return region, nil, grid
end

local function capture_region_props(region)
    local unavailable = unavailable_world_reason("runtime.world.entity.query_props_in_bounds")
    if unavailable then return nil, unavailable end
    local recipes = resolve_placeable_items()
    if not recipes then return nil end
    local bounds, anchor = region.queryBounds, region.anchor
    if not bounds then
        local grid, grid_reason = get_voxel_grid_spec()
        if not grid then return nil, grid_reason or "native voxel grid metadata is unavailable" end
        bounds = {grid_to_world(region.x, 1, grid), grid_to_world(region.y, 2, grid), grid_to_world(region.z, 3, grid),
            grid_to_world(region.x + region.sx, 1, grid), grid_to_world(region.y + region.sy, 2, grid), grid_to_world(region.z + region.sz, 3, grid)}
        anchor = {bounds[1], bounds[2], bounds[3]}
    end
    anchor = anchor or {bounds[1], bounds[2], bounds[3]}
    local live_props, reason = runtime.world.entity.query_props_in_bounds(bounds)
    if not live_props then
        if not query_is_pending(reason) then
            shroudforge.log.warn("World Editor prop enumeration failed: " .. tostring(reason))
        end
        return nil, reason
    end
    local maximum = maximum_copyable_props()
    if #live_props > maximum then
        return nil, string.format("selection contains %d props, above the configured maximum of %d. Increase Maximum props per blueprint in mod settings", #live_props, maximum)
    end
    local props = {}
    for _, live_prop in ipairs(live_props) do
        local transform = live_prop.transform
        local position = transform and world_position(transform.position)
        local id = item_id(live_prop.itemId)
        local recipe = id and recipes[id]
        if position and recipe then
            local rotation = transform.orientation or {}
            local scale = transform.scale or {x = 1, y = 1, z = 1}
            local qx, qy, qz, qw = tonumber(rotation.x) or 0, tonumber(rotation.y) or 0,
                tonumber(rotation.z) or 0, tonumber(rotation.w) or 1
            local sx, sy, sz = tonumber(scale.x) or 1, tonumber(scale.y) or 1, tonumber(scale.z) or 1
            local rotation_norm = qx*qx + qy*qy + qz*qz + qw*qw
            local valid = finite_number(qx) and finite_number(qy) and finite_number(qz) and finite_number(qw) and
                finite_number(sx) and finite_number(sy) and finite_number(sz) and
                finite_number(rotation_norm) and rotation_norm > 1e-12
            if not valid then
                shroudforge.log.warn("World Editor prop capture stopped: invalid transform data for item " .. tostring(id))
                return nil
            end
            live_prop_cache[live_prop.handle] = {
                itemId = id, position = {position.x, position.y, position.z},
                qx = qx, qy = qy, qz = qz, qw = qw, sx = sx, sy = sy, sz = sz,
            }
            props[#props + 1] = {
                itemId = id, entityHandle = live_prop.handle,
                templateUuidHighHex = live_prop.templateUuidHighHex,
                templateUuidLowHex = live_prop.templateUuidLowHex,
                expected_transform = live_prop,
                x = position.x - anchor[1], y = position.y - anchor[2], z = position.z - anchor[3],
                qx = qx, qy = qy, qz = qz, qw = qw,
                sx = sx, sy = sy, sz = sz,
            }
        end
    end
    if #props > maximum then
        return nil, string.format("capture produced %d props, above the configured maximum of %d", #props, maximum)
    end
    return props
end

local function request_prop_capture(region, callback)
    if pending_prop_capture then
        shroudforge.log.warn("World Editor is already waiting for a live prop query")
        return
    end
    shroudforge.log.info("World Editor capture: resolving recipes and starting bounded prop scan")
    local props, reason = capture_region_props(region)
    if props then callback(props); return end
    if query_is_pending(reason) then
        pending_prop_capture = {region = region, callback = callback, elapsed = 0}
        shroudforge.log.info("World Editor capture: prop scan pending; continuing on subsequent updates; native watchdog checks scan progress")
        return
    end
    set_editor_message(editor_stage == "capturing" and "ready" or nil,
        "Could not read props in the selected area. Check the runtime status and try again.")
    shroudforge.log.warn("World Editor could not capture props: " .. tostring(reason or "live prop query unavailable"))
end

local function update_pending_queries(delta)
    if current_session() == 0 and (pending_world_action or pending_cursor_action or pending_prop_capture) then
        pending_world_action, pending_cursor_action, pending_prop_capture = nil, nil, nil
        set_editor_message("ready", "The world session became unavailable. Capture cancelled; mark the selection again after loading.")
        return
    end
    if pending_world_action then
        local pending = pending_world_action
        pending.elapsed = pending.elapsed + math.max(0, tonumber(delta) or 0)
        local session = current_session()
        if session ~= pending.session or pending.elapsed >= world_wait_timeout then
            pending_world_action = nil
            local reason = session ~= pending.session and "World session changed or became unavailable" or
                "The client world context did not become readable within the retry window"
            set_editor_message("ready", reason .. ". Action cancelled; retry when the world is ready.")
            shroudforge.log.warn("World Editor cancelled " .. pending.operation .. ": " .. reason)
        elseif runtime.has(pending.operation) then
            pending_world_action = nil
            pending.action()
        else
            local status = runtime.status(pending.operation)
            local reason = status and status.reason
            if not world_feature_pending(reason) then
                pending_world_action = nil
                if editor_stage == "capturing" then
                    editor_stage = "ready"
                    editor_hint = "Capture cancelled: " .. tostring(reason or "World API unavailable")
                    publish_editor_state()
                end
                shroudforge.log.warn("World Editor world query stopped: " .. tostring(reason or "world operation unavailable"))
            end
        end
    end
    if pending_cursor_action then
        local point, reason = cursor_point()
        if point then
            local action = pending_cursor_action
            pending_cursor_action = nil
            action(point)
        elseif not query_is_pending(reason) then
            pending_cursor_action = nil
            editor_hint = "Could not read the cursor: " .. tostring(reason or "cursor position unavailable")
            publish_editor_state()
            shroudforge.log.warn("World Editor cursor query stopped: " .. tostring(reason))
        end
    end
    if pending_prop_capture then
        local pending = pending_prop_capture
        pending.elapsed = pending.elapsed + math.max(0, tonumber(delta) or 0)
        if not runtime.has("runtime.world.entity.query_props_in_bounds") then
            pending_prop_capture = nil
            editor_stage = "ready"
            editor_hint = "Capture cancelled: the world became unavailable. Mark the selection again."
            publish_editor_state()
            shroudforge.log.warn("World Editor cancelled pending prop capture after world change; no blueprint was saved")
            return
        end
        local props, reason = capture_region_props(pending.region)
        if props then
            pending_prop_capture = nil
            shroudforge.log.info("World Editor capture: prop scan completed; " .. tostring(#props) .. " props")
            pending.callback(props)
        elseif not query_is_pending(reason) then
            pending_prop_capture = nil
            if editor_stage == "capturing" then
                editor_stage = "ready"
                editor_hint = "Prop capture failed: " .. tostring(reason or "live prop query unavailable")
                publish_editor_state()
            end
            shroudforge.log.warn("World Editor prop query stopped: " .. tostring(reason or "live prop query unavailable"))
        end
    end
end

local function update_world_session()
    local session = current_session()
    -- Zero also occurs briefly during loading. Do not discard an undo journal
    -- or call it a different world until a new nonzero generation is known.
    if session == 0 then return end
    if observed_session == nil then observed_session = session; return end
    if session == observed_session then return end
    local previous = observed_session
    observed_session = session
    if undo_state then
        retired_sessions[#retired_sessions + 1] = {session = previous, undo = undo_state}
        shroudforge.log.warn("World Editor retained the journal for ended session " .. tostring(previous) ..
            "; it cannot be replayed in session " .. tostring(session))
    end
    undo_state = nil
    if editor_is_server then server_remote_transaction = nil end
    pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
    selection_a, selection_b, selection_target, placement_preview = nil, nil, nil, nil
    live_prop_cache, voxel_grid_spec = {}, nil
    readiness_logged, readiness_wait_logged = false, false
    if save_progress.state == "saving" then
        save_step(save_progress.completed, "Capture cancelled: world session changed", "error")
    end
    set_editor_message(clipboard and "selected" or "need_a",
        "World changed. Blueprint retained; mark a new selection or placement target. Previous-world undo is archived.")
end

local function count_prop_at(item, position)
    local unavailable = unavailable_world_reason("runtime.world.entity.query_props")
    if unavailable then return nil, unavailable end
    local epsilon = 0.01
    local props, reason = runtime.world.entity.query_props({position[1] - epsilon, position[2] - epsilon, position[3] - epsilon,
        position[1] + epsilon, position[2] + epsilon, position[3] + epsilon}, 0)
    if not props then return nil, reason end
    if #props > 1000000 then return nil, "native prop query exceeded the 1,000,000-prop safety limit" end
    local handles = {}
    for _, prop in ipairs(props) do
        local transform = prop.transform
        local actual_position = transform and world_position(transform.position)
        if actual_position and item_id(prop.itemId) == item and
           math.abs(actual_position.x - position[1]) < 0.01 and
           math.abs(actual_position.y - position[2]) < 0.01 and
           math.abs(actual_position.z - position[3]) < 0.01 then
            handles[#handles + 1] = prop.handle
        end
    end
    return #handles, handles
end

local function same_cells(left, right)
    if type(left) ~= "table" or type(right) ~= "table" or #left ~= #right then return false end
    for index = 1, #left do if left[index] ~= right[index] then return false end end
    return true
end

local function transform_matches(actual, expected)
    if type(actual) ~= "table" or type(expected) ~= "table" then return false end
    local actual_transform = actual.transform or actual
    local expected_transform = expected.transform or expected
    local actual_position, expected_position = actual_transform.position, expected_transform.position
    local actual_rotation = actual_transform.orientation or actual_transform.rotation
    local expected_rotation = expected_transform.orientation or expected_transform.rotation
    local actual_scale, expected_scale = actual_transform.scale, expected_transform.scale
    if not actual_position or not expected_position or not actual_rotation or not expected_rotation or
       not actual_scale or not expected_scale then return false end
    local function near(a, b)
        a, b = tonumber(a), tonumber(b)
        return finite_number(a) and finite_number(b) and math.abs(a - b) <= 1e-7
    end
    for _, axis in ipairs({"x", "y", "z"}) do
        if not near(actual_position[axis], expected_position[axis]) or
           not near(actual_scale[axis], expected_scale[axis]) then return false end
    end
    for _, axis in ipairs({"x", "y", "z", "w"}) do
        if not near(actual_rotation[axis], expected_rotation[axis]) then return false end
    end
    return (actual.itemId == nil or expected.itemId == nil or actual.itemId == expected.itemId) and
        (actual.templateUuidHighHex == nil or expected.templateUuidHighHex == nil or
            actual.templateUuidHighHex == expected.templateUuidHighHex) and
        (actual.templateUuidLowHex == nil or expected.templateUuidLowHex == nil or
            actual.templateUuidLowHex == expected.templateUuidLowHex)
end

local function intended_spawn_transform(position, rotation, scale, item_id_value, template_high, template_low)
    return {
        itemId = item_id_value,
        templateUuidHighHex = template_high,
        templateUuidLowHex = template_low,
        transform = {
            position = {x = position[1], y = position[2], z = position[3]},
            orientation = {x = rotation[1], y = rotation[2], z = rotation[3], w = rotation[4]},
            scale = {x = scale[1], y = scale[2], z = scale[3]},
        },
    }
end

local function finish_capture(region, cells, props, save_and_select, anchor, extent, voxel_offset)
    local maximum = maximum_copyable_props()
    if #(props or {}) > maximum then
        set_editor_message("ready", string.format("Capture stopped. It found %d props, above your limit of %d. Raise the limit in mod settings or select a smaller area.", #props, maximum))
        shroudforge.log.warn(string.format("World Editor refused capture: %d props exceed the configured maximum of %d", #props, maximum))
        return
    end
    local rotation_axis = region and region.rotationAxis or setting("rotationAxis")
    if rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z" then rotation_axis = "y" end
    local blueprint_region = {rotationAxis = rotation_axis}
    if region then blueprint_region.sx, blueprint_region.sy, blueprint_region.sz = region.sx, region.sy, region.sz end
    clipboard = {region = blueprint_region, cells = cells, props = props,
        hasVoxels = cells ~= nil, extent = extent, voxelOffset = voxel_offset,
        cellSize = cells and region and (region.cellSize or {0.5, 0.5, 0.5}) or nil}
    if cells then
        clipboard.coverage = {}
        for index, value in ipairs(cells) do clipboard.coverage[index] = value == 0 and 1 or 2 end
    end
    placement_preview = nil
    local occupied = 0
    for _, value in ipairs(cells or {}) do if value ~= 0 then occupied = occupied + 1 end end
    if cells then
        shroudforge.log.info(string.format("World Editor captured %d verified voxel cells (%d occupied) and %d resolved props from %d,%d,%d",
            #cells, occupied, #props, region.x, region.y, region.z))
    else
        shroudforge.log.info(string.format("World Editor captured a props-only region (%d props, extent %.3f, %.3f, %.3f)",
            #props, extent[1], extent[2], extent[3]))
    end
    if save_and_select then
        local name
        for index = 1, 999999 do
            local candidate = "capture-" .. tostring(index)
            local path = "world-editor/blueprints/" .. candidate .. ".sfbp"
            local ok, exists = pcall(io.export_exists, path)
            if not ok then
                set_editor_message("ready", "Capture stopped. The blueprint folder could not be checked. Check the export folder and try F8 again.")
                shroudforge.log.error("World Editor cannot safely choose a capture name because export storage could not be inspected: " .. tostring(exists))
                return
            end
            if exists == false then name = candidate; break end
        end
        if not name or not save_blueprint_named(name) then
            set_editor_message("ready", "Capture is ready, but the blueprint could not be saved. Check the export folder and try F8 again.")
            shroudforge.log.error("World Editor captured the region but could not save a unique persistent blueprint")
            return
        end
        active_blueprint_name = name
        editor_stage = "selected"
        set_editor_message("selected", "Blueprint " .. name .. " saved and selected. Press F7 to place it. Open Manage to add a cover from F12 screenshots.")
        finish_save_progress()
        shroudforge.log.info("World Editor help: blueprint saved as '" .. name .. "' and selected. Click to select or double-click to manage it. Press F7 to place it.")
    end
end

local function export_ready_for_capture(save_and_select)
    if not save_and_select then return true end
    local ok, result = pcall(function()
        return io.export_exists("world-editor/blueprints/capture-1.sfbp")
    end)
    if not ok then
        shroudforge.log.error("World Editor cannot start F8 capture because blueprint export is unavailable: " .. tostring(result))
        return false
    end
    return true
end

local function copy_voxels(save_and_select)
    if not export_ready_for_capture(save_and_select) then return end
    if not require_world_feature("runtime.world.voxel.read", function() copy_voxels(save_and_select) end) then return end
    local region, reason = voxel_region(true)
    if not region then
        set_editor_message("ready", "Capture stopped. " .. tostring(reason) .. " Choose a smaller or valid selection, then try again.")
        shroudforge.log.warn("World Editor: " .. reason)
        return
    end
    local cells, read_reason = runtime.world.voxel.read(region.x, region.y, region.z,
        region.sx, region.sy, region.sz)
    if not cells then
        set_editor_message("ready", "Capture stopped because terrain data could not be read. Check runtime status, then try F8 again.")
        runtime.report_effect("waiting", read_reason or "voxel read failed")
        shroudforge.log.warn("World Editor voxel copy failed: " .. tostring(read_reason))
        return
    end
    local grid = get_voxel_grid_spec()
    if not grid then
        set_editor_message("ready", "Capture stopped because voxel grid information is unavailable. Check runtime status, then try again.")
        shroudforge.log.warn("World Editor could not get voxel grid metadata")
        return
    end
    local anchor, extent
    if selection_a and selection_b then
        anchor = {math.min(selection_a.x, selection_b.x), math.min(selection_a.y, selection_b.y), math.min(selection_a.z, selection_b.z)}
        local far = {math.max(selection_a.x, selection_b.x), math.max(selection_a.y, selection_b.y), math.max(selection_a.z, selection_b.z)}
        extent = {far[1] - anchor[1], far[2] - anchor[2], far[3] - anchor[3]}
    else
        anchor = {grid_to_world(region.x, 1, grid), grid_to_world(region.y, 2, grid), grid_to_world(region.z, 3, grid)}
        extent = {region.sx * grid.cellSize[1], region.sy * grid.cellSize[2], region.sz * grid.cellSize[3]}
    end
    local bounds = {grid_to_world(region.x, 1, grid), grid_to_world(region.y, 2, grid), grid_to_world(region.z, 3, grid),
        grid_to_world(region.x + region.sx, 1, grid), grid_to_world(region.y + region.sy, 2, grid), grid_to_world(region.z + region.sz, 3, grid)}
    local props_region = {queryBounds = bounds, anchor = anchor}
    local voxel_offset = {bounds[1] - anchor[1], bounds[2] - anchor[2], bounds[3] - anchor[3]}
    region.rotationAxis, region.cellSize = setting("rotationAxis"), grid.cellSize
    request_prop_capture(props_region, function(props)
        finish_capture(region, cells, props, save_and_select, anchor, extent, voxel_offset)
    end)
end

local function capture_and_save()
    if save_progress.state == "saving" or pending_prop_capture then
        shroudforge.log.warn("World Editor: wait for the current blueprint save to finish")
        return
    end
    if not selection_a then
        clipboard, active_blueprint_name, placement_preview = nil, nil, nil
        editor_stage = "need_a"
        editor_hint = "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press F5."
        publish_editor_state()
        shroudforge.log.info("World Editor help: select the building hammer, choose a single voxel, aim at the first corner, then press F5.")
        return
    end
    if not selection_b then
        editor_stage = "need_b"
        editor_hint = "Corner A is marked. Aim at the opposite corner and press F5 again."
        publish_editor_state()
        shroudforge.log.info("World Editor help: aim at the opposite corner and press F5 before capturing with F8.")
        return
    end
    editor_stage = "capturing"
    editor_hint = "Capturing props and voxels, creating the blueprint file, and adding it to the library…"
    publish_editor_state()
    save_step(0, "Capturing blueprint")
    copy_voxels(true)
    if editor_stage == "capturing" and not pending_prop_capture and not pending_world_action then
        editor_stage = "ready"
        set_editor_message("ready", "Blueprint was not saved. Check the runtime status and export folder, then try F8 again.")
    end
end

local function copy_props_only(save_and_select)
    if not export_ready_for_capture(save_and_select) then return end
    if not selection_a or not selection_b then
        shroudforge.log.warn("World Editor props-only capture requires cursor selection corners A and B")
        return
    end
    local anchor = {math.min(selection_a.x, selection_b.x), math.min(selection_a.y, selection_b.y), math.min(selection_a.z, selection_b.z)}
    local far = {math.max(selection_a.x, selection_b.x), math.max(selection_a.y, selection_b.y), math.max(selection_a.z, selection_b.z)}
    local extent = {far[1] - anchor[1], far[2] - anchor[2], far[3] - anchor[3]}
    if extent[1] <= 0 or extent[2] <= 0 or extent[3] <= 0 then
        shroudforge.log.warn("World Editor props-only capture needs a non-zero selection extent on every axis")
        return
    end
    local region = {anchor = anchor, queryBounds = {anchor[1], anchor[2], anchor[3], far[1], far[2], far[3]},
        rotationAxis = setting("rotationAxis")}
    request_prop_capture(region, function(props)
        finish_capture(region, nil, props, save_and_select, anchor, extent, nil)
    end)
end

local function blueprint_path(name)
    name = name or setting("blueprintName")
    if name == "" or not name:match("^[%w_-]+$") then
        return nil, "blueprint name may contain only letters, digits, underscores, and hyphens"
    end
    return "world-editor/blueprints/" .. name .. ".sfbp"
end

local function encode_blueprint(source)
    if not source then return nil, "no blueprint data is active" end
    local region, values = source.region, source.cells
    local extent = source.extent
    local body = {"SHROUDFORGE_WORLD_BLUEPRINT_V7",
        region.rotationAxis or "y", table.concat(extent, ","), source.hasVoxels and "voxel" or "props",
        source.hasVoxels and table.concat({region.sx, region.sy, region.sz}, ",") or "-",
        source.hasVoxels and table.concat(source.voxelOffset, ",") or "-",
        source.hasVoxels and table.concat(source.cellSize, ",") or "-",
        source.hasVoxels and table.concat(values, ",") or "-",
        source.hasVoxels and table.concat(source.coverage, ",") or "-",
        tostring(#(source.props or {}))}
    for _, prop in ipairs(source.props or {}) do
        body[#body + 1] = table.concat({prop.itemId, prop.x, prop.y, prop.z,
            prop.qx, prop.qy, prop.qz, prop.qw, prop.sx, prop.sy, prop.sz,
            prop.templateUuidHighHex, prop.templateUuidLowHex}, ",")
    end
    local content = table.concat(body, "\n")
    if #content > maximum_blueprint_bytes then
        return nil, "blueprint exceeds the 256 MiB file limit"
    end
    return content
end

save_blueprint_named = function(name)
    if not clipboard then
        set_editor_message("need_a", "There is no capture to save. Mark both corners with F5 first.")
        shroudforge.log.warn("World Editor: capture a region before saving a blueprint")
        return
    end
    local prop_count = #(clipboard.props or {})
    local maximum = maximum_copyable_props()
    if prop_count > maximum then
        set_editor_message("ready", string.format("Save stopped. This capture has %d props, above your limit of %d. Raise the limit in mod settings or capture a smaller area.", prop_count, maximum))
        shroudforge.log.warn(string.format("World Editor: cannot save %d props because the configured blueprint maximum is %d", prop_count, maximum))
        return false
    end
    local path, reason = blueprint_path(name)
    if not path then
        set_editor_message("ready", "Save stopped. " .. tostring(reason) .. ". Use letters, numbers, underscores, or hyphens in the blueprint name.")
        shroudforge.log.warn("World Editor: " .. reason)
        return
    end
    -- A direct save already has a complete capture in the clipboard.
    if save_progress.state ~= "saving" then
        editor_stage = "capturing"
        save_step(0, "Preparing blueprint")
    end
    save_step(1, "Encoding blueprint")
    local content, encode_reason = encode_blueprint(clipboard)
    if not content then
        set_editor_message("ready", "Save stopped. " .. tostring(encode_reason))
        shroudforge.log.warn("World Editor: " .. tostring(encode_reason))
        return
    end
    save_step(2, "Writing blueprint")
    local ok, err = pcall(io.export, path, content)
    if not ok then
        set_editor_message("ready", "Save failed. Check that export storage is enabled, then try again.")
        shroudforge.log.error("World Editor could not save persistent blueprint (enable export): " .. tostring(err))
        return false
    end
    if type(content) ~= "string" or #content > maximum_blueprint_bytes then
        shroudforge.log.warn("World Editor: blueprint is not text or exceeds the 256 MiB file limit")
        return false
    end
    save_step(3, "Updating library")
    shroudforge.log.info("World Editor saved persistent blueprint: " .. path)
    return true
end

local function save_blueprint()
    local name = setting("blueprintName")
    if save_blueprint_named(name) then
        active_blueprint_name = name
        set_editor_message("selected", "Blueprint " .. name .. " saved and selected. Press F7 to place it. Open Manage to add a cover from F12 screenshots.")
        finish_save_progress()
    end
end

local function load_blueprint(name, supplied_content)
    name = name or setting("blueprintName")
    local path, reason = blueprint_path(name)
    if not path then shroudforge.log.warn("World Editor: " .. reason); return end
    local content = supplied_content
    if content == nil then
        local ok, read_content = pcall(io.read_export_to_string, path)
        if not ok then
            shroudforge.log.warn("World Editor could not load blueprint (enable export and verify the file): " .. tostring(read_content))
            return
        end
        content = read_content
    end
    local lines = {}
    for line in (content .. "\n"):gmatch("([^\n]*)\n") do lines[#lines + 1] = line end
    local version = lines[1]
    local rotation_axis, extent_text, kind = lines[2], lines[3], lines[4]
    local dimensions, offset_text, cell_size_text, encoded, coverage_encoded = lines[5], lines[6], lines[7], lines[8], lines[9]
    local has_voxels = kind == "voxel"
    if version ~= "SHROUDFORGE_WORLD_BLUEPRINT_V7" or
       (rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z") or
       (kind ~= "voxel" and kind ~= "props") or not extent_text or not dimensions or not offset_text or
       not cell_size_text or not encoded or not coverage_encoded then
        shroudforge.log.warn("World Editor: blueprint format is invalid")
        return
    end
    local function parse_triplet(line)
        local a, b, c = line:match("^([^,]+),([^,]+),([^,]+)$")
        a, b, c = tonumber(a), tonumber(b), tonumber(c)
        if not finite_number(a) or not finite_number(b) or not finite_number(c) then return nil end
        return {a, b, c}
    end
    local extent = parse_triplet(extent_text)
    if not extent or extent[1] < 0 or extent[2] < 0 or extent[3] < 0 or
       (kind == "props" and (extent[1] <= 0 or extent[2] <= 0 or extent[3] <= 0)) then
        shroudforge.log.warn("World Editor: blueprint extent is invalid")
        return
    end
    local cells, coverage = {}, {}
    local sx, sy, sz, voxel_offset, cell_size
    if has_voxels then
        local dimensions_values = parse_triplet(dimensions)
        voxel_offset = parse_triplet(offset_text)
        cell_size = parse_triplet(cell_size_text)
        if not dimensions_values or not voxel_offset or not cell_size or
           cell_size[1] <= 0 or cell_size[2] <= 0 or cell_size[3] <= 0 then
            shroudforge.log.warn("World Editor: voxel channel metadata is invalid")
            return
        end
        sx, sy, sz = dimensions_values[1], dimensions_values[2], dimensions_values[3]
        if sx % 1 ~= 0 or sy % 1 ~= 0 or sz % 1 ~= 0 or sx < 1 or sy < 1 or sz < 1 or sx * sy * sz > 65536 then
            shroudforge.log.warn("World Editor: blueprint dimensions exceed supported limits")
            return
        end
        for cell in encoded:gmatch("[^,]+") do
            local value = tonumber(cell)
            if not value or value % 1 ~= 0 or value < 0 or value > 65535 then
                shroudforge.log.warn("World Editor: blueprint contains an invalid voxel cell")
                return
            end
            cells[#cells + 1] = value
        end
        if #cells ~= sx * sy * sz then
            shroudforge.log.warn("World Editor: blueprint cell count does not match its dimensions")
            return
        end
        for state in coverage_encoded:gmatch("[^,]+") do
            local value = tonumber(state)
            if not value or value % 1 ~= 0 or value < 0 or value > 2 then
                shroudforge.log.warn("World Editor: blueprint contains invalid voxel coverage")
                return
            end
            coverage[#coverage + 1] = value
        end
        if #coverage ~= #cells then
            shroudforge.log.warn("World Editor: blueprint coverage count does not match its dimensions")
            return
        end
        for index, state in ipairs(coverage) do
            if state == 2 and cells[index] == 0 then
                shroudforge.log.warn("World Editor: occupied blueprint cells must have a non-zero value")
                return
            elseif state ~= 2 then
                cells[index] = 0
            end
        end
    elseif dimensions ~= "-" or offset_text ~= "-" or cell_size_text ~= "-" or encoded ~= "-" or coverage_encoded ~= "-" then
        shroudforge.log.warn("World Editor: props-only blueprint has unexpected voxel data")
        return
    end
    local props = {}
    local count_line = 10
    local count = tonumber(lines[count_line])
    local maximum = maximum_copyable_props()
    if not count or count % 1 ~= 0 or count < 0 or count > maximum or count > maximum_supported_props or #lines ~= count_line + count then
        shroudforge.log.warn("World Editor: blueprint prop count is invalid")
        return
    end
    local recipes = count > 0 and resolve_placeable_items() or {}
    for index = 1, count do
        local fields = {}
        local field_count, invalid_field = 0, false
        for field in (lines[index + count_line] .. ","):gmatch("(.-),") do
            field_count = field_count + 1
            if field_count <= 11 then
                fields[field_count] = tonumber(field)
                if not finite_number(fields[field_count]) then invalid_field = true end
            else
                fields[field_count] = field
            end
        end
        if field_count ~= 13 or invalid_field or not fields[1] or fields[1] < 1 or
           fields[1] % 1 ~= 0 or not recipes or not recipes[fields[1]] then
            shroudforge.log.warn("World Editor: blueprint references an item without a current placement recipe")
            return
        end
        for field = 2, 11 do
            if not finite_number(fields[field]) then
                shroudforge.log.warn("World Editor: blueprint has invalid prop transform data")
                return
            end
        end
        local rotation_norm = fields[5]^2 + fields[6]^2 + fields[7]^2 + fields[8]^2
        if not finite_number(rotation_norm) or rotation_norm < 1e-12 then
            shroudforge.log.warn("World Editor: blueprint contains a zero-length prop rotation")
            return
        end
        local template_high, template_low = fields[12], fields[13]
        if type(template_high) ~= "string" or type(template_low) ~= "string" or
           not template_high:match("^%x+$") or not template_low:match("^%x+$") or
           #template_high > 16 or #template_low > 16 or
           (template_high:match("^0+$") and template_low:match("^0+$")) then
            shroudforge.log.warn("World Editor: blueprint prop has an invalid native template UUID")
            return
        end
        template_high, template_low = template_high:lower(), template_low:lower()
        props[#props + 1] = {itemId = fields[1], templateUuidHighHex = template_high,
            templateUuidLowHex = template_low, x = fields[2], y = fields[3], z = fields[4],
            qx = fields[5], qy = fields[6], qz = fields[7], qw = fields[8],
            sx = fields[9], sy = fields[10], sz = fields[11]}
    end
    clipboard = {region = {sx = sx, sy = sy, sz = sz, rotationAxis = rotation_axis},
        cells = has_voxels and cells or nil, props = props, hasVoxels = has_voxels,
        coverage = has_voxels and coverage or nil,
        extent = extent, voxelOffset = voxel_offset,
        cellSize = cell_size}
    selection_a, selection_b = nil, nil
    placement_preview = nil
    active_blueprint_name = name
    editor_stage = "selected"
    set_editor_message("selected", "Blueprint " .. name .. " loaded and selected. Press F7 to place it. Open Manage to add a cover from F12 screenshots.")
    shroudforge.log.info(string.format("World Editor loaded blueprint '%s' (%d cells, %d props). It is selected and ready for F7 placement.", active_blueprint_name, #cells, #props))
end

local blueprint_library = {}

refresh_blueprint_library = function()
    local ok, paths = pcall(io.export_list, "world-editor/blueprints")
    if not ok then
        shroudforge.log.warn("World Editor could not scan the blueprint folder: " .. tostring(paths))
        return false
    end
    local entries = {}
    for _, path in ipairs(paths) do
        local name = path:match("^world%-editor/blueprints/([^/]+)%.sfbp$")
        if name and name:match("^[%w_-]+$") then
            entries[#entries + 1] = {
                name = name,
                path = path,
                image = "world-editor/blueprints/" .. name .. ".png",
            }
        end
    end
    table.sort(entries, function(left, right) return left.name:lower() < right.name:lower() end)
    blueprint_library = entries
    shroudforge.log.info(string.format("World Editor blueprint library refreshed: %d blueprint(s)", #entries))
    for index, entry in ipairs(entries) do
        shroudforge.log.debug(string.format("World Editor blueprint %d: %s%s", index, entry.name,
            entry.name == active_blueprint_name and " [selected]" or ""))
    end
    return true
end

local function library_name_setting(key)
    local name = setting(key)
    local path, reason = blueprint_path(name)
    if not path then
        shroudforge.log.warn("World Editor: " .. tostring(reason))
        return nil
    end
    return name, path
end

local function action_names(value)
    if type(value) == "string" then
        local old_name, new_name = value:match("^([^\t]+)\t([^\t]+)$")
        if old_name and new_name then return old_name, new_name end
    end
    return setting("blueprintName"), setting("blueprintNewName")
end

local function rename_library_blueprint(value)
    local old_name, new_name = action_names(value)
    local old_path = blueprint_path(old_name)
    local new_path = blueprint_path(new_name)
    if not old_path or not new_path then
        shroudforge.log.warn("World Editor: use blueprint names containing only letters, digits, underscores, and hyphens")
        return
    end
    if not old_name or not new_name then return end
    if old_name == new_name then
        set_editor_message(nil, "Rename stopped. Enter a different blueprint name.")
        shroudforge.log.warn("World Editor: source and new blueprint names are identical")
        return
    end
    if io.export_exists(new_path) then
        set_editor_message(nil, "Rename stopped. A blueprint with that name already exists.")
        shroudforge.log.warn("World Editor: a blueprint with that name already exists")
        return
    end
    local old_image, new_image = "world-editor/blueprints/" .. old_name .. ".png",
        "world-editor/blueprints/" .. new_name .. ".png"
    local old_thumb, new_thumb = "world-editor/blueprints/" .. old_name .. ".thumb.png",
        "world-editor/blueprints/" .. new_name .. ".thumb.png"
    local has_image = io.export_exists(old_image)
    local has_thumb = io.export_exists(old_thumb)
    local ok, reason = pcall(io.export_rename, old_path, new_path)
    if not ok then
        set_editor_message(nil, "Rename failed. Check the export folder and try again.")
        shroudforge.log.error("World Editor could not rename blueprint: " .. tostring(reason))
        return
    end
    if has_image then
        local image_ok, image_reason = pcall(io.export_rename, old_image, new_image)
        if not image_ok then
            pcall(io.export_rename, new_path, old_path)
            shroudforge.log.error("World Editor could not rename the blueprint image. The rename was rolled back: " .. tostring(image_reason))
            return
        end
    end
    if has_thumb then
        local thumb_ok, thumb_reason = pcall(io.export_rename, old_thumb, new_thumb)
        if not thumb_ok then
            if has_image then pcall(io.export_rename, new_image, old_image) end
            pcall(io.export_rename, new_path, old_path)
            shroudforge.log.error("World Editor could not rename the blueprint thumbnail. The rename was rolled back: " .. tostring(thumb_reason))
            return
        end
    end
    if active_blueprint_name == old_name then
        active_blueprint_name = new_name
        publish_editor_state()
    end
    refresh_blueprint_library()
    set_editor_message(nil, "Blueprint renamed to " .. new_name .. ".")
    shroudforge.log.info("World Editor renamed blueprint " .. old_name .. " to " .. new_name)
end

local function duplicate_library_blueprint(value)
    local source_name, new_name = action_names(value)
    local source_path = blueprint_path(source_name)
    local destination = blueprint_path(new_name)
    if not source_path or not destination then
        shroudforge.log.warn("World Editor: use blueprint names containing only letters, digits, underscores, and hyphens")
        return
    end
    if not source_name or not new_name then return end
    if source_name == new_name then
        set_editor_message(nil, "Duplicate stopped. Enter a different name for the copy.")
        shroudforge.log.warn("World Editor: choose a different name for the duplicate")
        return
    end
    if io.export_exists(destination) then
        set_editor_message(nil, "Duplicate stopped. A blueprint with that name already exists.")
        shroudforge.log.warn("World Editor: a blueprint with that name already exists")
        return
    end
    local ok, reason = pcall(io.export_copy, source_path, destination)
    if not ok then
        set_editor_message(nil, "Duplicate failed. Check the export folder and try again.")
        shroudforge.log.error("World Editor could not duplicate blueprint: " .. tostring(reason))
        return
    end
    local source_image, destination_image = "world-editor/blueprints/" .. source_name .. ".png",
        "world-editor/blueprints/" .. new_name .. ".png"
    local source_thumb, destination_thumb = "world-editor/blueprints/" .. source_name .. ".thumb.png",
        "world-editor/blueprints/" .. new_name .. ".thumb.png"
    if io.export_exists(source_image) then
        local image_ok, image_reason = pcall(io.export_copy, source_image, destination_image)
        if not image_ok then
            pcall(io.export_delete, destination)
            shroudforge.log.error("World Editor could not duplicate the blueprint image. The duplicate was rolled back: " .. tostring(image_reason))
            return
        end
    end
    if io.export_exists(source_thumb) then
        local thumb_ok, thumb_reason = pcall(io.export_copy, source_thumb, destination_thumb)
        if not thumb_ok then
            pcall(io.export_delete, destination)
            pcall(io.export_delete, destination_image)
            shroudforge.log.error("World Editor could not duplicate the blueprint thumbnail. The duplicate was rolled back: " .. tostring(thumb_reason))
            return
        end
    end
    refresh_blueprint_library()
    set_editor_message(nil, "Blueprint duplicated as " .. new_name .. ".")
    shroudforge.log.info("World Editor duplicated blueprint " .. source_name .. " as " .. new_name)
end

local function delete_library_blueprint(name)
    name = name or setting("blueprintName")
    local path = blueprint_path(name)
    if not path then
        set_editor_message(nil, "Delete stopped because the blueprint name is invalid.")
        shroudforge.log.warn("World Editor: invalid blueprint name")
        return
    end
    if not name then return end
    if not io.export_exists(path) then
        set_editor_message(nil, "Delete stopped because this blueprint file no longer exists.")
        shroudforge.log.warn("World Editor: blueprint file does not exist: " .. path)
        return
    end
    local image = "world-editor/blueprints/" .. name .. ".png"
    local thumbnail = "world-editor/blueprints/" .. name .. ".thumb.png"
    local ok, reason = pcall(io.export_delete, path)
    if not ok then
        set_editor_message(nil, "Delete failed. Check the export folder and try again.")
        shroudforge.log.error("World Editor could not delete blueprint: " .. tostring(reason))
        return
    end
    if io.export_exists(image) then
        local image_ok, image_reason = pcall(io.export_delete, image)
        if not image_ok then shroudforge.log.warn("World Editor deleted the blueprint but could not delete its image: " .. tostring(image_reason)) end
    end
    if io.export_exists(thumbnail) then
        local thumb_ok, thumb_reason = pcall(io.export_delete, thumbnail)
        if not thumb_ok then shroudforge.log.warn("World Editor deleted the blueprint but could not delete its thumbnail: " .. tostring(thumb_reason)) end
    end
    if active_blueprint_name == name then
        clipboard, active_blueprint_name, placement_preview = nil, nil, nil
        editor_stage = "need_a"
        publish_editor_state()
    end
    refresh_blueprint_library()
    set_editor_message(nil, "Blueprint " .. name .. " deleted.")
    shroudforge.log.info("World Editor deleted blueprint " .. name)
end

local function paste_voxels(use_current_cursor, cursor_override, captured_props, target_override)
    if not clipboard then
        set_editor_message("need_a", "No blueprint is active. Press F5 to create one or select a saved blueprint.")
        shroudforge.log.warn("World Editor: capture or load a blueprint before pasting")
        return
    end
    if remote_request_pending or (p2p_bridge and p2p_bridge.pending()) then
        set_editor_message("building", "The World Editor is waiting for the server response. No second request was sent.")
        return
    end
    local maximum = maximum_copyable_props()
    local prop_count = #(clipboard.props or {})
    if prop_count > maximum then
        set_editor_message("selected", string.format("Placement stopped. This blueprint has %d props, above your limit of %d. Raise the limit in mod settings or choose a smaller blueprint.", prop_count, maximum))
        shroudforge.log.warn(string.format("World Editor: paste refused because the blueprint has %d props and the configured maximum is %d", prop_count, maximum))
        return
    end
    if undo_state and undo_state.recovery_required then
        set_editor_message("recovery", "Placement is paused. Press F4 to finish undoing the previous placement first.")
        shroudforge.log.warn("World Editor: restore the incomplete previous placement with F4 before starting another paste")
        return
    end
    local backend = execution_backend()
    if not require_execution_backend(backend, "F7 placement") then return end
    if backend == "p2p" then
        if paste_remote then paste_remote(use_current_cursor, cursor_override) end
        return
    end
    if clipboard.hasVoxels and not require_world_feature("runtime.world.voxel.write", function()
        paste_voxels(use_current_cursor, cursor_override, captured_props, target_override)
    end) then return end
    local context, reason
    if target_override then context = target_override
    elseif placement_preview and placement_preview.blueprint == clipboard and
       placement_preview.turns == rotation_turns() then
        context = placement_preview
    else
        local anchor
        anchor, reason = target_anchor(use_current_cursor, cursor_override)
        if not anchor and use_current_cursor and query_is_pending(reason) then
            request_cursor_action(function(point) paste_voxels(true, point) end)
            return
        end
        if not anchor then
            set_editor_message("selected", "Placement stopped. " .. tostring(reason) .. " Aim at a valid target and try again.")
            shroudforge.log.warn("World Editor: " .. tostring(reason))
            return
        end
        local turns = rotation_turns()
        local rotated = rotated_blueprint(clipboard, turns)
        local axis = clipboard.region.rotationAxis or setting("rotationAxis")
        if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
        local target
        target, reason = voxel_target_region(rotated, anchor)
        if clipboard.hasVoxels and not target then
            set_editor_message("selected", "Placement stopped. " .. tostring(reason) .. " Aim at a grid-aligned target and try again.")
            shroudforge.log.warn("World Editor: " .. tostring(reason))
            return
        end
        context = {anchor = anchor, plan = rotated, target = target,
            bounds = plan_world_bounds(rotated, anchor), turns = turns, axis = axis, blueprint = clipboard}
    end
    local anchor, target = context.anchor, context.target
    local rotated = context.plan
    if not anchor or not rotated then
        set_editor_message("selected", "Placement stopped because its plan is incomplete. Aim again, then press F7.")
        shroudforge.log.warn("World Editor: placement plan is incomplete")
        return
    end
    local turns = rotation_turns()
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    if context.blueprint ~= clipboard or context.turns ~= turns or context.axis ~= axis or
       context.plan.hasVoxels ~= clipboard.hasVoxels then
        set_editor_message("selected", "Placement settings changed while preparing. Aim again and press F7.")
        shroudforge.log.warn("World Editor: placement settings changed while the prop query was pending. Prepare the preview again.")
        return
    end
    if not clipboard.hasVoxels and #rotated.props == 0 and setting("targetPropMode") ~= "replace" then
        set_editor_message("selected", "This blueprint has no props to place. Enable target prop replacement or choose another blueprint.")
        shroudforge.log.warn("World Editor: empty props-only blueprint has nothing to paste unless target props are set to replace")
        return
    end
    local recipes = (#rotated.props > 0 or setting("targetPropMode") == "replace") and resolve_placeable_items() or {}
    if #rotated.props > 0 and (not feature("runtime.world.entity.spawn") or
       not feature("runtime.world.entity.set_scale")) then return end
    for _, prop in ipairs(rotated.props or {}) do
        local maximum_scale = 3.402823466e38
        if not finite_number(prop.sx) or not finite_number(prop.sy) or not finite_number(prop.sz) or
           math.abs(prop.sx) > maximum_scale or math.abs(prop.sy) > maximum_scale or math.abs(prop.sz) > maximum_scale then
            set_editor_message("selected", "Placement stopped because a prop has invalid scale data. Check the blueprint file.")
            shroudforge.log.warn("World Editor: blueprint prop scale is outside the native transform range")
            return
        end
        if not recipes or not recipes[prop.itemId] then
            set_editor_message("selected", "Placement stopped because a prop has no current placement recipe. Check the blueprint or game assets.")
            shroudforge.log.warn("World Editor: blueprint prop has no current native ItemInfo placement recipe")
            return
        end
    end
    local previous_cells, paste_cells
    if clipboard.hasVoxels then
        local read_reason
        previous_cells, read_reason = runtime.world.voxel.read(target.x, target.y, target.z, target.sx, target.sy, target.sz)
        if not previous_cells then
            set_editor_message("selected", "Placement stopped because the target terrain could not be backed up. Aim at another target and retry.")
            shroudforge.log.warn("World Editor could not snapshot the target before paste: " .. tostring(read_reason))
            return
        end
        paste_cells = {}
        local additive = setting("pasteVoxelMode") == "add"
        for index, value in ipairs(rotated.cells) do
            local state = rotated.coverage[index]
            if state == 0 or (additive and state == 1) then
                paste_cells[index] = previous_cells[index]
            elseif state == 1 then
                paste_cells[index] = 0
            else
                paste_cells[index] = value
            end
        end
    end
    local removed_props = {}
    if setting("targetPropMode") == "replace" then
        if not feature("runtime.world.entity.destroy") or not feature("runtime.world.entity.spawn") or
           not feature("runtime.world.entity.set_scale") then return end
        if not captured_props then
            local query_region = {queryBounds = context.bounds, anchor = anchor}
            request_prop_capture(query_region, function(props)
                paste_voxels(false, nil, props, context)
            end)
            return
        end
        local props = captured_props
        for _, prop in ipairs(props) do
            local recipe = recipes and recipes[prop.itemId]
            if not recipe then
                shroudforge.log.warn("World Editor cannot replace a target prop without a current ItemInfo recipe")
                return
            end
            local live_prop = runtime.world.entity.get_transform(prop.entityHandle)
            if not live_prop or live_prop.itemId ~= prop.itemId or
               live_prop.templateUuidHighHex ~= prop.templateUuidHighHex or
               live_prop.templateUuidLowHex ~= prop.templateUuidLowHex then
                shroudforge.log.warn("World Editor refused replace-props paste because the captured entity handle is stale")
                return
            end
            local live_transform = live_prop.transform
            local live_position = live_transform and world_position(live_transform.position)
            local live_rotation = live_transform and (live_transform.orientation or live_transform.rotation)
            local live_scale = live_transform and live_transform.scale
            if not live_position or not live_rotation or not live_scale then
                shroudforge.log.warn("World Editor cannot snapshot the exact target prop transform for undo")
                return
            end
            local sx, sy, sz = tonumber(live_scale.x), tonumber(live_scale.y), tonumber(live_scale.z)
            if not finite_number(sx) or not finite_number(sy) or not finite_number(sz) then
                shroudforge.log.warn("World Editor cannot replace a target prop with invalid scale data")
                return
            end
            removed_props[#removed_props + 1] = {
                recipe = recipe, position = {live_position.x, live_position.y, live_position.z},
                rotation = {live_rotation.x, live_rotation.y, live_rotation.z, live_rotation.w}, entityHandle = prop.entityHandle,
                templateUuidHighHex = prop.templateUuidHighHex,
                templateUuidLowHex = prop.templateUuidLowHex,
                scale = {sx, sy, sz},
                expected_transform = live_prop,
            }
        end
    end
    local session = runtime.world.session_id and runtime.world.session_id() or 0
    if session == 0 then
        set_editor_message("recovery", "World session is unavailable. No placement was sent."); return
    end
    undo_state = {
        session = session,
        region = target, cells = previous_cells, expected_cells = previous_cells,
        entities = {}, entity_index = 0, removed_props = {}, removed_prop_index = 0,
        voxel_written = false, recovery_required = true, hasVoxels = clipboard.hasVoxels,
    }
    editor_stage = "placing"
    set_editor_message("placing", "Placing " .. (active_blueprint_name or "the blueprint") .. ". Keep the target clear until placement finishes.")
    for _, prop in ipairs(removed_props) do
        local removed, remove_reason = runtime.world.entity.destroy(prop.entityHandle)
        local still_present = runtime.world.entity.get_transform(prop.entityHandle)
        local handle_gone = still_present == nil
        if handle_gone then
            undo_state.removed_props[#undo_state.removed_props + 1] = prop
            undo_state.removed_prop_index = #undo_state.removed_props
        end
        if not handle_gone then
            runtime.report_effect("write-failed", remove_reason or "target prop removal was not verified")
            shroudforge.log.warn("World Editor stopped before voxel paste because a target prop could not be safely removed")
            rollback_partial_paste()
            return
        end
    end
    if clipboard.hasVoxels then
        -- A native write can partially modify the grid even when it reports
        -- failure. Mark the snapshot as applied before calling it so automatic
        -- rollback always attempts to restore the original region.
        undo_state.voxel_written = true
        undo_state.expected_cells = paste_cells
        local ok, write_reason = runtime.world.voxel.write(target.x, target.y, target.z,
            target.sx, target.sy, target.sz, paste_cells)
        if not ok then
            local partial_cells = runtime.world.voxel.read(target.x, target.y, target.z, target.sx, target.sy, target.sz)
            if partial_cells and not same_cells(partial_cells, previous_cells) then
                undo_state.expected_cells = partial_cells
                undo_state.voxel_written = true
            end
            runtime.report_effect("write-failed", write_reason or "voxel write failed")
            shroudforge.log.error("World Editor voxel paste failed: " .. tostring(write_reason))
            rollback_partial_paste()
            return
        end
        undo_state.expected_cells = paste_cells
        undo_state.voxel_written = true
    end
    local spawned = {}
    for _, prop in ipairs(rotated.props or {}) do
        local recipe = recipes[prop.itemId]
        local position = {anchor[1] + prop.x, anchor[2] + prop.y, anchor[3] + prop.z}
        local rotation = {prop.qx, prop.qy, prop.qz, prop.qw}
        local entity_handle, spawn_reason = runtime.world.entity.spawn(prop.templateUuidHighHex or recipe.uuidHigh,
            prop.templateUuidLowHex or recipe.uuidLow,
            position, rotation, recipe.id, 0)
        if not entity_handle then
            runtime.report_effect("write-failed", spawn_reason or "entity spawn was not verified")
            shroudforge.log.error("World Editor stopped after partial paste. Entity spawn failed for item " ..
                tostring(prop.itemId) .. ": " .. tostring(spawn_reason))
            undo_state.entities, undo_state.entity_index = spawned, #spawned
            rollback_partial_paste()
            return
        end
        local scale = {prop.sx, prop.sy, prop.sz}
        local tracked = {recipe = recipe, position = position, rotation = rotation, scale = scale,
            entityHandle = entity_handle, expected_transform = intended_spawn_transform(position, rotation, scale,
                recipe.id, prop.templateUuidHighHex or recipe.uuidHigh, prop.templateUuidLowHex or recipe.uuidLow)}
        spawned[#spawned + 1] = tracked
        undo_state.entities, undo_state.entity_index = spawned, #spawned
        local scaled, scale_reason = runtime.world.entity.set_scale(entity_handle, scale)
        if not scaled then
            local current = runtime.world.entity.get_transform(entity_handle)
            if current then tracked.expected_transform = current end
            runtime.report_effect("write-failed", scale_reason or "spawned prop scale could not be set")
            shroudforge.log.error("World Editor stopped after spawn because native prop scale could not be set: " .. tostring(scale_reason))
            rollback_partial_paste()
            return
        end
        local created = runtime.world.entity.get_transform(entity_handle)
        if not created or created.itemId ~= recipe.id then
            tracked.expected_transform = created or tracked.expected_transform
            local detail = "spawn returned an entity handle that did not resolve to the requested live prop"
            runtime.report_effect("write-failed", detail)
            shroudforge.log.error("World Editor stopped after spawn because it could not track the new prop: " .. tostring(detail))
            rollback_partial_paste()
            return
        end
        tracked.expected_transform = created
        tracked.entityId = created.entityId
    end
    undo_state.entities, undo_state.entity_index = spawned, #spawned
    undo_state.recovery_required = false
    placement_preview = nil
    set_editor_message("selected", "Blueprint placed successfully. Press F4 to undo it or F7 to place it again.")
    runtime.report_effect("write-confirmed", string.format("Wrote and verified %d voxel cells and %d new props in live ECS. Save persistence is not verified.",
        #(clipboard.cells or {}), #spawned))
    shroudforge.log.info(string.format("World Editor placed blueprint with %d voxel cells and %d props at %.3f,%.3f,%.3f",
        #(clipboard.cells or {}), #spawned, anchor[1], anchor[2], anchor[3]))
end

undo_voxels = function()
    local backend = execution_backend()
    if not require_execution_backend(backend, "F4 undo") then return end
    if backend == "p2p" then
        if remote_undo and undo_remote then
            undo_remote()
        elseif remote_undo then
            set_editor_message("recovery", "The server undo token is retained, but Steam P2P is unavailable.")
        else
            set_editor_message(nil, "There is no server placement to undo. Place a blueprint on the server with F7 first.")
        end
        return
    end
    -- A recovered server token belongs to a different process/world. Let a
    -- newer local singleplayer placement keep its normal F4 priority.
    if not undo_state and remote_undo then
        set_editor_message("recovery", "A server undo token is retained. Rejoin its Dedicated Server world and press F4 to resume that undo.")
        return
    end
    local expected = undo_state and undo_state.session
    if undo_state then
        local session = runtime.world.session_id and runtime.world.session_id() or 0
        if not expected or session == 0 or session ~= expected then
            set_editor_message("recovery", "Undo belongs to a different or unavailable world session. The journal is retained; no changes were sent.")
            return
        end
    end
    if not undo_state then
        set_editor_message(nil, "Nothing to undo. Place a blueprint with F7 first.")
        shroudforge.log.warn("World Editor: there is no verified blueprint placement to undo")
        return
    end
    if #(undo_state.removed_props or {}) > 0 and
       (not feature("runtime.world.entity.spawn") or not feature("runtime.world.entity.set_scale")) then return end
    if undo_state.hasVoxels and not require_world_feature("runtime.world.voxel.write", undo_voxels) then return end
    if undo_state.hasVoxels and not undo_state.automaticRollback and
       not require_world_feature("runtime.world.voxel.read", undo_voxels) then return end
    local region = undo_state.region
    if undo_state.hasVoxels then
        if not undo_state.automaticRollback then
            local current_cells, read_reason = runtime.world.voxel.read(region.x, region.y, region.z,
                region.sx, region.sy, region.sz)
            if not current_cells then
                set_editor_message("recovery", "Undo paused. The terrain snapshot could not be read. Press F4 to retry.")
                shroudforge.log.warn("World Editor paused undo because the pasted voxel region could not be checked: " .. tostring(read_reason))
                return
            end
            local expected_cells = undo_state.voxel_written and undo_state.expected_cells or undo_state.cells
            if not same_cells(current_cells, expected_cells) then
                set_editor_message("recovery", "Undo paused because the terrain changed after placement. Restore those changes manually, then press F4 to retry.")
                shroudforge.log.warn("World Editor paused undo because the target voxels changed after the paste. No later changes were overwritten.")
                return
            end
        end
    end
    local entities = undo_state.entities or {}
    local index = undo_state.entity_index or #entities
    local removed_index = undo_state.removed_prop_index or 0
    -- Restore voxel cells before props so one unreadable prop cannot block the
    -- independent terrain snapshot. The journal preserves any remaining work.
    if undo_state.hasVoxels and undo_state.voxel_written then
        local ok, reason = runtime.world.voxel.write(region.x, region.y, region.z,
            region.sx, region.sy, region.sz, undo_state.cells)
        if not ok then
            set_editor_message("recovery", "Undo paused because the previous terrain could not be restored. Press F4 to retry.")
            runtime.report_effect("write-failed", reason or "voxel undo failed")
            shroudforge.log.error("World Editor voxel undo failed: " .. tostring(reason))
            return
        end
        undo_state.voxel_written = false
        undo_state.expected_cells = undo_state.cells
    end
    -- A queued/paused prop dismantle must never block independent terrain
    -- restoration. F4 may therefore restore the snapshot while that one input
    -- remains pending; it still must not queue a duplicate dismantle.
    if (#entities > 0 or #(undo_state.removed_props or {}) > 0) and
       not feature("runtime.world.entity.get_transform") then return end
    for entity_index = 1, index do
        local entity = entities[entity_index]
        local current, read_reason
        if entity.entityHandle then current, read_reason = runtime.world.entity.get_transform(entity.entityHandle) end
        if not current and read_reason and read_reason ~= "entity handle is stale or not a live prop" then
            set_editor_message("recovery", "Undo paused because the prop state could not be read. Terrain has been restored; press F4 to retry the prop.")
            shroudforge.log.warn("World Editor paused prop undo because the live transform read failed: " .. tostring(read_reason))
            return
        end
        if current and not undo_state.automaticRollback and not transform_matches(current, entity.expected_transform) then
            set_editor_message("recovery", "Undo paused because a placed prop was moved or changed. Restore it to its pasted state, then press F4 to retry.")
            runtime.report_effect("write-failed", "a pasted prop changed after paste")
            shroudforge.log.warn("World Editor refused undo because a pasted prop changed after paste")
            return
        end
    end
    for prop_index = 1, removed_index do
        local prop = undo_state.removed_props[prop_index]
        local current, read_reason
        if prop.entityHandle then current, read_reason = runtime.world.entity.get_transform(prop.entityHandle) end
        if not current and read_reason and read_reason ~= "entity handle is stale or not a live prop" then
            set_editor_message("recovery", "Undo paused because the replaced prop state could not be read. Terrain has been restored; press F4 to retry.")
            shroudforge.log.warn("World Editor paused replaced-prop undo because the live transform read failed: " .. tostring(read_reason))
            return
        end
        if not undo_state.automaticRollback and current then
            set_editor_message("recovery", "Undo paused because a replaced prop is already present. Check the world, then press F4 to retry.")
            runtime.report_effect("write-failed", "a replaced prop reappeared before undo")
            shroudforge.log.warn("World Editor refused undo because a replaced prop is already live again")
            return
        end
    end
    set_editor_message("placing", undo_state.automaticRollback and "Restoring the world after the failed placement…" or "Undo in progress. Removing placed props and restoring the previous world state…")
    while index >= 1 do
        local entity = entities[index]
        if not entity.entityHandle then
            set_editor_message("recovery", "Undo paused because a prop handle is missing. Press F4 to retry recovery.")
            runtime.report_effect("write-failed", "pasted prop has no live entity handle")
            shroudforge.log.warn("World Editor paused undo because live prop state could not be inspected")
            return
        end
        local current, read_reason = runtime.world.entity.get_transform(entity.entityHandle)
        if not current and read_reason and read_reason ~= "entity handle is stale or not a live prop" then
            set_editor_message("recovery", "Undo paused because the prop state could not be read. Terrain has been restored; press F4 to retry the prop.")
            shroudforge.log.warn("World Editor paused prop removal because the live transform read failed: " .. tostring(read_reason))
            return
        end
        if not current then
            index = index - 1
            undo_state.entity_index = index
            shroudforge.log.info("World Editor undo accepted an already absent pasted prop handle " .. tostring(entity.entityHandle))
        else
        -- Remove the exact server/local prop handle from the same runtime that
        -- created it; handles never leave this process.
        local removed, remove_reason = runtime.world.entity.destroy(entity.entityHandle)
        if runtime.world.session_id() ~= undo_state.session then
            set_editor_message("recovery", "World session changed during removal. Undo progress is retained for inspection."); return
        end
        if not runtime.world.entity.get_transform(entity.entityHandle) then
            index = index - 1
            undo_state.entity_index = index
        else
            local detail = remove_reason or "the exact pasted ECS handle is still present after the destroy call"
            set_editor_message("recovery", "Undo paused because a placed prop could not be removed. " .. tostring(detail) .. ". Press F4 to retry.")
            runtime.report_effect("write-failed", detail)
            shroudforge.log.error("World Editor paused undo and preserved its progress: " .. tostring(detail))
            return
        end
        end
    end
    while removed_index >= 1 do
        local prop = undo_state.removed_props[removed_index]
        if not feature("runtime.world.entity.spawn") then return end
        local entity_handle, spawn_reason = prop.restoreHandle
        if not entity_handle then
            entity_handle, spawn_reason = runtime.world.entity.spawn(prop.templateUuidHighHex or prop.recipe.uuidHigh,
                prop.templateUuidLowHex or prop.recipe.uuidLow,
                prop.position, prop.rotation, prop.recipe.id, 0)
            prop.restoreHandle = entity_handle
        end
        if entity_handle then
            local scaled, scale_reason = runtime.world.entity.set_scale(entity_handle, prop.scale)
            if not scaled then
                set_editor_message("recovery", "Undo paused while restoring a replaced prop's scale. Press F4 to retry.")
                shroudforge.log.error("World Editor paused undo while restoring a replaced prop's scale: " ..
                    tostring(scale_reason or "native scale update failed"))
                return
            end
        end
        local restored = entity_handle and runtime.world.entity.get_transform(entity_handle)
        if not restored or not transform_matches(restored, prop.expected_transform) then
            set_editor_message("recovery", "Undo paused while restoring a replaced prop. Press F4 to retry.")
            shroudforge.log.error("World Editor paused undo while restoring a replaced target prop: " ..
                tostring(spawn_reason or "spawn could not be verified by entity handle"))
            return
        end
        prop.restoreHandle = nil
        removed_index = removed_index - 1
        undo_state.removed_prop_index = removed_index
    end
    local had_voxels = undo_state.hasVoxels
    undo_state = nil
    set_editor_message("selected", had_voxels and "Undo completed successfully. The previous terrain and prop state was restored." or
        "Undo completed successfully. The placed props were removed and replaced props were restored.")
    runtime.report_effect("write-confirmed", had_voxels and
        "Restored the previous voxel snapshot and removed/restored verified props" or
        "Removed pasted props and restored replaced props")
    shroudforge.log.info(had_voxels and "World Editor restored the previous voxel snapshot and affected props" or
        "World Editor undid the props-only placement")
end

rollback_partial_paste = function()
    if not undo_state then return end
    shroudforge.log.warn("World Editor is rolling back the changes completed before the paste failure")
    undo_state.automaticRollback = true
    undo_voxels()
    if undo_state then
        undo_state.automaticRollback = nil
        shroudforge.log.error("World Editor rollback is incomplete. Press F4 again after resolving the recovery issue.")
        set_editor_message("recovery", "Rollback is incomplete. Press F4 to continue restoring the previous world state.")
    else
        shroudforge.log.info("World Editor automatically restored the pre-paste snapshot")
        set_editor_message("selected", "Placement failed, and the previous world state was restored.")
    end
end

local function mark_cursor(which)
    request_cursor_action(function(point)
        if which == "a" then selection_a = point
        elseif which == "b" then selection_b = point
        else selection_target = point end
        if which == "a" then
            editor_stage = "need_b"
            editor_hint = "Corner A is marked. Aim at the opposite corner and press F5 again."
        elseif which == "b" then
            editor_stage = "ready"
            editor_hint = "Both corners are marked. Press F8 to capture and save the blueprint."
        end
        publish_editor_state()
        shroudforge.log.info(string.format("World Editor cursor selection %s = %.3f, %.3f, %.3f",
            which == "target" and "TARGET" or which:upper(), point.x, point.y, point.z))
    end)
end

local function preview_paste_at(point)
    if not clipboard then
        set_editor_message("need_a", "No blueprint is active. Press F5 to create one or select a saved blueprint.")
        shroudforge.log.warn("World Editor: copy or load a blueprint before previewing")
        return
    end
    if undo_state and undo_state.recovery_required then
        shroudforge.log.warn("World Editor: restore the incomplete previous placement with F4 before preparing another placement")
        return
    end
    local turns = rotation_turns()
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local plan = rotated_blueprint(clipboard, turns)
    local anchor = {point.x, point.y, point.z}
    local target, reason = voxel_target_region(plan, anchor)
    if clipboard.hasVoxels and not target then
        set_editor_message("selected", "Preview could not use that target. " .. tostring(reason) .. " Aim at another point.")
        shroudforge.log.warn("World Editor preview rejected: " .. tostring(reason))
        return
    end
    local occupied = 0
    for _, cell in ipairs(plan.cells or {}) do if cell ~= 0 then occupied = occupied + 1 end end
    placement_preview = {blueprint = clipboard, target = target, anchor = anchor, turns = turns, axis = axis, plan = plan}
    set_editor_message("selected", "Preview ready. No world changes were made. Press F7 to place the blueprint.")
    shroudforge.log.info(string.format(
        "World Editor placement plan prepared: axis=%s turns=%d anchor=%.3f,%.3f,%.3f voxels=%s size=%d,%d,%d occupied=%d props=%d. No world changes were made.",
        axis, turns, anchor[1], anchor[2], anchor[3], tostring(plan.hasVoxels), plan.sx, plan.sy, plan.sz, occupied, #plan.props))
end

local function preview_paste()
    if not clipboard then
        set_editor_message("need_a", "No blueprint is active. Press F5 to create one or select a saved blueprint.")
        shroudforge.log.warn("World Editor: copy or load a blueprint before previewing")
        return
    end
    request_cursor_action(preview_paste_at, "Reading the cursor to prepare a placement preview…")
end

local function clear_cursor_selection()
    selection_a, selection_b, selection_target = nil, nil, nil
    shroudforge.log.info("World Editor cursor marks cleared. Manual coordinates are active.")
end

local function mark_cursor_next()
    if undo_state and undo_state.recovery_required then
        editor_hint = "Finish the incomplete placement recovery with F4 before starting a new blueprint."
        publish_editor_state()
        shroudforge.log.warn("World Editor: cannot start a new capture while placement recovery is required")
        return
    end
    if editor_stage == "capturing" or editor_stage == "placing" then
        shroudforge.log.warn("World Editor: wait for the current capture or placement to finish")
        return
    end
    local starting_new_capture = editor_stage ~= "need_b"
    if starting_new_capture then
        clipboard, active_blueprint_name, placement_preview = nil, nil, nil
        selection_a, selection_b = nil, nil
        editor_stage = "need_a"
        editor_hint = "New blueprint selected. Aim at the first corner and press F5 to mark it."
        publish_editor_state()
    end
    request_cursor_action(function(point)
        if starting_new_capture then
            selection_a, selection_b = point, nil
            editor_stage = "need_b"
            editor_hint = "Corner A is marked. Aim at the opposite corner and press F5 again."
            publish_editor_state()
            shroudforge.log.info(string.format("World Editor help: first corner marked at %.3f, %.3f, %.3f. Aim at the opposite corner and press F5 again.",
                point.x, point.y, point.z))
        else
            selection_b = point
            editor_stage = "ready"
            editor_hint = "Both corners are marked. Press F8 to capture and save the blueprint."
            publish_editor_state()
            shroudforge.log.info(string.format("World Editor help: second corner marked at %.3f, %.3f, %.3f. Selection is ready. Press F8 to capture and save.",
                point.x, point.y, point.z))
        end
    end, starting_new_capture and "Reading the first selection corner…" or "Reading the second selection corner…")
end

local function start_new_blueprint()
    if undo_state and undo_state.recovery_required then
        editor_hint = "Finish the incomplete placement recovery with F4 before starting a new blueprint."
        publish_editor_state()
        shroudforge.log.warn("World Editor: cannot start a new blueprint while placement recovery is required")
        return
    end
    pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
    clipboard, active_blueprint_name, placement_preview = nil, nil, nil
    selection_a, selection_b = nil, nil
    set_editor_message("need_a", "New blueprint selected.")
    editor_stage = "need_a"
    editor_hint = "New blueprint selected. Aim at the first corner and press F5 to mark it."
    publish_editor_state()
    shroudforge.log.info("World Editor: new blueprint capture selected. Press F5 to mark the first corner.")
end

local function reset_editor()
    if undo_state and undo_state.recovery_required then
        shroudforge.log.warn("World Editor reset refused: undo the incomplete paste with F4 before clearing editor state")
        editor_hint = "Reset blocked: press F4 to restore the incomplete placement first."
        publish_editor_state()
        return
    end
    selection_a, selection_b, selection_target = nil, nil, nil
    if save_progress.state == "saving" then
        save_step(save_progress.completed, "Save cancelled", "error")
    end
    pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
    -- Cancelling a capture must not discard the journal of an earlier paste.
    clipboard, active_blueprint_name = nil, nil
    live_prop_cache = {}
    placement_preview = nil
    editor_stage = "need_a"
    editor_hint = "Editor reset. Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press F5."
    publish_editor_state()
    shroudforge.log.info("World Editor reset: capture cancelled and selection cleared; undo history preserved")
end

local function list_props()
    local region, reason = voxel_region(true)
    if not region then shroudforge.log.warn("World Editor: " .. tostring(reason)); return end
    request_prop_capture(region, function(props)
    shroudforge.log.debug(string.format("World Editor found %d placeable props with known ItemInfo recipes in the selected region", #props))
        for index = 1, math.min(#props, 100) do
            local prop = props[index]
            local recipe = resolve_placeable_items()[prop.itemId]
        shroudforge.log.trace(string.format("World Editor prop %d: handle=%d %s item=%d at local %.3f, %.3f, %.3f",
                index, prop.entityHandle, recipe.name, prop.itemId, prop.x, prop.y, prop.z))
        end
    if #props > 100 then shroudforge.log.debug("World Editor prop listing is truncated at 100 entries") end
    end)
end

local function direct_edit_allowed()
    local backend = execution_backend()
    if backend == "unknown" then
        require_execution_backend(backend, "direct entity operation")
        return false
    end
    if backend == "p2p" or remote_request_pending or (p2p_bridge and p2p_bridge.pending()) then
        set_editor_message("selected","Use blueprint paste and F4 undo for server edits.")
        return false
    end
    return true
end

local function destroy_selected_prop()
    if not direct_edit_allowed() then return end
    if not feature("runtime.world.entity.query_props") or not feature("runtime.world.entity.destroy") then return end
    local handle = tonumber(setting("entityHandle"))
    if not handle or handle < 1 or handle % 1 ~= 0 then
        shroudforge.log.warn("World Editor: enter a live prop handle from the List props output")
        return
    end
    local selected = live_prop_cache[handle]
    local item = selected and selected.itemId
    local position = selected and {x = selected.position[1], y = selected.position[2], z = selected.position[3]}
    local recipe = item and resolve_placeable_items() and resolve_placeable_items()[item]
    if not position or not recipe then
        shroudforge.log.warn("World Editor: the selected handle is stale or has no verified ItemInfo placement recipe")
        return
    end
    local live = runtime.world.entity.get_transform(handle)
    if not live or live.itemId ~= item then
        shroudforge.log.warn("World Editor cannot safely delete this selection: its handle is stale")
        return
    end
    local ok, reason = runtime.world.entity.destroy(handle)
    if runtime.world.entity.get_transform(handle) then
        runtime.report_effect("write-failed", "the selected live ECS handle remains after the destroy call")
        shroudforge.log.error("World Editor could not remove selected prop handle " .. tostring(handle) .. ": " .. tostring(reason))
        return
    end
    runtime.report_effect("write-confirmed", "The selected live ECS handle disappeared. Save persistence is not verified.")
    shroudforge.log.info("World Editor removed the selected prop at its live transform. Persistence is not verified.")
end

local function report_recipe_catalog()
    local recipes = resolve_placeable_items()
    if not recipes then return end
    local count = 0
    for _ in pairs(recipes) do count = count + 1 end
    shroudforge.log.debug("World Editor recipe catalog is ready from this build's ItemInfo assets: " .. count .. " placeable item recipes")
end

local function entity_transform()
    return {tonumber(setting("entityX")), tonumber(setting("entityY")), tonumber(setting("entityZ"))},
        {0, 0, 0, 1}
end

local function entity_bounds()
    return {
        tonumber(setting("boundsMinX")), tonumber(setting("boundsMinY")), tonumber(setting("boundsMinZ")),
        tonumber(setting("boundsMaxX")), tonumber(setting("boundsMaxY")), tonumber(setting("boundsMaxZ")),
    }
end

local function spawn_entity()
    if not direct_edit_allowed() then return end
    if not feature("runtime.world.entity.spawn") then return end
    local tracking = tonumber(setting("trackingId"))
    if not tracking or tracking < 1 or tracking % 1 ~= 0 then
        shroudforge.log.warn("World Editor: enter a nonzero placement tracking ID")
        return
    end
    local position, rotation = entity_transform()
    local entity_handle, reason = runtime.world.entity.spawn(setting("templateUuidHigh"), setting("templateUuidLow"),
        position, rotation, tracking, 0)
    if not entity_handle then
        runtime.report_effect("waiting", reason or "native spawn was not dispatched")
        shroudforge.log.warn("World Editor spawn failed: " .. tostring(reason))
        return
    end
    local prop = runtime.world.entity.get_transform(entity_handle)
    if not prop or prop.itemId ~= tracking then
        runtime.report_effect("write-failed", "spawn returned a handle that did not resolve to the requested live prop")
        shroudforge.log.error("World Editor could not resolve the spawned entity handle")
        return
    end
    runtime.report_effect("write-confirmed", "Spawn returned live ECS entity handle " .. tostring(entity_handle) .. ". Save persistence is not verified.")
    shroudforge.log.info("World Editor verified spawned entity handle=" .. tostring(entity_handle))
end

local function placement_operation(destroy)
    if not direct_edit_allowed() then return end
    local operation = destroy and "runtime.world.entity.destroy" or "runtime.world.entity.place"
    if not feature(operation) or not feature("runtime.world.entity.finish_building") then return end
    if not feature("runtime.world.entity.query_props") then return end
    local tracking, feedback = tonumber(setting("trackingId")), tonumber(setting("feedbackId"))
    local bounds = entity_bounds()
    if not tracking or tracking < 1 or tracking % 1 ~= 0 or
       (not destroy and (not feedback or feedback < 1 or feedback % 1 ~= 0)) then
        shroudforge.log.warn(destroy and "World Editor: tracking ID must be a nonzero integer" or
            "World Editor: tracking and material feedback IDs must be nonzero integers")
        return
    end
    for _, value in ipairs(bounds) do
        if not value or value ~= value or value == math.huge or value == -math.huge then
            shroudforge.log.warn("World Editor: all placement bounds must be finite numbers")
            return
        end
    end
    local position, rotation = entity_transform()
    local before_count, before_handles = count_prop_at(tracking, position)
    if before_count == nil then
        shroudforge.log.warn("World Editor cannot dispatch this operation without a readable pre-operation prop count: " .. tostring(before_handles))
        return
    end
    if destroy and before_count == 0 then
        runtime.report_effect("no-target", "No matching live prop exists at these coordinates")
        shroudforge.log.warn("World Editor found no matching live prop to destroy")
        return
    end
    if destroy and before_count ~= 1 then
        shroudforge.log.warn("World Editor requires one unambiguous prop handle at the configured transform")
        return
    end
    local ok, reason
    if destroy then
        ok, reason = runtime.world.entity.destroy(before_handles[1])
    else
        ok, reason = runtime.world.entity.place(position, rotation, bounds, tracking, feedback)
    end
    if destroy then
        if not runtime.world.entity.get_transform(before_handles[1]) then
            runtime.report_effect("write-confirmed", "Matching live prop count decreased. Save persistence is not verified.")
            shroudforge.log.info("World Editor verified exact-handle removal in the live ECS")
        else
            runtime.report_effect("write-failed", reason or "native destroy call did not reduce the matching live prop count")
            shroudforge.log.error("World Editor destroy did not remove a matching live prop")
        end
    else
        local after_count, after_reason = count_prop_at(tracking, position)
        if after_count == nil then
            runtime.report_effect("no-change", "Native operation returned " .. tostring(ok) .. ". Live ECS result could not be read: " .. tostring(after_reason))
            shroudforge.log.warn("World Editor operation is unconfirmed because the live ECS could not be read: " .. tostring(after_reason))
            return
        end
        if after_count > before_count then
            runtime.report_effect("write-confirmed", "Matching live prop count increased after placement. Save persistence is not verified.")
            shroudforge.log.info("World Editor verified native placement in the live ECS")
        else
            runtime.report_effect("no-change", reason or "placement returned without a matching live prop appearing")
            shroudforge.log.warn("World Editor placement was dispatched but no matching live prop was observed")
        end
    end
end

local function configured_server_steam_id()
    if runtime.network and runtime.network.status then
        local ok, status = pcall(runtime.network.status)
        local discovered = ok and status and tostring(status.local_dedicated_server_steam_id or "") or ""
        if #discovered >= 16 and #discovered <= 20 and discovered:match("^%d+$") and discovered ~= "0" then
            return discovered, true
        end
    end
    local value = tostring(setting("serverSteamId") or ""):gsub("%s", "")
    if value == "" or #value < 16 or #value > 20 or not value:match("^%d+$") or value == "0" then
        return nil
    end
    return value, false
end

local function configured_client_steam_ids()
    local ids, seen = {}, {}
    for candidate in tostring(setting("allowedClientSteamIds") or ""):gmatch("[^,;]+") do
        local peer = candidate:gsub("%s", "")
        if #peer >= 16 and #peer <= 20 and peer:match("^%d+$") and peer ~= "0" and not seen[peer] then
            seen[peer] = true
            ids[#ids + 1] = peer
        end
    end
    return ids
end

local function valid_peer_id(value)
    return type(value) == "string" and #value >= 16 and #value <= 20 and
        value:match("^%d+$") ~= nil and value ~= "0"
end

local function server_peer_ids()
    local configured = configured_client_steam_ids()
    if #configured > 0 then return configured end
    if not editor_is_server or not runtime.network or not runtime.network.connected_peers then return {} end
    local peers, reason = runtime.network.connected_peers()
    if type(peers) ~= "table" then
        if reason then shroudforge.log.debug("World Editor could not read authenticated server players: " .. tostring(reason)) end
        return {}
    end
    local ids, seen = {}, {}
    for _, peer in ipairs(peers) do
        if valid_peer_id(peer) and not seen[peer] then
            ids[#ids + 1], seen[peer] = peer, true
        end
    end
    return ids
end

local server_undo_export_path = "world-editor/server-undo-token.txt"
local server_undo_export_header = "SHROUDFORGE_WORLD_EDITOR_SERVER_UNDO_V1"

local function persist_server_undo_token()
    if not editor_ui_available or not io.export then return false end
    local peer, transaction = "none", "none"
    if remote_undo then
        peer, transaction = tostring(remote_undo.peer or ""), tostring(remote_undo.transaction or "")
        if not valid_peer_id(peer) or not transaction:match("^tx_[%w_-]+$") then
            shroudforge.log.error("World Editor refused to persist an invalid server undo token")
            return false
        end
    end
    local contents = table.concat({server_undo_export_header, peer, transaction, ""}, "\n")
    local ok, reason = pcall(io.export, server_undo_export_path, contents)
    if not ok then
        shroudforge.log.error("World Editor could not persist the server undo token: " .. tostring(reason))
        return false
    end
    return true
end

local function recover_server_undo_token()
    if not editor_ui_available or not io.read_export_to_string then return false end
    local ok, contents = pcall(io.read_export_to_string, server_undo_export_path)
    if not ok or type(contents) ~= "string" then return false end
    contents = contents:gsub("\r\n", "\n")
    local header, peer, transaction = contents:match("^([^\n]+)\n([^\n]+)\n([^\n]+)\n?$")
    if header ~= server_undo_export_header or peer == "none" or transaction == "none" then return false end
    if not valid_peer_id(peer) or not transaction:match("^tx_[%w_-]+$") then
        shroudforge.log.warn("World Editor ignored an invalid persisted server undo token")
        return false
    end
    remote_undo = {peer = peer, transaction = transaction}
    return true
end

local function on_p2p_result(operation, ok, transaction, reason, peer)
    remote_request_pending = false
    if operation == "paste" then
        if transaction then
            remote_undo = {peer = peer, transaction = transaction}
            persist_server_undo_token()
        end
        if ok then
            set_editor_message("selected", "Server placed the blueprint through its native world runtime. Press F4 to undo.")
            shroudforge.log.info("World Editor P2P paste confirmed by dedicated server; undo token=" .. tostring(transaction))
        else
            set_editor_message(transaction and "recovery" or "selected",
                transaction and "Server placement needs recovery. Press F4 to request the server-owned undo." or
                    ("Server refused blueprint placement: " .. tostring(reason)))
            shroudforge.log.warn("World Editor P2P paste failed: " .. tostring(reason))
        end
    elseif operation == "undo" then
        if ok then
            remote_undo = nil
            persist_server_undo_token()
            set_editor_message("selected", "The server confirmed undo through its native world runtime.")
            shroudforge.log.info("World Editor P2P undo confirmed by dedicated server")
        else
            set_editor_message("recovery", "Server undo is incomplete. Press F4 to retry safely. " .. tostring(reason))
            shroudforge.log.warn("World Editor P2P undo is incomplete: " .. tostring(reason))
        end
    end
end

local function server_begin_remote_paste(peer, request_id, content, metadata, done)
    if not editor_is_server then done(false, nil, "server-runtime-not-active"); return end
    if server_remote_paste or server_remote_transaction or undo_state then
        done(false, nil, "server-has-an-unresolved-world-editor-transaction")
        return
    end
    clipboard, active_blueprint_name = nil, nil
    local blueprint_name = "remote-" .. tostring(request_id):gsub("[^%w_-]", "")
    load_blueprint(blueprint_name, content)
    if not clipboard then
        done(false, nil, editor_hint or "server-could-not-parse-blueprint")
        return
    end
    local configured_rotation = math.floor(tonumber(shroudforge.settings.get("rotationQuarterTurns")) or 0) % 4
    active_rotation_turns, observed_rotation_setting = metadata.turns, configured_rotation
    local plan = rotated_blueprint(clipboard, metadata.turns)
    local anchor = metadata.anchor
    local target, target_reason = voxel_target_region(plan, anchor)
    if clipboard.hasVoxels and not target then
        done(false, nil, "server-rejected-target-grid-alignment: " .. tostring(target_reason))
        return
    end
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local context = {anchor = anchor, plan = plan, target = target,
        bounds = plan_world_bounds(plan, anchor), turns = metadata.turns, axis = axis, blueprint = clipboard}
    remote_setting_override = {pasteVoxelMode = metadata.voxelMode, targetPropMode = metadata.targetPropMode}
    server_remote_paste = {peer = peer, request_id = request_id, metadata = metadata,
        done = done, context = context, elapsed = 0, started = false,
        session = current_session(), initial_undo = undo_state}
    shroudforge.log.info(string.format("World Editor P2P received validated V7 blueprint %s from Steam peer %s (%d bytes; server applies it natively)",
        blueprint_name, peer, #content))
end

local function server_begin_remote_undo(peer, request_id, transaction, done)
    local journal = server_remote_transaction
    if not journal or journal.peer ~= peer or journal.transaction ~= transaction then
        done(false, "server-undo-token-is-stale-or-belongs-to-another-peer")
        return
    end
    if server_remote_undo then done(false, "server-undo-is-already-running"); return end
    server_remote_undo = {peer = peer, request_id = request_id, transaction = transaction,
        done = done, elapsed = 0, session = journal.session}
end

local function network_api()
    return {network = runtime.network, world = runtime.world, is_server = editor_is_server,
        log = shroudforge.log}
end

local function create_p2p_bridge()
    if not runtime.network then return nil end
    return require("p2p")(network_api(), {
        is_allowed_peer = function(peer)
            for _, allowed in ipairs(server_peer_ids()) do
                if allowed == peer then return true end
            end
            return false
        end,
        on_result = on_p2p_result,
        on_paste = server_begin_remote_paste,
        on_undo = server_begin_remote_undo,
    })
end

paste_remote = function(use_current_cursor, cursor_override)
    if not p2p_bridge then
        set_editor_message("selected", "Steam P2P is unavailable in this client runtime.")
        return
    end
    -- The persisted token can outlive a successful server-side undo when its
    -- acknowledgement is lost. The server owns the authoritative journal and
    -- rejects F7 while that journal is genuinely unresolved, so a stale client
    -- token must not permanently lock placement.
    local peer, discovered_local_server = configured_server_steam_id()
    if not peer then
        set_editor_message("selected", "Set the dedicated server's SteamID64 in World Editor → Server P2P before using F7 online.")
        return
    end
    local function start_at(anchor)
        if not clipboard then return end
        local content, reason = encode_blueprint(clipboard)
        if not content then set_editor_message("selected", tostring(reason)); return end
        local ok, request = p2p_bridge.start_paste(peer, content, {
            anchor = anchor, turns = rotation_turns(),
            voxelMode = setting("pasteVoxelMode") == "add" and "add" or "replace",
            targetPropMode = setting("targetPropMode") == "replace" and "replace" or "keep",
        })
        if not ok then set_editor_message("selected", tostring(request)); return end
        remote_request_pending = true
        set_editor_message("building", "Sending the existing V7 blueprint to the dedicated server. Its runtime performs the placement; wait for confirmation before pressing F4.")
        shroudforge.log.info("World Editor queued P2P blueprint request " .. tostring(request) .. " to server Steam peer " .. peer ..
            (discovered_local_server and " (discovered from the running Dedicated Server)" or " (configured fallback)"))
    end
    local anchor, reason = target_anchor(use_current_cursor, cursor_override)
    if not anchor and use_current_cursor and query_is_pending(reason) then
        request_cursor_action(function(point) start_at({point.x, point.y, point.z}) end,
            "Reading the cursor before sending the blueprint to the server…")
        return
    end
    if not anchor then
        set_editor_message("selected", "Server placement stopped. " .. tostring(reason) .. " Aim at a valid target and retry.")
        return
    end
    start_at(anchor)
end

undo_remote = function()
    if not remote_undo then
        set_editor_message(nil, "There is no confirmed server placement to undo. Place a blueprint on the server with F7 first.")
        return
    end
    if not p2p_bridge then set_editor_message("recovery", "Steam P2P is unavailable; the server undo token is retained."); return end
    if remote_request_pending or p2p_bridge.pending() then
        set_editor_message("building", "The server is still processing the previous World Editor request. No duplicate was sent.")
        return
    end
    local ok, request = p2p_bridge.start_undo(remote_undo.peer, remote_undo.transaction)
    if not ok then set_editor_message("recovery", tostring(request)); return end
    remote_request_pending = true
    set_editor_message("building", "F4 sent the server-owned undo request. Waiting for its native readback result…")
    shroudforge.log.info("World Editor queued P2P undo request " .. tostring(request))
end

local function finish_server_remote_paste(job, ok, transaction, reason)
    if server_remote_paste ~= job then return end
    server_remote_paste = nil
    remote_setting_override = nil
    job.done(ok, transaction, reason)
end

local function update_server_remote_paste(delta_seconds)
    local job = server_remote_paste
    if not job then return end
    job.elapsed = job.elapsed + math.max(0, tonumber(delta_seconds) or 0)
    local session = current_session()
    if job.session ~= 0 and session ~= 0 and session ~= job.session then
        finish_server_remote_paste(job, false, nil, "server-world-session-changed-before-placement")
        return
    end
    if not job.started then
        local required = {}
        if clipboard and clipboard.hasVoxels then required[#required + 1] = "runtime.world.voxel.write" end
        if clipboard and #clipboard.props > 0 then
            required[#required + 1] = "runtime.world.entity.spawn"
            required[#required + 1] = "runtime.world.entity.set_scale"
        end
        if setting("targetPropMode") == "replace" then
            required[#required + 1] = "runtime.world.entity.query_props_in_bounds"
            required[#required + 1] = "runtime.world.entity.destroy"
        end
        local ready = session ~= 0
        for _, operation in ipairs(required) do if not runtime.has(operation) then ready = false end end
        if not ready then
            if job.elapsed >= 45 then finish_server_remote_paste(job, false, nil, "server-world-runtime-did-not-become-ready"); return end
            return
        end
        job.session = session
        job.started = true
        local before = undo_state
        paste_voxels(false, nil, nil, job.context)
        job.initial_undo = before
    end
    if undo_state and undo_state ~= job.initial_undo then
        local transaction = "tx_" .. tostring(job.request_id):gsub("[^%w_-]", "")
        server_remote_transaction = {peer = job.peer, transaction = transaction, session = job.session}
        if undo_state.recovery_required then
            finish_server_remote_paste(job, false, transaction, "placement-partially-applied; press F4 for server recovery")
        else
            finish_server_remote_paste(job, true, transaction, "placement-confirmed")
        end
    elseif pending_prop_capture or pending_world_action then
        return
    else
        finish_server_remote_paste(job, false, nil, editor_hint or "server-native-placement-was-not-confirmed")
    end
end

local function update_server_remote_undo(delta_seconds)
    local job = server_remote_undo
    if not job then return end
    job.elapsed = job.elapsed + math.max(0, tonumber(delta_seconds) or 0)
    local transaction = server_remote_transaction
    if not transaction or transaction.peer ~= job.peer or transaction.transaction ~= job.transaction or
       transaction.session ~= current_session() then
        server_remote_undo = nil
        job.done(false, "server-world-session-changed-or-undo-token-expired")
        return
    end
    if undo_state and undo_state.hasVoxels and
       (not runtime.has("runtime.world.voxel.read") or not runtime.has("runtime.world.voxel.write")) then
        if job.elapsed >= 45 then
            server_remote_undo = nil
            job.done(false, "server-voxel-runtime-did-not-become-ready")
        end
        return
    end
    if not undo_state then
        server_remote_transaction, server_remote_undo = nil, nil
        job.done(true, "undo-already-complete")
        return
    end
    if not job.started then
        undo_voxels()
        job.started = true
    end
    if not undo_state then
        server_remote_transaction, server_remote_undo = nil, nil
        job.done(true, "undo-confirmed")
    elseif pending_world_action then
        if job.elapsed >= 45 then
            server_remote_undo = nil
            job.done(false, "server-undo-world-context-did-not-become-ready")
        end
    else
        server_remote_undo = nil
        job.done(false, editor_hint or "server-undo-needs-recovery; retry F4")
    end
end

local function create_server_peer_list()
    return server_peer_ids()
end

p2p_bridge = create_p2p_bridge()

feature = function(feature_name)
    if runtime.has(feature_name) then return true end
    local status = runtime.status(feature_name)
    set_editor_message(nil, "Waiting for " .. feature_name .. ". " .. tostring(status and status.reason or "Runtime feature unavailable."))
    shroudforge.log.warn("World Editor: " .. feature_name .. " unavailable: " ..
        tostring(status and status.reason or "feature unavailable"))
    return false
end

local function on_action(name, callback)
    if not editor_ui_available then return end
    shroudforge.ui.on_action(name, function(...)
        update_world_session()
        return callback(...)
    end)
end

on_action("copyVoxels", copy_voxels)
on_action("captureAndSave", capture_and_save)
on_action("capturePropsOnly", function() copy_props_only(false) end)
on_action("saveBlueprint", save_blueprint)
on_action("loadBlueprint", load_blueprint)
on_action("selectBlueprint", function(name)
    if type(name) ~= "string" then return end
    load_blueprint(name)
end)
on_action("newBlueprint", start_new_blueprint)
on_action("refreshBlueprintLibrary", refresh_blueprint_library)
on_action("renameLibraryBlueprint", rename_library_blueprint)
on_action("duplicateLibraryBlueprint", duplicate_library_blueprint)
on_action("deleteLibraryBlueprint", delete_library_blueprint)
on_action("pasteVoxels", paste_voxels)
on_action("previewPaste", preview_paste)
on_action("rotateBlueprint", rotate_blueprint)
on_action("pasteAtCursor", function() paste_voxels(true) end)
on_action("undoVoxels", undo_voxels)
on_action("markCursorNext", mark_cursor_next)
on_action("resetEditor", reset_editor)
on_action("spawnEntity", spawn_entity)
on_action("placeEntity", function() placement_operation(false) end)
on_action("destroyEntity", function() placement_operation(true) end)
on_action("destroySelectedProp", destroy_selected_prop)
on_action("markCursorA", function() mark_cursor("a") end)
on_action("markCursorB", function() mark_cursor("b") end)
on_action("markCursorTarget", function() mark_cursor("target") end)
on_action("clearCursorSelection", clear_cursor_selection)
on_action("listProps", list_props)
on_action("resolveRecipes", report_recipe_catalog)

return {
    update_interval_ms = 30,
    on_load = function()
        if editor_is_server then
            local status = runtime.network and runtime.network.status and runtime.network.status() or nil
            shroudforge.log.info("World Editor server target loaded. Client requests use Steam P2P and the server's native world runtime.")
            if status and status.local_steam_id then
                shroudforge.log.info("World Editor dedicated-server SteamID64: " .. tostring(status.local_steam_id))
            end
            if #configured_client_steam_ids() == 0 then
                shroudforge.log.info("World Editor server P2P authorization follows Enshrouded's authenticated connected-player list; no client SteamID64 entry is required.")
            else
                shroudforge.log.info("World Editor server P2P uses the explicitly configured allowedClientSteamIds list.")
            end
            if not status or not status.available then
                shroudforge.log.warn("World Editor server P2P is unavailable: " .. tostring(status and status.reason or "network runtime missing"))
            end
        else
            local backend = execution_backend()
            last_execution_backend = backend
            local target = backend == "p2p" and "Dedicated Server P2P" or
                backend == "direct" and "local / singleplayer direct" or "waiting for a validated world context"
            shroudforge.log.info("World Editor client target detection is automatic; current target: " .. target)
            if recover_server_undo_token() then
                shroudforge.log.warn("World Editor recovered a pending server undo token from the local journal")
                set_editor_message("recovery", backend == "p2p" and
                    "Recovered an unfinished server undo. Press F4 to resume it; F7 stays paused until the server confirms." or
                    "Recovered an unfinished server undo. Rejoin that Dedicated Server world and press F4 to resume it.")
            end
            local status = runtime.network and runtime.network.status and runtime.network.status() or nil
            if status and status.local_steam_id then
                shroudforge.log.info("World Editor client SteamID64 (add this to the server allowlist): " .. tostring(status.local_steam_id))
            end
            refresh_blueprint_library()
            publish_editor_state()
        end
    end,
    on_update = function(_delta_seconds)
        update_world_session()
        update_pending_queries(_delta_seconds)
        if p2p_bridge then
            p2p_bridge.tick(_delta_seconds, editor_is_server and create_server_peer_list or nil)
        end
        if editor_is_server then
            update_server_remote_paste(_delta_seconds)
            update_server_remote_undo(_delta_seconds)
            return
        end
        local backend = execution_backend()
        if backend ~= last_execution_backend then
            last_execution_backend = backend
            publish_editor_state()
            if backend == "direct" then
                shroudforge.log.info("World Editor automatic target selected: local native world")
            elseif backend == "p2p" then
                shroudforge.log.info("World Editor automatic target selected: dedicated server via Steam P2P")
            else
                shroudforge.log.debug("World Editor automatic target is waiting for a validated world session")
            end
        end
        if not readiness_logged then
            local props_ready = runtime.has("runtime.world.entity.query_props_in_bounds")
            local cursor_ready = runtime.has("runtime.world.cursor.get")
            local write_feature = "runtime.world.entity.spawn"
            local spawn_ready = runtime.has(write_feature)
            local read_ready = runtime.has("runtime.world.voxel.read")
            if props_ready and cursor_ready and spawn_ready and read_ready then
                readiness_logged = true
                shroudforge.log.info("World Editor native entity APIs ready: F3 rotate, F4 undo, F5 mark A/B, F6 reset, F7 paste, F8 capture/save.")
            elseif not readiness_wait_logged then
                readiness_wait_logged = true
                local props = runtime.status("runtime.world.entity.query_props_in_bounds")
                local cursor = runtime.status("runtime.world.cursor.get")
                local spawn = runtime.status(write_feature)
                local read = runtime.status("runtime.world.voxel.read")
                shroudforge.log.info("World Editor waiting for runtime readiness: cursor=" .. tostring(cursor and cursor.reason or "ready") ..
                    ", props=" .. tostring(props and props.reason or "ready") ..
                    ", spawn=" .. tostring(spawn and spawn.reason or "ready") ..
                    ", worldRead=" .. tostring(read and read.reason or "ready"))
            end
        end
        local rotate = key_pressed("F3")
        local undo = key_pressed("F4")
        local mark = key_pressed("F5")
        local reset = key_pressed("F6")
        local paste = key_pressed("F7")
        local capture = key_pressed("F8")
        if reset then reset_editor(); return end
        if rotate then rotate_blueprint() end
        if undo then undo_voxels() end
        if mark then mark_cursor_next() end
        if paste then paste_voxels(true) end
        if capture then capture_and_save() end
        publish_editor_state()
    end,
    on_unload = function()
        if save_progress.state == "saving" then
            save_step(save_progress.completed, "Save cancelled", "error")
        end
        pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
        live_prop_cache = {}
        shroudforge.log.debug("World Editor Lua runtime unloaded")
    end,
}
