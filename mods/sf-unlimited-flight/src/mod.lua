local overrides = runtime.overrides.new()
local pitch_overrides = runtime.overrides.new()
local active_session
local last_state, last_detail
local gliders = {}
local peak_flight_delta = 0
local upward_recovery = false

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
    if not actor then return nil, reason end
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
    if not finite(item_id) or item_id <= 0 then return nil, "Player is not actively using a glider" end
    return item_id
end

local function pitch_command(entity, delta_seconds)
    if not finite(delta_seconds) or delta_seconds <= 0 then
        return nil, "Glider steering requires a positive measured update interval"
    end
    local mode, reason = runtime.ecs.read_fields(entity, "keen::ecs::PlayerInput",
        {"fromClient.movementStickInputMode"})
    if not mode then return nil, reason end
    local value = mode["fromClient.movementStickInputMode"]
    if value ~= "YawSpeedPitchSpeed" and value ~= 0 then
        return nil, "Upward tilt requires pitch-speed input controls. Target-angle controls are not supported"
    end
    local input, why = runtime.ecs.read_fields(entity, "keen::ecs::ActorInput", {"desiredLocalMoveInput.z"})
    if not input then return nil, why end
    local command = input["desiredLocalMoveInput.z"]
    if not finite(command) or math.abs(command) > 1.001 then return nil, "Glider pitch input is unavailable" end
    command = math.max(-1, math.min(1, command))
    local prefix = "transform.orientation."
    local rotation, failure = runtime.ecs.read_fields(entity, "keen::ecs::CurrentTransform",
        {prefix .. "x", prefix .. "y", prefix .. "z", prefix .. "w"})
    if not rotation then return nil, failure end
    local gravity, error = runtime.ecs.read_fields(entity, "keen::ecs::Gravity",
        {"direction.x", "direction.y", "direction.z"})
    if not gravity then return nil, error end
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
    -- asin folds back into [-90,90] after crossing the vertical. Keep that
    -- folded angle from restarting a negative rate while climb stays held.
    if upright <= 0 then upward_recovery = true end
    if upright > 0 and command >= 0 then upward_recovery = false end
    peak_flight_delta = math.max(peak_flight_delta, delta_seconds)
    local horizon = math.max(0.25, 2 * peak_flight_delta)
    -- This margin uses measured runtime intervals. A resource rate has no
    -- expiry, so an arbitrarily longer future Lua pause is still unbounded.
    return {input=command, recovery=upward_recovery,
        upward_rate_limit=math.max(0, (pitch + 85) / horizon)}
end

local function update(delta_seconds)
    local session = runtime.world.session_id()
    if session == 0 then
        peak_flight_delta, upward_recovery = 0, false
        report("waiting", "Waiting for an authoritative world") return
    end
    if active_session and session ~= active_session then
        gliders = {}
        peak_flight_delta, upward_recovery = 0, false
    end
    active_session = session
    overrides:begin_update()
    pitch_overrides:begin_update()

    overrides:restore_disabled(setting_enabled)
    pitch_overrides:restore_disabled(setting_enabled)
    if not setting_enabled("unlimitedFlying") then
        peak_flight_delta, upward_recovery = 0, false
    end
    if overrides.fault then report("write-failed", overrides.fault) return end
    if pitch_overrides.fault then report("write-failed", pitch_overrides.fault) return end
    if not has_effect() then
        report(overrides:has_ownership() and "waiting" or "no-change",
            overrides:summary("Unlimited Flying and all speed multipliers are at their defaults"))
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
        peak_flight_delta, upward_recovery = 0, false
        pitch_overrides:restore_all()
        report("no-target", "No live player entity is available") return
    end

    local pitch_detail = ""
    for _, entity in ipairs(players) do
        if setting_enabled("unlimitedGliderStamina") then
            local target, failure = runtime.overrides.attribute(entity, "StaminaDepletionMod_Glider")
            if target then
                overrides:apply(target.key, "unlimitedGliderStamina", target, function() return 0 end)
            else overrides:waiting("Glider stamina: " .. tostring(failure)) end
        end

        local item_id, item_reason = active_item(entity)
        if item_id then
            local resource, resource_reason = current_glider(item_id)
            if resource then
                if setting_enabled("unlimitedFlying") then
                    if #players == 1 then
                        local command, failure = pitch_command(entity, delta_seconds)
                        if command then
                            if command.recovery then
                                pitch_detail = " | inverted recovery: release climb to resume"
                            end
                            local target, why = runtime.overrides.resource(resource, pitch_path)
                            if target then
                                pitch_overrides:apply(target.key, "unlimitedFlying", target, function(_, baseline)
                                    if not finite(baseline) or baseline <= 0 then return nil, "A positive original pitch rate is required" end
                                    -- A positive original rate steers back toward
                                    -- the engine's bounded native target.
                                    if command.recovery then return baseline end
                                    if command.input < 0 then return -math.min(baseline * -command.input, command.upward_rate_limit) end
                                    if command.input == 0 then return 0 end
                                    return baseline
                                end)
                            else pitch_overrides:waiting(tostring(why)) end
                        else pitch_overrides:waiting(tostring(failure)) end
                    else pitch_overrides:waiting("Upward tilt requires one authoritative player because glider ItemInfo is shared") end
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
                overrides:waiting("Glider settings: " .. tostring(resource_reason))
            end
        elseif setting_enabled("unlimitedFlying") or setting_enabled("turnSpeed") or setting_enabled("moveSpeed")
            or setting_enabled("liftSpeed") or setting_enabled("descendSpeed") then
            peak_flight_delta, upward_recovery = 0, false
            overrides:waiting("Glider settings: " .. tostring(item_reason))
        end
    end

    pitch_overrides:restore_unused()

    local state = (overrides.fault or pitch_overrides.fault) and "write-failed" or
        (overrides.waiting_count + pitch_overrides.waiting_count > 0 and "waiting" or
        (overrides.writes + pitch_overrides.writes > 0 and "write-confirmed" or "no-change"))
    report(state, pitch_overrides:summary(overrides:summary("Native glider controls")) .. pitch_detail)
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
