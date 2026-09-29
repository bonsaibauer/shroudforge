-- Export-capable mods can also be loaded during the pregame asset pass,
-- where runtime-only namespaces may not be available.
if type(shroudforge) ~= "table" or type(shroudforge.ui) ~= "table" then return {} end

runtime.require("runtime.lifecycle")

local function setting(name)
    local value = shroudforge.settings.get(name)
    if value == nil then return "" end
    return value
end

local clipboard = nil
local placement_preview = nil
local undo_state = nil
local feature
local component_type
local selection_a = nil
local selection_b = nil
local selection_target = nil
local active_blueprint_name = nil
local live_prop_cache = {}
local placeable_items = nil
local live_component_types = nil
local pending_cursor_action = nil
local pending_prop_capture = nil
local pending_world_action = nil
local readiness_logged = false
local readiness_wait_logged = false
local maximum_blueprint_bytes = 32 * 1024 * 1024
local save_blueprint_named
local previous_key_state = {}
local observed_rotation_setting = nil
local active_rotation_turns = nil

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
        shroudforge.log.warn("World Editor: capture or load a blueprint before rotating it")
        return
    end
    active_rotation_turns = (rotation_turns() + 1) % 4
    placement_preview = nil
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local degrees = active_rotation_turns * 90
    shroudforge.log.info(string.format(
        "World Editor rotation: %d/3 quarter turns (%d degrees) around %s; F3 rotates again, F7 pastes, Preview checks the cursor target",
        active_rotation_turns, degrees, string.upper(axis)))
end

local function finite_number(value)
    return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function live_read(entity, component_name)
    local info = component_type and component_type(component_name)
    if not info then return nil end
    local value, reason = runtime.ecs.read(entity, info)
    if not value then return nil, reason end
    return value
end

local function unavailable_ecs_reason(operation)
    if runtime.has(operation) then return nil end
    local status = runtime.status(operation)
    return status and status.reason or (operation .. " is unavailable")
end

local function world_position(position)
    if not position then return nil end
    local scale = 4294967296
    local x, y, z = tonumber(position.x), tonumber(position.y), tonumber(position.z)
    if not finite_number(x) or not finite_number(y) or not finite_number(z) then return nil end
    return {x = x / scale, y = y / scale, z = z / scale}
end

local function rotated_blueprint(source, quarter_turns)
    local region = source.region
    local sx, sy, sz = region.sx, region.sy, region.sz
    local axis = region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local cells, props = source.cells, {}
    for index, prop in ipairs(source.props or {}) do
        props[index] = {
            itemId = prop.itemId, entityHandle = prop.entityHandle,
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
        local next_cells = {}
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
                end
            end
        end
        for _, prop in ipairs(props) do
            local x, y, z = prop.x, prop.y, prop.z
            if axis == "x" then prop.y, prop.z = z, sy * 0.5 - y
            elseif axis == "y" then prop.x, prop.z = z, sx * 0.5 - x
            else prop.x, prop.y = sy * 0.5 - y, x end
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
        sx, sy, sz, cells = next_sx, next_sy, next_sz, next_cells
    end
    return {sx = sx, sy = sy, sz = sz, cells = cells, props = props}
end

local function cursor_point()
    local reason = unavailable_ecs_reason("runtime.world.cursor.get")
    if reason then return nil, reason end
    local snapshot, reason = runtime.world.cursor.get()
    if not snapshot then return nil, reason end
    local value = snapshot.value
    local transform = value and value.primaryTransform
    local position = transform and world_position(transform.position)
    if position then return position end
    return nil, "native cursor snapshot has no readable primary transform"
end

local function query_is_pending(reason)
    return type(reason) == "string" and (
        reason:find("still scanning", 1, true) ~= nil or
        reason:find("has not published a live sample", 1, true) ~= nil or
        reason:find("waiting for the live Keen ECS world", 1, true) ~= nil or
        reason:find("KFC Runtime is not ready", 1, true) ~= nil)
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
            shroudforge.log.warn("World Editor is already waiting for the live world context")
            return false
        end
        pending_world_action = {operation = operation, action = action}
        shroudforge.log.debug("World Editor is waiting for the validated world context; the requested action will continue automatically")
        return false
    end
    feature(operation)
    return false
end

local function request_cursor_action(action)
    if pending_cursor_action then
        shroudforge.log.warn("World Editor is already waiting for its live cursor query")
        return
    end
    local point, reason = cursor_point()
    if point then action(point); return end
    if query_is_pending(reason) then
        pending_cursor_action = action
        shroudforge.log.debug("World Editor is resolving the live cursor; the requested action will continue automatically")
    else
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
    if type(value) == "table" then value = value.value or value.id end
    return tonumber(value)
end

local function intersects_recipe(position, transform, recipe, minimum, maximum)
    local bounds = recipe.bounds
    local center = {(bounds[1] + bounds[4]) / 2, (bounds[2] + bounds[5]) / 2, (bounds[3] + bounds[6]) / 2}
    local scale_value = transform.scale or {x = 1, y = 1, z = 1}
    local scale = {tonumber(scale_value.x) or 1, tonumber(scale_value.y) or 1, tonumber(scale_value.z) or 1}
    for _, value in ipairs(scale) do if not finite_number(value) then return false end end
    local half = {
        math.abs((bounds[4] - bounds[1]) * 0.5 * scale[1]),
        math.abs((bounds[5] - bounds[2]) * 0.5 * scale[2]),
        math.abs((bounds[6] - bounds[3]) * 0.5 * scale[3]),
    }
    local q = transform.orientation or {}
    local x, y, z, w = tonumber(q.x) or 0, tonumber(q.y) or 0, tonumber(q.z) or 0, tonumber(q.w) or 1
    if not finite_number(x) or not finite_number(y) or not finite_number(z) or not finite_number(w) then return false end
    local norm = x*x + y*y + z*z + w*w
    local matrix = {{1,0,0},{0,1,0},{0,0,1}}
    if norm > 1e-12 then
        local s = 2 / norm
        matrix = {
            {1-s*(y*y+z*z), s*(x*y-z*w), s*(x*z+y*w)},
            {s*(x*y+z*w), 1-s*(x*x+z*z), s*(y*z-x*w)},
            {s*(x*z-y*w), s*(y*z+x*w), 1-s*(x*x+y*y)},
        }
    end
    local scaled_center = {center[1]*scale[1], center[2]*scale[2], center[3]*scale[3]}
    local world_center, world_half = {position.x, position.y, position.z}, {0,0,0}
    for row = 1, 3 do
        for column = 1, 3 do
            world_center[row] = world_center[row] + matrix[row][column] * scaled_center[column]
            world_half[row] = world_half[row] + math.abs(matrix[row][column]) * half[column]
        end
    end
    if world_half[1] + world_half[2] + world_half[3] < 1e-6 then world_half = {0.25,0.25,0.25} end
    return world_center[1]+world_half[1] >= minimum[1] and world_center[1]-world_half[1] < maximum[1] and
        world_center[2]+world_half[2] >= minimum[2] and world_center[2]-world_half[2] < maximum[2] and
        world_center[3]+world_half[3] >= minimum[3] and world_center[3]-world_half[3] < maximum[3]
end

local function resolve_placeable_items()
    if placeable_items then return placeable_items end
    local resolved = {}
    local ok, resources = pcall(game.assets.get_resources_by_type, "keen::ItemInfo")
    if not ok or type(resources) ~= "table" then
        shroudforge.log.warn("World Editor could not read ItemInfo assets: " .. tostring(resources))
        return nil
    end
    for _, resource in ipairs(resources) do
        local data = resource.data
        local equipment = data and data.equipment
        local id = data and item_id(data.itemId)
        local reference = equipment and equipment.placedEntity
        local high, low = guid_halves(reference)
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
        if id and id > 0 and id % 1 == 0 and high and low and valid_bounds then
            local feedback
            for _, collider in ipairs(equipment.placementColliders or {}) do
                for _, entry in ipairs(collider.dataArray or {}) do
                    local entry_value = entry["$value"] or entry.value or entry
                    local candidate = entry_value.materialFeedbackId
                    feedback = item_id(candidate)
                    if feedback and feedback ~= 0 then break end
                end
                if feedback and feedback ~= 0 then break end
            end
            if feedback and feedback ~= 0 then
                resolved[id] = {
                    id = id, uuidHigh = high, uuidLow = low, feedback = feedback,
                    bounds = bounds,
                    name = data.debugName or tostring(id),
                }
            end
        end
    end
    placeable_items = resolved
    local count = 0
    for _ in pairs(resolved) do count = count + 1 end
    shroudforge.log.debug("World Editor resolved " .. count .. " placeable ItemInfo recipes from current game assets")
    return resolved
end

local function voxel_region(source, use_current_cursor, cursor_override)
    local names = source and {"sourceX", "sourceY", "sourceZ"} or {"targetX", "targetY", "targetZ"}
    local x, y, z
    local sx, sy, sz
    if source and (selection_a or selection_b) then
        if not selection_a or not selection_b then
            return nil, "mark both selection corners at the cursor, or clear the cursor selection"
        end
        local ax, ay, az = math.floor(selection_a.x * 2 + 0.5), math.floor(selection_a.y * 2 + 0.5), math.floor(selection_a.z * 2 + 0.5)
        local bx, by, bz = math.floor(selection_b.x * 2 + 0.5), math.floor(selection_b.y * 2 + 0.5), math.floor(selection_b.z * 2 + 0.5)
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
            x, y, z = math.floor(point.x * 2 + 0.5), math.floor(point.y * 2 + 0.5), math.floor(point.z * 2 + 0.5)
        else
            x, y, z = tonumber(setting(names[1])), tonumber(setting(names[2])), tonumber(setting(names[3]))
        end
    end
    if not sx then sx, sy, sz = tonumber(setting("sizeX")), tonumber(setting("sizeY")), tonumber(setting("sizeZ")) end
    if not source and clipboard then
        local turns = rotation_turns()
        local axis = clipboard.region.rotationAxis or setting("rotationAxis")
        sx, sy, sz = clipboard.region.sx, clipboard.region.sy, clipboard.region.sz
        if turns % 2 == 1 then
            if axis == "x" then sy, sz = sz, sy
            elseif axis == "z" then sx, sy = sy, sx
            else sx, sz = sz, sx end
        end
    end
    if not x or not y or not z or not sx or not sy or not sz or
       x % 1 ~= 0 or y % 1 ~= 0 or z % 1 ~= 0 or
       sx % 1 ~= 0 or sy % 1 ~= 0 or sz % 1 ~= 0 or
       sx < 1 or sy < 1 or sz < 1 or sx > 256 or sy > 256 or sz > 256 or
       sx * sy * sz > 65536 then
        return nil, "coordinates must be integers and the region must contain at most 65,536 cells"
    end
    return {x = x, y = y, z = z, sx = sx, sy = sy, sz = sz}
end

local function capture_region_props(region)
    local unavailable = unavailable_ecs_reason("runtime.world.entity.query_props")
    if unavailable then return nil, unavailable end
    local recipes = resolve_placeable_items()
    if not recipes then return nil end
    local minimum = {region.x / 2, region.y / 2, region.z / 2}
    local maximum = {(region.x + region.sx) / 2, (region.y + region.sy) / 2, (region.z + region.sz) / 2}
    -- Query prop pivots far enough outside the voxel box to include recipes
    -- whose rotated placement bounds overlap the selected region. Lua applies
    -- the exact per-recipe rotated-box test below.
    local query_margin = 0
    for _, recipe in pairs(recipes) do
        local bounds = recipe.bounds
        if type(bounds) == "table" and #bounds >= 6 then
            local radius = math.sqrt(math.max(math.abs(bounds[1]), math.abs(bounds[4]))^2 +
                math.max(math.abs(bounds[2]), math.abs(bounds[5]))^2 +
                math.max(math.abs(bounds[3]), math.abs(bounds[6]))^2)
            if finite_number(radius) then query_margin = math.max(query_margin, radius) end
        end
    end
    query_margin = math.max(query_margin, 0.25)
    local live_props, reason = runtime.world.entity.query_props({minimum[1], minimum[2], minimum[3],
        maximum[1], maximum[2], maximum[3]}, query_margin)
    if not live_props then
        if not query_is_pending(reason) then
            shroudforge.log.warn("World Editor prop enumeration failed: " .. tostring(reason))
        end
        return nil, reason
    end
    if #live_props > 100000 then
        shroudforge.log.warn("World Editor refused prop capture: native query exceeded the 100,000-prop safety limit")
        return nil
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
            if intersects_recipe(position, transform, recipe, minimum, maximum) then
                live_prop_cache[live_prop.handle] = {
                    itemId = id, position = {position.x, position.y, position.z},
                    qx = qx, qy = qy, qz = qz, qw = qw, sx = sx, sy = sy, sz = sz,
                }
                props[#props + 1] = {
                    itemId = id, entityHandle = live_prop.handle,
                    x = position.x - minimum[1], y = position.y - minimum[2], z = position.z - minimum[3],
                    qx = qx, qy = qy, qz = qz, qw = qw,
                    sx = sx, sy = sy, sz = sz,
                }
            end
        end
    end
    return props
end

local function request_prop_capture(region, callback)
    if pending_prop_capture then
        shroudforge.log.warn("World Editor is already waiting for a live prop query")
        return
    end
    local props, reason = capture_region_props(region)
    if props then callback(props); return end
    if query_is_pending(reason) then
        pending_prop_capture = {region = region, callback = callback}
        shroudforge.log.debug("World Editor is scanning live props; the requested action will continue automatically")
        return
    end
    shroudforge.log.warn("World Editor could not capture props: " .. tostring(reason or "live prop query unavailable"))
end

local function update_pending_queries()
    if pending_world_action then
        local pending = pending_world_action
        if runtime.has(pending.operation) then
            pending_world_action = nil
            pending.action()
        else
            local status = runtime.status(pending.operation)
            local reason = status and status.reason
            if not world_feature_pending(reason) then
                pending_world_action = nil
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
            shroudforge.log.warn("World Editor cursor query stopped: " .. tostring(reason))
        end
    end
    if pending_prop_capture then
        local pending = pending_prop_capture
        local props, reason = capture_region_props(pending.region)
        if props then
            pending_prop_capture = nil
            pending.callback(props)
        elseif not query_is_pending(reason) then
            pending_prop_capture = nil
            shroudforge.log.warn("World Editor prop query stopped: " .. tostring(reason or "live prop query unavailable"))
        end
    end
end

local function count_prop_at(item, position)
    local unavailable = unavailable_ecs_reason("runtime.world.entity.query_props")
    if unavailable then return nil, unavailable end
    local epsilon = 0.01
    local props, reason = runtime.world.entity.query_props({position[1] - epsilon, position[2] - epsilon, position[3] - epsilon,
        position[1] + epsilon, position[2] + epsilon, position[3] + epsilon}, 0)
    if not props then return nil, reason end
    if #props > 100000 then return nil, "native prop query exceeded the 100,000-prop safety limit" end
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

local function contains_handle(handles, expected)
    for _, handle in ipairs(handles or {}) do
        if handle == expected then return true end
    end
    return false
end

local function same_cells(left, right)
    if type(left) ~= "table" or type(right) ~= "table" or #left ~= #right then return false end
    for index = 1, #left do if left[index] ~= right[index] then return false end end
    return true
end

local function newly_matching_handles(before, after)
    local result = {}
    if type(before) ~= "table" or type(after) ~= "table" then return result end
    for _, handle in ipairs(after or {}) do
        if not contains_handle(before, handle) then result[#result + 1] = handle end
    end
    return result
end

local function finish_copy_voxels(region, cells, props, save_and_select)
    local rotation_axis = setting("rotationAxis")
    if rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z" then rotation_axis = "y" end
    region.rotationAxis = rotation_axis
    clipboard = {region = region, cells = cells, props = props}
    local occupied = 0
    for _, value in ipairs(cells) do if value ~= 0 then occupied = occupied + 1 end end
    shroudforge.log.info(string.format("World Editor captured %d verified voxel cells (%d occupied) and %d resolved props from %d,%d,%d",
        #cells, occupied, #props, region.x, region.y, region.z))
    if save_and_select then
        local name
        for index = 1, 999999 do
            local candidate = "capture-" .. tostring(index)
            local path = "world-editor/blueprints/" .. candidate .. ".sfbp"
            local ok, exists = pcall(io.export_exists, path)
            if not ok then
                shroudforge.log.error("World Editor cannot safely choose a capture name because export storage could not be inspected: " .. tostring(exists))
                return
            end
            if exists == false then name = candidate; break end
        end
        if not name or not save_blueprint_named(name) then
            shroudforge.log.error("World Editor captured the region but could not save a unique persistent blueprint")
            return
        end
        active_blueprint_name = name
        shroudforge.log.info("World Editor saved and selected blueprint " .. name)
    end
end

local function copy_voxels(save_and_select)
    if not require_world_feature("runtime.world.voxel.read", function() copy_voxels(save_and_select) end) then return end
    local region, reason = voxel_region(true)
    if not region then shroudforge.log.warn("World Editor: " .. reason); return end
    local cells, read_reason = runtime.world.voxel.read(region.x, region.y, region.z,
        region.sx, region.sy, region.sz)
    if not cells then
        runtime.report_effect("waiting", read_reason or "voxel read failed")
        shroudforge.log.warn("World Editor voxel copy failed: " .. tostring(read_reason))
        return
    end
    request_prop_capture(region, function(props)
        finish_copy_voxels(region, cells, props, save_and_select)
    end)
end

local function blueprint_path(name)
    name = name or setting("blueprintName")
    if name == "" or not name:match("^[%w_-]+$") then
        return nil, "blueprint name may contain only letters, digits, underscores, and hyphens"
    end
    return "world-editor/blueprints/" .. name .. ".sfbp"
end

save_blueprint_named = function(name)
    if not clipboard then shroudforge.log.warn("World Editor: copy a voxel region before saving a blueprint"); return end
    local path, reason = blueprint_path(name)
    if not path then shroudforge.log.warn("World Editor: " .. reason); return end
    local region, values = clipboard.region, clipboard.cells
    local body = {"SHROUDFORGE_WORLD_BLUEPRINT_V4",
        table.concat({region.sx, region.sy, region.sz}, ","),
        region.rotationAxis or "y", table.concat(values, ","), tostring(#(clipboard.props or {}))}
    for _, prop in ipairs(clipboard.props or {}) do
        body[#body + 1] = table.concat({prop.itemId, prop.x, prop.y, prop.z,
            prop.qx, prop.qy, prop.qz, prop.qw, prop.sx, prop.sy, prop.sz}, ",")
    end
    local content = table.concat(body, "\n")
    if #content > maximum_blueprint_bytes then
        shroudforge.log.warn("World Editor: blueprint exceeds the 32 MiB file limit")
        return
    end
    local ok, err = pcall(io.export, path, content)
    if not ok then
        shroudforge.log.error("World Editor could not save persistent blueprint (enable export): " .. tostring(err))
        return false
    end
    if type(content) ~= "string" or #content > maximum_blueprint_bytes then
        shroudforge.log.warn("World Editor: blueprint is not text or exceeds the 32 MiB file limit")
        return false
    end
    shroudforge.log.info("World Editor saved persistent voxel-and-prop blueprint: " .. path)
    return true
end

local function save_blueprint()
    local name = setting("blueprintName")
    if save_blueprint_named(name) then active_blueprint_name = name end
end

local function load_blueprint()
    local path, reason = blueprint_path()
    if not path then shroudforge.log.warn("World Editor: " .. reason); return end
    local ok, content = pcall(io.read_export_to_string, path)
    if not ok then
        shroudforge.log.warn("World Editor could not load blueprint (enable export and verify the file): " .. tostring(content))
        return
    end
    local lines = {}
    for line in (content .. "\n"):gmatch("([^\n]*)\n") do lines[#lines + 1] = line end
    local version = lines[1]
    local version_four = version == "SHROUDFORGE_WORLD_BLUEPRINT_V4"
    local version_three = version == "SHROUDFORGE_WORLD_BLUEPRINT_V3"
    local dimensions = lines[2]
    local rotation_axis = version_four and lines[3] or "y"
    local data_line = version_four and 4 or 3
    local encoded = lines[data_line]
    if (not version_four and not version_three) or
       (rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z") or
       not dimensions or not encoded then
        shroudforge.log.warn("World Editor: blueprint format is invalid")
        return
    end
    local sx, sy, sz = dimensions:match("^(%d+),(%d+),(%d+)$")
    sx, sy, sz = tonumber(sx), tonumber(sy), tonumber(sz)
    if not sx or not sy or not sz or sx < 1 or sy < 1 or sz < 1 or sx > 256 or sy > 256 or sz > 256 or sx * sy * sz > 65536 then
        shroudforge.log.warn("World Editor: blueprint dimensions exceed supported limits")
        return
    end
    local cells = {}
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
    local props = {}
    local count_line = data_line + 1
    local count = tonumber(lines[count_line])
    if not count or count % 1 ~= 0 or count < 0 or count > 100000 or #lines ~= count_line + count then
        shroudforge.log.warn("World Editor: blueprint prop count is invalid")
        return
    end
    for index = 1, count do
        local fields = {}
        local field_count, invalid_field = 0, false
        for field in (lines[index + count_line] .. ","):gmatch("(.-),") do
            field_count = field_count + 1
            fields[field_count] = tonumber(field)
            if not finite_number(fields[field_count]) then invalid_field = true end
        end
        local recipes = resolve_placeable_items()
        if field_count ~= 11 or invalid_field or not fields[1] or fields[1] < 1 or
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
        props[#props + 1] = {itemId = fields[1], x = fields[2], y = fields[3], z = fields[4],
            qx = fields[5], qy = fields[6], qz = fields[7], qw = fields[8],
            sx = fields[9], sy = fields[10], sz = fields[11]}
    end
    clipboard = {region = {x = 0, y = 0, z = 0, sx = sx, sy = sy, sz = sz, rotationAxis = rotation_axis}, cells = cells, props = props}
    active_blueprint_name = setting("blueprintName")
    shroudforge.log.info(string.format("World Editor loaded persistent blueprint '%s' (%d cells, %d props)", active_blueprint_name, #cells, #props))
end

local function paste_voxels(use_current_cursor, cursor_override, captured_props, target_override)
    if not require_world_feature("runtime.world.voxel.write", function()
        paste_voxels(use_current_cursor, cursor_override, captured_props, target_override)
    end) then return end
    if not clipboard then shroudforge.log.warn("World Editor: copy a voxel region before pasting"); return end
    local target, reason
    if target_override then target = target_override
    else target, reason = voxel_region(false, use_current_cursor, cursor_override) end
    if not target and use_current_cursor and query_is_pending(reason) then
        request_cursor_action(function(point)
            paste_voxels(true, point)
        end)
        return
    end
    if not target then shroudforge.log.warn("World Editor: " .. reason); return end
    local turns = rotation_turns()
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local preview = placement_preview
    local same_target = preview and preview.target.x == target.x and preview.target.y == target.y and
        preview.target.z == target.z and preview.target.sx == target.sx and
        preview.target.sy == target.sy and preview.target.sz == target.sz
    local rotated = same_target and preview.blueprint == clipboard and preview.turns == turns and
        preview.axis == axis and preview.plan or rotated_blueprint(clipboard, turns)
    local source = rotated
    if source.sx ~= target.sx or source.sy ~= target.sy or source.sz ~= target.sz then
        shroudforge.log.warn("World Editor: copied region dimensions changed; copy the region again")
        return
    end
    for _, prop in ipairs(rotated.props or {}) do
        if math.abs(prop.sx - 1) > 1e-6 or math.abs(prop.sy - 1) > 1e-6 or math.abs(prop.sz - 1) > 1e-6 then
            shroudforge.log.warn("World Editor: this KFC Runtime build's native spawn operation supports unit scale only; no paste was applied")
            return
        end
    end
    local previous_cells, read_reason = runtime.world.voxel.read(target.x, target.y, target.z,
        target.sx, target.sy, target.sz)
    if not previous_cells then
        shroudforge.log.warn("World Editor could not snapshot the target before paste: " .. tostring(read_reason))
        return
    end
    local paste_cells = {}
    local additive = setting("pasteVoxelMode") == "add"
    for index, value in ipairs(rotated.cells) do
        paste_cells[index] = additive and value == 0 and previous_cells[index] or value
    end
    local removed_props = {}
    if setting("targetPropMode") == "replace" then
        if not feature("runtime.world.entity.destroy") then return end
        if not captured_props then
            request_prop_capture(target, function(props)
                paste_voxels(false, nil, props, target)
            end)
            return
        end
        local props = captured_props
        local recipes = resolve_placeable_items()
        for _, prop in ipairs(props) do
            if math.abs(prop.sx - 1) > 1e-6 or math.abs(prop.sy - 1) > 1e-6 or math.abs(prop.sz - 1) > 1e-6 then
                shroudforge.log.warn("World Editor cannot replace target props with non-unit scale because undo cannot restore their scale")
                return
            end
            local recipe = recipes and recipes[prop.itemId]
            if not recipe then
                shroudforge.log.warn("World Editor cannot replace a target prop without a current ItemInfo recipe")
                return
            end
            local position = {target.x / 2 + prop.x, target.y / 2 + prop.y, target.z / 2 + prop.z}
            local rotation = {prop.qx, prop.qy, prop.qz, prop.qw}
            local count, handles = count_prop_at(prop.itemId, position)
            if count ~= 1 or not contains_handle(handles, prop.entityHandle) then
                shroudforge.log.warn("World Editor refused replace-props paste because a target prop is stale or spatially ambiguous")
                return
            end
            removed_props[#removed_props + 1] = {
                recipe = recipe, position = position, rotation = rotation, entityHandle = prop.entityHandle,
            }
        end
    end
    undo_state = {
        region = target, cells = previous_cells, expected_cells = previous_cells,
        entities = {}, entity_index = 0, removed_props = {}, removed_prop_index = 0,
        voxel_written = false, recovery_required = true,
    }
    for _, prop in ipairs(removed_props) do
        local removed, remove_reason = runtime.world.entity.destroy(prop.position, prop.rotation,
            prop.recipe.bounds, prop.recipe.id, prop.recipe.feedback)
        local count_after, handles_after = count_prop_at(prop.recipe.id, prop.position)
        local handle_gone = count_after ~= nil and not contains_handle(handles_after, prop.entityHandle)
        if handle_gone then
            undo_state.removed_props[#undo_state.removed_props + 1] = prop
            undo_state.removed_prop_index = #undo_state.removed_props
        end
        if count_after == nil or not handle_gone or count_after ~= 0 then
            runtime.report_effect("write-failed", remove_reason or "target prop removal was not verified")
            shroudforge.log.warn("World Editor stopped before voxel paste because a target prop could not be safely removed")
            return
        end
    end
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
        return
    end
    undo_state.expected_cells = paste_cells
    undo_state.voxel_written = true
    local spawned = {}
    local origin = {target.x / 2, target.y / 2, target.z / 2}
    for _, prop in ipairs(rotated.props or {}) do
        local recipe = resolve_placeable_items() and resolve_placeable_items()[prop.itemId]
        if not recipe then
            shroudforge.log.error("World Editor stopped after voxel paste: no current placement recipe for item " .. tostring(prop.itemId))
            undo_state.entities, undo_state.entity_index = spawned, #spawned
            return
        end
        local position = {origin[1] + prop.x, origin[2] + prop.y, origin[3] + prop.z}
        local rotation = {prop.qx, prop.qy, prop.qz, prop.qw}
        local count_before, before_handles = count_prop_at(recipe.id, position)
        if count_before == nil then
            undo_state.entities, undo_state.entity_index = spawned, #spawned
            runtime.report_effect("write-failed", before_handles or "could not snapshot the prop count before spawn")
            shroudforge.log.error("World Editor stopped after voxel paste because prop state could not be snapshotted")
            return
        end
        local token, spawn_reason = runtime.world.entity.spawn(recipe.uuidHigh, recipe.uuidLow,
            position, rotation, recipe.id, 0)
        if not token then
            local count_after, after_handles = count_prop_at(recipe.id, position)
            if count_after and count_after > count_before then
                local new_handles = newly_matching_handles(before_handles, after_handles)
                spawned[#spawned + 1] = {recipe = recipe, position = position, rotation = rotation,
                    token = nil, entityHandle = #new_handles == 1 and new_handles[1] or nil}
            end
            runtime.report_effect("write-failed", spawn_reason or "entity spawn was not verified")
            shroudforge.log.error("World Editor stopped after partial paste; entity spawn failed for item " ..
                tostring(prop.itemId) .. ": " .. tostring(spawn_reason))
            undo_state.entities, undo_state.entity_index = spawned, #spawned
            return
        end
        local matching_count, after_handles = count_prop_at(recipe.id, position)
        local new_handles = matching_count ~= nil and newly_matching_handles(before_handles, after_handles) or {}
        if not matching_count or matching_count <= count_before or #new_handles ~= 1 then
            spawned[#spawned + 1] = {recipe = recipe, position = position, rotation = rotation,
                token = token, entityHandle = #new_handles == 1 and new_handles[1] or nil}
            undo_state.entities, undo_state.entity_index = spawned, #spawned
            local detail = after_handles or "spawn returned but Lua could not uniquely identify its new live ECS handle"
            runtime.report_effect("write-failed", detail)
            shroudforge.log.error("World Editor stopped after spawn because it could not track the new prop: " .. tostring(detail))
            return
        end
        spawned[#spawned + 1] = {recipe = recipe, position = position, rotation = rotation,
            token = token, entityHandle = new_handles[1]}
    end
    undo_state.entities, undo_state.entity_index = spawned, #spawned
    undo_state.recovery_required = false
    placement_preview = nil
    runtime.report_effect("write-confirmed", string.format("Wrote and read back %d voxel cells and verified %d new props in live ECS; save persistence is not verified",
        #clipboard.cells, #spawned))
    shroudforge.log.info(string.format("World Editor wrote %d voxel cells and verified %d spawned props at %d,%d,%d",
        #clipboard.cells, #spawned, target.x, target.y, target.z))
end

local function undo_voxels()
    if not undo_state then shroudforge.log.warn("World Editor: there is no verified world paste to undo"); return end
    if not require_world_feature("runtime.world.voxel.write", undo_voxels) or
       not require_world_feature("runtime.world.voxel.read", undo_voxels) then return end
    local region = undo_state.region
    local current_cells, read_reason = runtime.world.voxel.read(region.x, region.y, region.z,
        region.sx, region.sy, region.sz)
    if not current_cells then
        shroudforge.log.warn("World Editor paused undo because the pasted voxel region could not be checked: " .. tostring(read_reason))
        return
    end
    local expected_cells = undo_state.voxel_written and undo_state.expected_cells or undo_state.cells
    if not same_cells(current_cells, expected_cells) then
        shroudforge.log.warn("World Editor paused undo because the target voxels changed after the paste; no later changes were overwritten")
        return
    end
    local entities = undo_state.entities or {}
    local index = undo_state.entity_index or #entities
    while index >= 1 do
        local entity = entities[index]
        local current_count, current_handles = count_prop_at(entity.recipe.id, entity.position)
        if current_count == nil then
            runtime.report_effect("write-failed", current_handles or "could not inspect pasted props before undo")
            shroudforge.log.warn("World Editor paused undo because live prop state could not be inspected")
            return
        end
        if current_count == 0 or (entity.entityHandle and not contains_handle(current_handles, entity.entityHandle)) then
            index = index - 1
            undo_state.entity_index = index
        else
            if not entity.entityHandle then
                local detail = "the pasted entity has no unique live ECS handle; undo cannot safely target it"
                runtime.report_effect("write-failed", detail)
                shroudforge.log.warn("World Editor paused undo to avoid deleting an unrelated prop or rolling back only the voxels")
                return
            end
            if current_count ~= 1 then
                local detail = "multiple matching props share this transform; the spatial native API cannot safely target one for undo"
                runtime.report_effect("write-failed", detail)
                shroudforge.log.warn("World Editor paused undo because the pasted prop cannot be identified uniquely")
                return
            end
            local removed, remove_reason = runtime.world.entity.destroy(entity.position, entity.rotation,
                entity.recipe.bounds, entity.recipe.id, entity.recipe.feedback)
            local after_count, after_handles = count_prop_at(entity.recipe.id, entity.position)
            if after_count ~= nil and not contains_handle(after_handles, entity.entityHandle) and
               (removed or after_count < current_count) then
                index = index - 1
                undo_state.entity_index = index
            else
                local detail = remove_reason or after_handles or "the exact pasted ECS handle is still present after the destroy call"
                runtime.report_effect("write-failed", detail)
                shroudforge.log.error("World Editor paused undo and preserved its progress: " .. tostring(detail))
                return
            end
        end
    end
    if undo_state.voxel_written then
        local ok, reason = runtime.world.voxel.write(region.x, region.y, region.z,
            region.sx, region.sy, region.sz, undo_state.cells)
        if not ok then
            runtime.report_effect("write-failed", reason or "voxel undo failed")
            shroudforge.log.error("World Editor voxel undo failed: " .. tostring(reason))
            return
        end
        undo_state.voxel_written = false
        undo_state.expected_cells = undo_state.cells
    end
    local removed_index = undo_state.removed_prop_index or 0
    while removed_index >= 1 do
        local prop = undo_state.removed_props[removed_index]
        if not feature("runtime.world.entity.spawn") then return end
        local before_count, before_reason = count_prop_at(prop.recipe.id, prop.position)
        if before_count == nil then
            shroudforge.log.error("World Editor paused undo because replaced target props could not be inspected: " .. tostring(before_reason))
            return
        end
        if before_count ~= 0 then
            shroudforge.log.error("World Editor paused undo because a prop now occupies a position reserved for a replaced target prop")
            return
        end
        local token, spawn_reason = runtime.world.entity.spawn(prop.recipe.uuidHigh, prop.recipe.uuidLow,
            prop.position, prop.rotation, prop.recipe.id, 0)
        local after_count, after_reason = count_prop_at(prop.recipe.id, prop.position)
        if after_count == nil or after_count ~= 1 then
            shroudforge.log.error("World Editor paused undo while restoring a replaced target prop: " ..
                tostring(spawn_reason or after_reason or token or "spawn could not be uniquely verified"))
            return
        end
        removed_index = removed_index - 1
        undo_state.removed_prop_index = removed_index
    end
    undo_state = nil
    runtime.report_effect("write-confirmed", "Restored the previous voxel region and removed the verified pasted props")
    shroudforge.log.info("World Editor restored the voxel snapshot and removed props captured before the last paste")
end

local function mark_cursor(which)
    request_cursor_action(function(point)
        if which == "a" then selection_a = point
        elseif which == "b" then selection_b = point
        else selection_target = point end
        shroudforge.log.info(string.format("World Editor cursor selection %s = %.3f, %.3f, %.3f",
            which == "target" and "TARGET" or which:upper(), point.x, point.y, point.z))
    end)
end

local function preview_paste_at(point)
    if not clipboard then shroudforge.log.warn("World Editor: copy or load a blueprint before previewing"); return end
    local target, reason = voxel_region(false, true, point)
    if not target then shroudforge.log.warn("World Editor preview: " .. tostring(reason)); return end
    local turns = rotation_turns()
    local axis = clipboard.region.rotationAxis or setting("rotationAxis")
    if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
    local plan = rotated_blueprint(clipboard, turns)
    local occupied = 0
    for _, cell in ipairs(plan.cells) do if cell ~= 0 then occupied = occupied + 1 end end
    if plan.sx ~= target.sx or plan.sy ~= target.sy or plan.sz ~= target.sz then
        shroudforge.log.warn("World Editor preview rejected: rotated blueprint dimensions do not match the target region")
        return
    end
    placement_preview = {blueprint = clipboard, target = target, turns = turns, axis = axis, plan = plan}
    shroudforge.log.info(string.format(
        "World Editor placement plan prepared: axis=%s turns=%d target=%d,%d,%d size=%d,%d,%d occupied=%d props=%d; no world changes made",
        axis, turns, target.x, target.y, target.z, plan.sx, plan.sy, plan.sz, occupied, #plan.props))
end

local function preview_paste()
    if not clipboard then shroudforge.log.warn("World Editor: copy or load a blueprint before previewing"); return end
    request_cursor_action(preview_paste_at)
end

local function clear_cursor_selection()
    selection_a, selection_b, selection_target = nil, nil, nil
    shroudforge.log.info("World Editor cursor marks cleared; manual coordinates are active")
end

local function mark_cursor_next()
    request_cursor_action(function(point)
        if not selection_a or selection_b then
            selection_a, selection_b = point, nil
            shroudforge.log.info(string.format("World Editor selection A = %.3f, %.3f, %.3f; press F5 at the opposite corner for B",
                point.x, point.y, point.z))
        else
            selection_b = point
            shroudforge.log.info(string.format("World Editor selection B = %.3f, %.3f, %.3f; region is ready for F8 capture",
                point.x, point.y, point.z))
        end
    end)
end

local function reset_editor()
    if undo_state and undo_state.recovery_required then
        shroudforge.log.warn("World Editor reset refused: undo the incomplete paste with F4 before clearing editor state")
        return
    end
    selection_a, selection_b, selection_target = nil, nil, nil
    pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
    clipboard, undo_state, active_blueprint_name = nil, nil, nil
    live_prop_cache = {}
    placement_preview = nil
    shroudforge.log.info("World Editor reset: selection, active blueprint, and undo history cleared")
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

local function destroy_selected_prop()
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
    local count, handles = count_prop_at(item, {position.x, position.y, position.z})
    if count == nil or count ~= 1 or not contains_handle(handles, handle) then
        shroudforge.log.warn("World Editor cannot safely delete this selection: its handle is stale or the spatial target is ambiguous")
        return
    end
    local rotation = {selected.qx, selected.qy, selected.qz, selected.qw}
    local ok, reason = runtime.world.entity.destroy(
        {position.x, position.y, position.z}, rotation, recipe.bounds, recipe.id, recipe.feedback)
    if not ok then
        local remaining, remaining_handles = count_prop_at(item, {position.x, position.y, position.z})
        if remaining == nil or contains_handle(remaining_handles, handle) then
            runtime.report_effect("write-failed", reason or "selected prop removal was not verified")
            shroudforge.log.error("World Editor could not remove selected prop handle " .. tostring(handle) .. ": " .. tostring(reason))
            return
        end
    end
    local remaining, remaining_handles = count_prop_at(item, {position.x, position.y, position.z})
    if remaining == nil or contains_handle(remaining_handles, handle) then
        runtime.report_effect("write-failed", "the selected live ECS handle remains after the destroy call")
        shroudforge.log.error("World Editor could not confirm removal of the selected prop handle")
        return
    end
    runtime.report_effect("write-confirmed", "The selected live ECS handle disappeared; save persistence is not verified")
    shroudforge.log.info("World Editor removed the selected prop at its live transform; persistence is not verified")
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
    if not feature("runtime.world.entity.spawn") then return end
    local tracking = tonumber(setting("trackingId"))
    if not tracking or tracking < 1 or tracking % 1 ~= 0 then
        shroudforge.log.warn("World Editor: enter a nonzero placement tracking ID")
        return
    end
    local position, rotation = entity_transform()
    local token, reason = runtime.world.entity.spawn(setting("templateUuidHigh"), setting("templateUuidLow"),
        position, rotation, tracking, 0)
    if not token then
        runtime.report_effect("waiting", reason or "native spawn was not dispatched")
        shroudforge.log.warn("World Editor spawn failed: " .. tostring(reason))
        return
    end
    runtime.report_effect("write-confirmed", "Spawn was matched to a live ECS CurrentTransform and UsedItem record; token " .. tostring(token) .. " is not an entity handle")
    shroudforge.log.info("World Editor verified the spawned prop in live ECS; engine queue token=" .. tostring(token) .. "; save persistence is not verified")
end

local function placement_operation(destroy)
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
    local before_count, before_reason = count_prop_at(tracking, position)
    if before_count == nil then
        shroudforge.log.warn("World Editor cannot dispatch this operation without a readable pre-operation prop count: " .. tostring(before_reason))
        return
    end
    if destroy and before_count == 0 then
        runtime.report_effect("no-target", "No matching live prop exists at these coordinates")
        shroudforge.log.warn("World Editor found no matching live prop to destroy")
        return
    end
    local ok, reason
    if destroy then
        ok, reason = runtime.world.entity.destroy(position, rotation, bounds, tracking, feedback or 0)
    else
        ok, reason = runtime.world.entity.place(position, rotation, bounds, tracking, feedback)
    end
    local after_count, after_reason = count_prop_at(tracking, position)
    if after_count == nil then
        runtime.report_effect("no-change", "Native operation returned " .. tostring(ok) .. "; live ECS result could not be read: " .. tostring(after_reason))
        shroudforge.log.warn("World Editor operation is unconfirmed because the live ECS could not be read: " .. tostring(after_reason))
        return
    end
    if destroy then
        if after_count < before_count then
            runtime.report_effect("write-confirmed", "Matching live prop count decreased; save persistence is not verified")
            shroudforge.log.info("World Editor verified the native prop removal in the live ECS")
        else
            runtime.report_effect("write-failed", reason or "native destroy call did not reduce the matching live prop count")
            shroudforge.log.error("World Editor destroy did not remove a matching live prop")
        end
    else
        if after_count > before_count then
            runtime.report_effect("write-confirmed", "Matching live prop count increased after placement; save persistence is not verified")
            shroudforge.log.info("World Editor verified native placement in the live ECS")
        else
            runtime.report_effect("no-change", reason or "placement returned without a matching live prop appearing")
            shroudforge.log.warn("World Editor placement was dispatched but no matching live prop was observed")
        end
    end
end

feature = function(feature_name)
    if runtime.has(feature_name) then return true end
    local status = runtime.status(feature_name)
    shroudforge.log.warn("World Editor: " .. feature_name .. " unavailable: " ..
        tostring(status and status.reason or "feature unavailable"))
    return false
end

component_type = function(name)
    if name == "" then return nil end
    if not live_component_types then
        local components = runtime.ecs.get_components()
        if type(components) ~= "table" then return nil end
        live_component_types = {}
        for _, info in ipairs(components) do
            if info.qualified_name then live_component_types[info.qualified_name] = info end
        end
    end
    return live_component_types[name]
end

local function field_names(type_info)
    local names = {}
    for name, field in pairs(type_info.struct_fields or {}) do
        local field_type = field.type
        names[#names + 1] = string.format("%s:%s@%s", name,
            field_type and field_type.qualified_name or "?", tostring(field.data_offset))
    end
    table.sort(names)
    return table.concat(names, ", ")
end

local function discover_types()
    if not feature("runtime.ecs.query") then return end
    local components, reason = runtime.ecs.get_components()
    if not components then
        shroudforge.log.warn("World Editor: component discovery failed: " .. tostring(reason))
        return
    end
    table.sort(components, function(left, right)
        return left.qualified_name < right.qualified_name
    end)
    shroudforge.log.debug("World Editor discovered " .. #components .. " live ECS component types")
    for index, type_info in ipairs(components) do
        shroudforge.log.trace(string.format("World Editor type %d/%d: %s size=%d fields={%s}",
            index, #components, type_info.qualified_name, type_info.size, field_names(type_info)))
    end
end

local function query_entities()
    if not feature("runtime.ecs.query") then return end
    local name = setting("queryComponent")
    if name == "" or not component_type(name) then
        shroudforge.log.warn("World Editor: enter an exact component name from the discovery log")
        return
    end
    local entities, reason = runtime.ecs.query(name)
    if not entities then
        shroudforge.log.warn("World Editor: entity query failed: " .. tostring(reason))
        return
    end
    shroudforge.log.debug("World Editor found " .. #entities .. " matching entities for " .. name)
    for index = 1, math.min(#entities, 100) do
        shroudforge.log.trace(string.format("World Editor entity %d: handle=%d", index, entities[index]))
    end
    if #entities > 100 then
        shroudforge.log.debug("World Editor: entity list truncated at 100 handles")
    end
end

local function read_selected_component()
    if not feature("runtime.ecs.read") then return nil end
    local handle = tonumber(setting("entityHandle"))
    local name = setting("componentType")
    if not handle or handle < 1 or handle % 1 ~= 0 or name == "" then
        shroudforge.log.warn("World Editor: enter a live opaque entity handle and exact component type")
        return nil
    end
    local info = component_type(name)
    if not info then
        shroudforge.log.warn("World Editor: component type is not present in this game's reflection data: " .. name)
        return nil
    end
    local value, reason = runtime.ecs.read(handle, info)
    if not value then
        shroudforge.log.warn("World Editor: component read failed: " .. tostring(reason))
        return nil
    end
    return handle, name, info, value
end

local function inspect_component()
    local handle, name, info = read_selected_component()
    if not handle then return end
    shroudforge.log.debug(string.format("World Editor component: handle=%d type=%s size=%d fields={%s}",
        handle, name, info.size, field_names(info)))
end

local function split_path(path)
    local result = {}
    for part in path:gmatch("[^%.]+") do result[#result + 1] = part end
    return result
end

local function unwrap_type(info)
    while info and info.primitive_type == "Typedef" do
        info = info.inner_type
    end
    return info
end

local function parse_primitive(text, info)
    info = unwrap_type(info)
    if not info then return nil, "field type metadata is missing" end
    local primitive = info.primitive_type
    if primitive == "Bool" then
        if text == "true" then return true end
        if text == "false" then return false end
        return nil, "boolean values must be true or false"
    end
    if primitive == "UInt8" or primitive == "UInt16" or primitive == "UInt32" or
       primitive == "UInt64" or primitive == "SInt8" or primitive == "SInt16" or
       primitive == "SInt32" or primitive == "SInt64" or primitive == "Enum" then
        local value = tonumber(text)
        if not value or value % 1 ~= 0 then return nil, "integer or enum value required" end
        return value
    end
    if primitive == "Float32" or primitive == "Float64" then
        local value = tonumber(text)
        if not value then return nil, "numeric value required" end
        return value
    end
    return nil, "only reflected boolean, integer, enum, and floating-point fields can be edited"
end

local function write_field()
    if not feature("runtime.ecs.write") then return end
    local handle, name, info, value = read_selected_component()
    if not handle then return end
    local path = split_path(setting("fieldPath"))
    if #path == 0 then
        shroudforge.log.warn("World Editor: enter a reflected field path, for example position.x")
        return
    end

    local parent = value
    local parent_type = info
    for index = 1, #path do
        local fields = parent_type and parent_type.struct_fields
        local field = fields and fields[path[index]]
        if not field then
            shroudforge.log.warn("World Editor: reflected field not found: " .. path[index])
            return
        end
        if index == #path then
            local parsed, reason = parse_primitive(setting("fieldValue"), field.type)
            if parsed == nil then
                shroudforge.log.warn("World Editor: " .. tostring(reason))
                return
            end
            local ok, assignment_error = pcall(function() parent[path[index]] = parsed end)
            if not ok then
                shroudforge.log.warn("World Editor: field assignment rejected: " .. tostring(assignment_error))
                return
            end
        else
            parent = parent[path[index]]
            parent_type = field.type
            if parent == nil then
                shroudforge.log.warn("World Editor: nested field is unavailable: " .. path[index])
                return
            end
        end
    end

    local ok, reason = runtime.ecs.write(handle, info, value)
    if not ok then
        runtime.report_effect("write-failed", tostring(reason))
        shroudforge.log.error("World Editor: live ECS write failed: " .. tostring(reason))
        return
    end
    runtime.report_effect("write-confirmed", string.format("Wrote %s on entity handle %d; runtime verified the typed-memory write", setting("fieldPath"), handle))
    shroudforge.log.info(string.format("World Editor wrote %s.%s on entity handle %d", name, setting("fieldPath"), handle))
end

shroudforge.ui.on_action("discoverTypes", discover_types)
shroudforge.ui.on_action("copyVoxels", copy_voxels)
shroudforge.ui.on_action("captureAndSave", function() copy_voxels(true) end)
shroudforge.ui.on_action("saveBlueprint", save_blueprint)
shroudforge.ui.on_action("loadBlueprint", load_blueprint)
shroudforge.ui.on_action("pasteVoxels", paste_voxels)
shroudforge.ui.on_action("previewPaste", preview_paste)
shroudforge.ui.on_action("rotateBlueprint", rotate_blueprint)
shroudforge.ui.on_action("pasteAtCursor", function() paste_voxels(true) end)
shroudforge.ui.on_action("undoVoxels", undo_voxels)
shroudforge.ui.on_action("markCursorNext", mark_cursor_next)
shroudforge.ui.on_action("resetEditor", reset_editor)
shroudforge.ui.on_action("spawnEntity", spawn_entity)
shroudforge.ui.on_action("placeEntity", function() placement_operation(false) end)
shroudforge.ui.on_action("destroyEntity", function() placement_operation(true) end)
shroudforge.ui.on_action("destroySelectedProp", destroy_selected_prop)
shroudforge.ui.on_action("queryEntities", query_entities)
shroudforge.ui.on_action("markCursorA", function() mark_cursor("a") end)
shroudforge.ui.on_action("markCursorB", function() mark_cursor("b") end)
shroudforge.ui.on_action("markCursorTarget", function() mark_cursor("target") end)
shroudforge.ui.on_action("clearCursorSelection", clear_cursor_selection)
shroudforge.ui.on_action("listProps", list_props)
shroudforge.ui.on_action("resolveRecipes", report_recipe_catalog)
shroudforge.ui.on_action("inspectComponent", inspect_component)
shroudforge.ui.on_action("writeField", write_field)

return {
    update_interval_ms = 30,
    on_load = function()
        shroudforge.log.info("World Editor loaded; checking live ECS and world readiness")
    end,
    on_update = function(_delta_seconds)
        if not readiness_logged then
            local ecs_ready = runtime.has("runtime.world.entity.query_props")
            local cursor_ready = runtime.has("runtime.world.cursor.get")
            local world_ready = runtime.has("runtime.world.context.active") and
                runtime.has("runtime.world.voxel.read") and runtime.has("runtime.world.voxel.write")
            if ecs_ready and cursor_ready and world_ready then
                readiness_logged = true
                shroudforge.log.info("World Editor ready: F3 rotate blueprint, F4 undo, F5 mark A/B, F6 reset, F7 paste at cursor, F8 capture/save/select.")
            elseif not readiness_wait_logged then
                readiness_wait_logged = true
                local ecs = runtime.status("runtime.world.entity.query_props")
                local cursor = runtime.status("runtime.world.cursor.get")
                local world = runtime.status("runtime.world.context.active")
                shroudforge.log.info("World Editor waiting for runtime readiness: cursor=" .. tostring(cursor and cursor.reason or "ready") ..
                    "; ECS=" .. tostring(ecs and ecs.reason or "ready") ..
                    "; world=" .. tostring(world and world.reason or "ready"))
            end
        end
        local rotate = key_pressed("F3")
        local undo = key_pressed("F4")
        local mark = key_pressed("F5")
        local reset = key_pressed("F6")
        local paste = key_pressed("F7")
        local capture = key_pressed("F8")
        if reset then reset_editor(); return end
        update_pending_queries()
        if rotate then rotate_blueprint() end
        if undo then undo_voxels() end
        if mark then mark_cursor_next() end
        if paste then paste_voxels(true) end
        if capture then copy_voxels(true) end
    end,
    on_unload = function()
        pending_cursor_action, pending_prop_capture, pending_world_action = nil, nil, nil
        live_component_types = nil
        live_prop_cache = {}
    shroudforge.log.debug("World Editor Lua runtime unloaded")
    end,
}
