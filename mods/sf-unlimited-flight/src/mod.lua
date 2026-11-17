local overrides = runtime.overrides.new()
local pitch_overrides = runtime.overrides.new()
local active_session
local last_state, last_detail
local gliders = {}
local upward_recovery = {}

local item_id_path = "currentAction.triggerContext.usedItem.itemId"
local pitch_path = "equipment.gliderConfig.pitchAngleSpeed"
local glider_fields = {
    {setting="turnSpeed", path="equipment.gliderConfig.yawAngleSpeed"},
    {setting="moveSpeed", path="equipment.gliderConfig.accelerationForward"},
    {setting="liftSpeed", path="equipment.gliderConfig.airResistanceLongitudinal"},
    {setting="descendSpeed", path="equipment.gliderConfig.airResistanceVertical"},
}

local function report(state, detail)
    if last_state == state and last_detail == detail then return end
    last_state, last_detail = state, detail
    runtime.report_effect(state, detail)
end

local function finite(value)
    return type(value) == "number" and value == value and math.abs(value) < math.huge
end

local function setting_enabled(key)
    if key == "unlimitedFlying" or key == "unlimitedGliderStamina" then
        return shroudforge.settings.get(key, false) == true
    end
    local value = shroudforge.settings.get(key, 100)
    return finite(value) and value ~= 100
end

local function multiplier(key)
    local value = shroudforge.settings.get(key, 100)
    if not finite(value) then return nil, "setting must be a finite percentage" end
    if value < 25 or value > 400 then return nil, "setting percentage must be between 25 and 400" end
    return value / 100
end

local function has_effect()
    if setting_enabled("unlimitedFlying") or setting_enabled("unlimitedGliderStamina") then return true end
    for _, field in ipairs(glider_fields) do
        if setting_enabled(field.setting) then return true end
    end
    return false
end

local function resource_identity(resource)
    return {guid=resource.guid, part=resource.part}
end

local function validate_glider(resource, item_id)
    if not resource or resource.loaded ~= true then return nil, "ItemInfo is not a loaded runtime resource" end
    local ok, data = pcall(function() return resource.data end)
    if not ok then return nil, tostring(data) end
    local resource_item_id = data and data.itemId
    if type(resource_item_id) == "table" or type(resource_item_id) == "userdata" then
        resource_item_id = resource_item_id.value
    end
    if not data or resource_item_id ~= item_id or not data.equipment or data.equipment.slot ~= "Glider" then
        return nil, "The active item is not a registered glider"
    end
    return data
end

local function current_glider(item_id)
    local key = tostring(item_id)
    if type(game.assets.resolve_loaded_resource) == "function" then
        local ok, resource, reason = pcall(game.assets.resolve_loaded_resource,
            "g38_items::ItemRegistry", item_id)
        if not ok then return nil, tostring(resource) end
        if resource then
            local data, failure = validate_glider(resource, item_id)
            if not data then return nil, failure end
            gliders[key] = resource_identity(resource)
            return resource
        end
        -- Older exact profiles may lack the relation extension. Those keep
        -- the common prepared index as their explicit compatible fallback.
        reason = tostring(reason or "Original item registry is unavailable")
        if not reason:find("lacks", 1, true) and not reason:find("contract is unavailable", 1, true) then
            return nil, reason
        end
    end

    -- Only the compatibility path retains an identity hint. The canonical engine
    -- relation is rechecked on every supported lookup, including map remaps.
    local identity = gliders[key]
    if identity then
        local ok, resource = pcall(game.assets.get_loaded_resource,
            identity.guid, "keen::ItemInfo", identity.part)
        if ok and resource then
            local data = validate_glider(resource, item_id)
            if data then return resource end
        end
        gliders[key] = nil
    end

    local ok, resource, reason, progress = pcall(game.assets.get_loaded_resource_by_field,
        "keen::ItemInfo", "itemId.value", item_id)
    if not ok then return nil, tostring(resource) end
    if not resource then
        if progress then
            return nil, tostring(reason or (progress.phase .. " " .. progress.scanned .. "/" .. progress.total))
        end
        return nil, tostring(reason or "Active glider resource is unavailable")
    end
    local data, failure = validate_glider(resource, item_id)
    if not data then return nil, failure end
    gliders[key] = resource_identity(resource)
    return resource
end

local function active_item(entity)
    local actor, reason = runtime.ecs.read_fields(entity, "keen::ecs::Actor", {
        item_id_path, "currentState", "currentAction.playbackContext.state",
        "currentAction.playbackContext.setStateMask", "currentAction.playbackContext.unsetStateMask"})
    if not actor then return nil, reason, true end
    local function has_flag(flags, name)
        if flags == nil then return false end
        for _, flag in ipairs(flags) do if flag == name then return true end end
        return false
    end
    -- The original state getter combines the active action's state masks.
    -- HangGliding need not exist in the stored currentState by itself.
    local active = has_flag(actor["currentAction.playbackContext.state"], "IsActive")
    local gliding = has_flag(actor.currentState, "HangGliding")
    if active then
        gliding = (gliding or has_flag(actor["currentAction.playbackContext.setStateMask"], "HangGliding"))
            and not has_flag(actor["currentAction.playbackContext.unsetStateMask"], "HangGliding")
    end
    if not gliding or not active then
        return nil, "Player is not actively using a glider"
    end
    local item_id = actor[item_id_path]
    if type(item_id) == "table" or type(item_id) == "userdata" then item_id = item_id.value end
    if not finite(item_id) or item_id <= 0 then return nil, "Active glider item ID is unavailable", true end
    return item_id
end

local function pitch_input(entity)
    local mode, reason = runtime.ecs.read_fields(entity, "keen::ecs::PlayerInput",
        {"fromClient.movementStickInputMode"})
    if not mode then return nil, reason end
    local value = mode["fromClient.movementStickInputMode"]
    local target_angle = value == "YawSpeedPitchTargetAngle" or value == 1
    if not target_angle and value ~= "YawSpeedPitchSpeed" and value ~= 0 then
        return nil, "Unsupported glider pitch input mode"
    end
    local input, why = runtime.ecs.read_fields(entity, "keen::ecs::ActorInput", {"desiredLocalMoveInput.z"})
    if not input then return nil, why end
    local command = input["desiredLocalMoveInput.z"]
    if not finite(command) or math.abs(command) > 1.001 then return nil, "Glider pitch input is unavailable" end
    return {input=math.max(-1, math.min(1, command)), target_angle=target_angle}
end

local function pitch_command(entity, input, delta_seconds)
    if not finite(delta_seconds) or delta_seconds <= 0 then
        return nil, "Glider steering requires a positive measured update interval"
    end
    local gravity, error = runtime.ecs.read_fields(entity, "keen::ecs::Gravity",
        {"direction.x", "direction.y", "direction.z"})
    if not gravity then return nil, error end
    -- Read the expected orientation last, immediately before its checked write.
    local prefix = "transform.orientation."
    local rotation, failure = runtime.ecs.read_fields(entity, "keen::ecs::CurrentTransform",
        {prefix .. "x", prefix .. "y", prefix .. "z", prefix .. "w"})
    if not rotation then return nil, failure end
    local x, y, z, w = rotation[prefix .. "x"], rotation[prefix .. "y"], rotation[prefix .. "z"], rotation[prefix .. "w"]
    local gx, gy, gz = gravity["direction.x"], gravity["direction.y"], gravity["direction.z"]
    if not (finite(x) and finite(y) and finite(z) and finite(w)
        and finite(gx) and finite(gy) and finite(gz)) then return nil, "Glider orientation or gravity is invalid" end
    local norm, length = x*x + y*y + z*z + w*w, math.sqrt(gx*gx + gy*gy + gz*gz)
    if norm < 0.99 or norm > 1.01 or length < 0.001 then return nil, "Glider orientation or gravity is not normalized" end
    local forward_x = 2 * (x*z + w*y) / norm
    local forward_y = 2 * (y*z - w*x) / norm
    local forward_z = 1 - 2 * (x*x + y*y) / norm
    local pitch = math.deg(math.asin(math.max(-1, math.min(1,
        (forward_x*gx + forward_y*gy + forward_z*gz) / length))))
    local up_x = 2 * (x*y - w*z) / norm
    local up_y = 1 - 2 * (x*x + z*z) / norm
    local up_z = 2 * (y*z + w*x) / norm
    local upright = -(up_x*gx + up_y*gy + up_z*gz) / length
    -- asin folds back into [-90,90] after crossing the vertical. Do not
    -- interpret an inverted basis as permission for another upward step.
    if upright <= 0 then upward_recovery[entity] = true end
    if upright > 0 and input.input >= 0 then upward_recovery[entity] = nil end
    return {input=input.input, target_angle=input.target_angle,
        recovery=upward_recovery[entity], pitch=pitch,
        rotation=rotation, x=x, y=y, z=z, w=w,
        -- World up cross forward is the horizontal pitch axis. Rotating
        -- around it preserves this player's heading and current bank.
        right_x=(-gy*forward_z + gz*forward_y) / length,
        right_y=(-gz*forward_x + gx*forward_z) / length,
        right_z=(-gx*forward_y + gy*forward_x) / length,
        delta_seconds=math.min(delta_seconds, 0.1)}
end

local function steer_pitch(entity, command, rate)
    if command.recovery then return true, nil, false end
    local step = rate * command.delta_seconds
    local delta = command.input * step
    if command.target_angle then
        -- The original target-angle branch maps input.z to input.z * pi/2.
        local desired_pitch = math.max(-85, math.min(80, command.input * 90))
        delta = math.max(-step, math.min(step, desired_pitch - command.pitch))
    end
    if delta < 0 then delta = math.min(0, math.max(delta, -85 - command.pitch))
    else delta = math.max(0, math.min(delta, 80 - command.pitch)) end
    if delta == 0 then return true, nil, false end

    local rx, ry, rz = command.right_x, command.right_y, command.right_z
    local axis_length = math.sqrt(rx*rx + ry*ry + rz*rz)
    if axis_length < 0.001 then return false, "Glider pitch axis is vertical" end
    local sine, cosine = math.sin(math.rad(delta) / 2) / axis_length,
        math.cos(math.rad(delta) / 2)
    rx, ry, rz = rx*sine, ry*sine, rz*sine
    local x, y, z, w = command.x, command.y, command.z, command.w
    local qx = cosine*x + rx*w + ry*z - rz*y
    local qy = cosine*y - rx*z + ry*w + rz*x
    local qz = cosine*z + rx*y - ry*x + rz*w
    local qw = cosine*w - rx*x - ry*y - rz*z
    local length = math.sqrt(qx*qx + qy*qy + qz*qz + qw*qw)
    local prefix = "transform.orientation."
    -- This is a movement step, not a persistent field override. Restoring an
    -- old transform on landing/unload would rotate a player back in time.
    -- The public writer checks this entity generation and the complete fresh
    -- component snapshot, including these expected orientation values.
    local ok, reason = runtime.ecs.write_fields(entity, "keen::ecs::CurrentTransform", {
        [prefix .. "x"]=qx/length, [prefix .. "y"]=qy/length,
        [prefix .. "z"]=qz/length, [prefix .. "w"]=qw/length,
    }, command.rotation)
    return ok, reason, ok
end

local function update(delta_seconds)
    local session = runtime.world.session_id()
    if session == 0 then
        upward_recovery = {}
        report("waiting", "Waiting for an authoritative world") return
    end
    if active_session and session ~= active_session then
        gliders = {}
        upward_recovery = {}
    end
    active_session = session
    overrides:begin_update()
    pitch_overrides:begin_update()

    overrides:restore_disabled(setting_enabled)
    pitch_overrides:restore_disabled(setting_enabled)
    if not setting_enabled("unlimitedFlying") then
        upward_recovery = {}
    end
    if overrides.fault then report("write-failed", overrides.fault) return end
    if pitch_overrides.fault then report("write-failed", pitch_overrides.fault) return end
    if not has_effect() then
        report((overrides:has_ownership() or pitch_overrides:has_ownership()) and "waiting" or "no-change",
            pitch_overrides:summary(overrides:summary("Unlimited Flying and all speed multipliers are at their defaults")))
        return
    end
    if not runtime.has("runtime.ecs.read") or not runtime.has("runtime.ecs.write") then
        report("waiting", "Waiting for typed ECS access and the authoritative write scope")
        return
    end

    local players, reason = runtime.ecs.query("keen::ecs::PlayerInput")
    if not players then
        pitch_overrides:restore_all()
        report("waiting", tostring(reason or "Player query is pending")) return
    end
    if #players == 0 then
        upward_recovery = {}
        pitch_overrides:restore_all()
        report("no-target", "No live player entity is available") return
    end

    local pitch_detail, pitch_writes = "", 0
    local active_players, pitch_groups, failed_items = {}, {}, {}
    local pitch_scan_incomplete = false
    for _, entity in ipairs(players) do
        if setting_enabled("unlimitedGliderStamina") then
            local target, failure = runtime.overrides.attribute(entity, "StaminaDepletionMod_Glider")
            if target then
                overrides:apply(target.key, "unlimitedGliderStamina", target, function() return 0 end)
            else overrides:waiting("Glider stamina: " .. tostring(failure)) end
        end

        local item_id, item_reason, unreadable = active_item(entity)
        if item_id then
            local resource, resource_reason = current_glider(item_id)
            if resource then
                if setting_enabled("unlimitedFlying") then
                    active_players[entity] = true
                    local target, why = runtime.overrides.resource(resource, pitch_path)
                    if target then
                        local group = pitch_groups[target.key]
                        if not group then
                            group = {target=target, item_id=item_id, players={}}
                            pitch_groups[target.key] = group
                        end
                        local input, failure = pitch_input(entity)
                        if input then
                            group.players[#group.players + 1] = {entity=entity, input=input}
                        else
                            -- A shared hold must not freeze a player whose
                            -- control mode this update cannot handle.
                            group.failed = true
                            pitch_overrides:waiting(tostring(failure))
                        end
                    else
                        failed_items[item_id] = true
                        pitch_overrides:waiting(tostring(why))
                    end
                end
                for _, field in ipairs(glider_fields) do
                    if setting_enabled(field.setting) then
                        local factor, why = multiplier(field.setting)
                        if factor then
                            -- Toolkit values are direct multipliers of these original coefficients.
                            -- The adapter captures a fresh snapshot before each field apply.
                            local target, failure = runtime.overrides.resource(resource, field.path)
                            if target then
                                overrides:apply(target.key, field.setting, target, function(snapshot, baseline)
                                    return baseline * factor
                                end)
                            else overrides:waiting(field.setting .. ": " .. tostring(failure)) end
                        else
                            overrides:waiting(field.setting .. ": " .. tostring(why))
                        end
                    end
                end
            elseif setting_enabled("unlimitedFlying") or setting_enabled("turnSpeed") or setting_enabled("moveSpeed")
                or setting_enabled("liftSpeed") or setting_enabled("descendSpeed") then
                failed_items[item_id] = true
                overrides:waiting("Glider settings: " .. tostring(resource_reason))
            end
        elseif setting_enabled("unlimitedFlying") or setting_enabled("turnSpeed") or setting_enabled("moveSpeed")
            or setting_enabled("liftSpeed") or setting_enabled("descendSpeed") then
            if unreadable then pitch_scan_incomplete = true end
            overrides:waiting("Glider settings: " .. tostring(item_reason))
        end
    end

    for _, group in pairs(pitch_groups) do
        local admitted = not pitch_scan_incomplete and not group.failed and not failed_items[group.item_id]
        if admitted and finite(delta_seconds) and delta_seconds > 0 then
            local rate
            local target = group.target
            local held = pitch_overrides:apply(target.key, "unlimitedFlying", target, function(_, baseline)
                if not finite(baseline) or baseline <= 0 then return nil, "A positive original pitch rate is required" end
                rate = baseline
                -- The unchanged original pitch calculation starts from each
                -- CurrentTransform every frame. Zero holds that pitch while
                -- native yaw and movement run. Every player shares this same
                -- resource value, and receives a separate ECS movement step.
                return 0
            end)
            if held then
                for _, player in ipairs(group.players) do
                    -- Resource admission can span several entities. Recheck
                    -- the action before applying this entity's movement step.
                    local item_id, item_reason, unreadable = active_item(player.entity)
                    if item_id == group.item_id then
                        local command, failure = pitch_command(player.entity, player.input, delta_seconds)
                        if command then
                            if command.recovery then
                                pitch_detail = " | inverted recovery: release climb when upright to resume"
                            end
                            local ok, error, changed = steer_pitch(player.entity, command, rate)
                            if not ok then pitch_overrides:waiting(tostring(error))
                            elseif changed then pitch_writes = pitch_writes + 1 end
                        else pitch_overrides:waiting(tostring(failure)) end
                    else
                        upward_recovery[player.entity] = nil
                        if unreadable then
                            pitch_overrides:waiting(tostring(item_reason))
                        end
                    end
                end
            end
        elseif admitted then
            pitch_overrides:waiting("Glider steering requires a positive measured update interval")
        end
    end
    for entity in pairs(upward_recovery) do
        if not active_players[entity] then upward_recovery[entity] = nil end
    end
    pitch_overrides:restore_unused()

    local state = (overrides.fault or pitch_overrides.fault) and "write-failed" or
        (overrides.waiting_count + pitch_overrides.waiting_count > 0 and "waiting" or
        (overrides.writes + pitch_overrides.writes + pitch_writes > 0 and "write-confirmed" or "no-change"))
    report(state, pitch_overrides:summary(overrides:summary("Native glider controls"))
        .. " | player-pitch-writes=" .. pitch_writes .. pitch_detail)
end

return {
    update_interval_ms = 100,
    on_load = function()
        if type(game.assets.prepare_loaded_index) == "function" then
            game.assets.prepare_loaded_index("keen::ItemInfo", "itemId.value")
        end
        report("waiting", "SF Unlimited Flight loaded. Waiting for typed engine access")
    end,
    on_update = update,
}
