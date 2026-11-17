local function equal(actual, expected, label)
    assert(actual == expected, (label or "value") .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
end

local function close(actual, expected, label)
    assert(math.abs(actual - expected) < 0.00001,
        (label or "value") .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
end

local function clone(value)
    if type(value) ~= "table" then return value end
    local copy = {}
    for key, item in pairs(value) do copy[key] = clone(item) end
    return copy
end

local function get_path(root, path)
    local value = root
    for part in path:gmatch("[^%.]+") do value = value[part] end
    return value
end

local function set_path(root, path, value)
    local parent = root
    local parts = {}
    for part in path:gmatch("[^%.]+") do parts[#parts + 1] = part end
    for i = 1, #parts - 1 do parent = parent[parts[i]] end
    parent[parts[#parts]] = value
end

local function fixture()
    local f = {
        settings={}, session=41, writes=0, players={11}, active_item=0,
        pitch_mode="YawSpeedPitchTargetAngle", pitch_input=0, pitch_degrees=0, delta_seconds=1/60,
        action_sets_gliding=false, action_unsets_gliding=false,
        attributes={[11]={StaminaDepletionMod_Glider=0.35}},
        trackers={}, player_states={}, orientations={}, transform_writes={},
        item_id=3357517884,
        glider={
            itemId={value=3357517884},
            equipment={slot="Glider",gliderConfig={
                yawAngleSpeed=55,
                pitchAngleSpeed=70,
                accelerationForward=1,
                airResistanceLongitudinal=string.unpack("f", string.pack("f", 0.155)),
                airResistanceVertical=3,
            }},
        },
    }
    local item_type = {qualified_name="keen::ItemInfo"}
    local scalar_type = {qualified_name="keen::float32"}
    local function player_value(entity, key)
        local player = f.player_states[entity]
        if player and player[key] ~= nil then return player[key] end
        return f[key]
    end
    local function orientation(entity)
        local current = f.orientations[entity]
        if current then return current end
        local angle = math.rad(player_value(entity, "pitch_degrees")) / 2
        return {x=math.sin(angle), y=0, z=0, w=math.cos(angle)}
    end
    local function resource()
        return {
            guid="80c9eaeb-7fa0-480d-a55f-91c2c51b383e",
            type=item_type, part=0, loaded=true, session_id=f.session, data=clone(f.glider),
        }
    end
    local overrides = assert(load(overrides_source, "runtime.overrides"))()
    local new_tracker = overrides.new
    overrides.new = function()
        local tracker = new_tracker(function() return f.session end)
        f.trackers[#f.trackers + 1] = tracker
        return tracker
    end
    local function claim()
        local bound_session = f.session
        return {
            acquire=function()
                if f.session ~= bound_session then return false, "claim belongs to another session" end
                return true
            end,
            release=function() end,
        }
    end
    overrides.get_component_claim = claim
    overrides.get_resource_claim = claim
    shroudforge = {settings={get=function(key, fallback)
        local value = f.settings[key]
        if value == nil then return fallback end
        return value
    end}}
    runtime = {
        overrides=overrides,
        has=function() return true end,
        report_effect=function(state, detail) f.state, f.detail = state, detail end,
        world={session_id=function() return f.session end},
        values={
            encode=function(_, value) return string.pack("f", value) end,
            decode=function(_, bytes) return string.unpack("f", bytes) end,
            normalize_field=function(_, _, value) return string.unpack("f", string.pack("f", value)) end,
        },
        ecs={
            query=function(name)
                equal(name, "keen::ecs::PlayerInput")
                return f.players
            end,
            get_attribute=function(selector) return {name=selector, root_hash=1} end,
            read_attributes=function(entity, selector)
                equal(selector, "StaminaDepletionMod_Glider")
                return {
                    entity=entity, root_hash=1, values=clone(f.attributes[entity]),
                    component={qualified_name="keen::ecs::StaminaDepletionMod"},
                }
            end,
            evaluate_attributes=function(field, values)
                equal(field, "StaminaDepletionMod_Glider")
                return {values=clone(values)}
            end,
            update_attribute=function(entity, field, value, expected)
                equal(type(expected), "table", "attribute expected value")
                equal(f.attributes[entity][field], expected[field], "attribute compare")
                f.attributes[entity][field] = value
                f.writes = f.writes + 1
                return {
                    entity=entity, root_hash=1, values=clone(f.attributes[entity]),
                    component={qualified_name="keen::ecs::StaminaDepletionMod"},
                }
            end,
            read_fields=function(entity, type_name, paths)
                if type_name == "keen::ecs::Actor" and f.actor_busy == entity then
                    return nil, "Actor snapshot is busy"
                end
                if type_name == "keen::ecs::ActorInput" and f.input_busy == entity then
                    return nil, "ActorInput snapshot is busy"
                end
                if type_name == "keen::ecs::Actor" then
                    equal(paths[1], "currentAction.triggerContext.usedItem.itemId")
                    local active_item = player_value(entity, "active_item")
                    local sets_gliding = player_value(entity, "action_sets_gliding")
                    return {[paths[1]]={value=active_item},
                        currentState=active_item > 0 and not sets_gliding and {"HangGliding"} or {},
                        ["currentAction.playbackContext.state"]=active_item > 0 and {"IsActive"} or {},
                        ["currentAction.playbackContext.setStateMask"]=sets_gliding and {"HangGliding"} or {},
                        ["currentAction.playbackContext.unsetStateMask"]=player_value(entity, "action_unsets_gliding") and {"HangGliding"} or {}}
                elseif type_name == "keen::ecs::PlayerInput" then
                    return {[paths[1]]=player_value(entity, "pitch_mode")}
                elseif type_name == "keen::ecs::ActorInput" then
                    return {[paths[1]]=player_value(entity, "pitch_input")}
                elseif type_name == "keen::ecs::CurrentTransform" then
                    local current = orientation(entity)
                    return {[paths[1]]=current.x, [paths[2]]=current.y, [paths[3]]=current.z, [paths[4]]=current.w}
                elseif type_name == "keen::ecs::Gravity" then
                    return {[paths[1]]=0, [paths[2]]=-1, [paths[3]]=0}
                end
                error("unexpected component " .. type_name)
            end,
            write_fields=function(entity, type_name, changes, expected)
                equal(type_name, "keen::ecs::CurrentTransform", "only the player's orientation is steered")
                if f.component_busy == true or f.component_busy == entity then
                    return false, "ecs-scene-frame-busy"
                end
                if f.compare_conflict == entity then
                    f:set_pitch(entity, 12)
                    f.compare_conflict = nil
                    return false, "expected field value changed"
                end
                local current = orientation(entity)
                local next_orientation = {}
                for _, axis in ipairs({"x", "y", "z", "w"}) do
                    local path = "transform.orientation." .. axis
                    equal(expected[path], current[axis], "orientation compare for player " .. entity)
                    equal(type(changes[path]), "number", "orientation field")
                    next_orientation[axis] = changes[path]
                end
                for path in pairs(changes) do
                    assert(path:match("^transform%.orientation%.[xyzw]$"), "unrelated transform field changed")
                end
                close(next_orientation.x^2 + next_orientation.y^2 + next_orientation.z^2 + next_orientation.w^2,
                    1, "movement orientation remains normalized")
                f.orientations[entity] = next_orientation
                f.transform_writes[entity] = (f.transform_writes[entity] or 0) + 1
                f.writes = f.writes + 1
                return true, nil, clone(changes)
            end,
        },
    }
    game = {assets={
        get_loaded_resource=function(guid, type_name, part)
            if f.read_busy then return nil, "ResourceService read gate is busy" end
            equal(guid, "80c9eaeb-7fa0-480d-a55f-91c2c51b383e")
            equal(type(type_name) == "table" and type_name.qualified_name or type_name, "keen::ItemInfo")
            equal(part, 0)
            return resource()
        end,
        get_loaded_resource_by_field=function(type_name, path, value)
            if f.read_busy then return nil, "ResourceService index gate is busy" end
            equal(type_name, "keen::ItemInfo")
            equal(path, "itemId.value")
            equal(value, f.item_id)
            return resource()
        end,
        read_loaded_field=function(snapshot, path)
            -- The returned resource owns the confirmed native snapshot.
            -- Reading it must continue to work while later live reads are busy.
            return get_path(snapshot.data, path), nil, scalar_type
        end,
        write_loaded_field=function(snapshot, path, value, expected)
            equal(snapshot.loaded, true)
            equal(get_path(f.glider, path), expected, "resource compare")
            set_path(f.glider, path, value)
            f.writes = f.writes + 1
            if f.block_after_write then f.read_busy = true end
            return resource()
        end,
    }}
    for name, target in pairs(override_targets_factory(runtime, game.assets)) do overrides[name] = target end
    f.mod = assert(load(flight_source, "flight"))()
    f.mod.on_load()
    function f:set_pitch(entity, degrees)
        local half = math.rad(degrees) / 2
        self.orientations[entity] = {x=math.sin(half), y=0, z=0, w=math.cos(half)}
    end
    function f:pitch(entity)
        local q = orientation(entity)
        return math.deg(math.atan(2 * q.x * q.w, 1 - 2 * q.x * q.x))
    end
    function f:tick()
        if self.cleanup_pending then return self:unload() end
        self.mod.on_update(self.delta_seconds)
    end
    function f:unload()
        self.cleanup_pending = true
        if self.mod.on_unload then self.mod.on_unload() end
        for _, tracker in ipairs(self.trackers) do
            tracker:begin_update()
            tracker:restore_all()
            assert(not tracker.fault, tracker.fault)
            assert(not tracker:has_ownership(), tracker:summary("API cleanup pending"))
        end
        self.cleanup_pending = false
    end
    return f
end

do
    local f = fixture()
    local preparations, resolutions = 0, 0
    local read = game.assets.get_loaded_resource
    game.assets.prepare_loaded_index = function(type_name, path)
        equal(type_name, "keen::ItemInfo")
        equal(path, "itemId.value")
        preparations = preparations + 1
        return false, "queued"
    end
    f.mod.on_load()
    equal(preparations, 1, "preparation is registered before any glider action")
    game.assets.resolve_loaded_resource = function(registry_name, item_id)
        equal(registry_name, "g38_items::ItemRegistry")
        equal(item_id, f.item_id)
        resolutions = resolutions + 1
        return read("80c9eaeb-7fa0-480d-a55f-91c2c51b383e", "keen::ItemInfo", 0)
    end
    game.assets.get_loaded_resource_by_field = function()
        error("a supported canonical relation must not scan every ItemInfo")
    end
    f.active_item, f.settings.turnSpeed = f.item_id, 200
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 110, "first action uses the canonical original relation")
    f:tick()
    equal(resolutions, 2, "each canonical lookup rechecks the registry rather than an old GUID hint")
    f:unload()
end

do
    local f = fixture()
    game.assets.resolve_loaded_resource = function()
        return nil, "original ItemRegistry is busy, unavailable or changed. Retry later"
    end
    game.assets.get_loaded_resource_by_field = function()
        error("a busy validated relation must not switch to a different selection contract")
    end
    f.active_item, f.settings.turnSpeed = f.item_id, 200
    f:tick()
    equal(f.writes, 0, "a busy original relation cannot authorize a guessed resource")
    equal(f.state, "waiting")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying, f.settings.turnSpeed = true, 200
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.action_sets_gliding = true
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "active action can supply the effective HangGliding state")
    close(f:pitch(11), -70 * f.delta_seconds, "effective action state enables the player's upward pitch")
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 110, "effective action state enables original glider controls")
    local pitch = f:pitch(11)
    f.action_unsets_gliding = true
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "unset mask takes precedence and restores native pitch")
    equal(f:pitch(11), pitch, "ending the action does not restore an old orientation")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "shared pitch rate remains independent of player input")
    close(f:pitch(11), -70 * f.delta_seconds, "upward command steers this player at the original rate")
    local upward_pitch, writes = f:pitch(11), f.writes
    f.pitch_input = 0
    f:tick()
    equal(f:pitch(11), upward_pitch, "neutral input holds the player's pitch")
    equal(f.writes, writes, "neutral input requires no repeated orientation write")
    f.pitch_input = 1
    f:tick()
    close(f:pitch(11), 0, "downward input independently steers the player down")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "descending never changes the shared rate")
    f.pitch_input = -1
    f:set_pitch(11, -84)
    f:tick()
    close(f:pitch(11), -85, "the finite movement step stops at the upward pitch limit")
    f:set_pitch(11, -86)
    f:tick()
    close(f:pitch(11), -86, "holding climb beyond the limit never adds further upward rotation")
    f.pitch_input = 1
    f:set_pitch(11, 79)
    f:tick()
    close(f:pitch(11), 80, "the finite movement step stops at the downward pitch limit")
    f.active_item = 0
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "landing automatically restores the native rate")
    close(f:pitch(11), 80, "landing preserves the current movement orientation")
    f.active_item, f.pitch_input = f.item_id, -1
    f:set_pitch(11, 0)
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0)
    f.settings.unlimitedFlying = false
    local current_pitch = f:pitch(11)
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "disabling restores the original resource rate")
    equal(f:pitch(11), current_pitch, "disabling does not rewind movement")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds, f.players = 0.1, {11, 12}
    f.player_states[12] = {pitch_input=1}
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "two players sharing one glider resource use one stable rate")
    close(f:pitch(11), -7, "first player climbs independently")
    close(f:pitch(12), 7, "second player descends independently with the same glider item")
    equal(f.transform_writes[11], 1)
    equal(f.transform_writes[12], 1)
    equal(f.writes, 3, "one shared resource write and one movement write per player")
    f.players, f.player_states[12].pitch_input = {12}, 0
    f:tick()
    close(f:pitch(11), -7, "a departed player is no longer steered")
    close(f:pitch(12), 7, "the remaining player's neutral input holds its own pitch")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "a departing player does not restore another player's shared glider")
    f.player_states[12].active_item = 0
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "the last gliding player landing restores the original shared rate")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode = f.item_id, "YawSpeedPitchSpeed"
    f.delta_seconds, f.players = 0.1, {}
    for i = 1, 16 do
        local entity = 100 + i
        f.players[i] = entity
        f.player_states[entity] = {pitch_input=i % 2 == 0 and 1 or -1}
    end
    f:tick()
    for i, entity in ipairs(f.players) do
        close(f:pitch(entity), i % 2 == 0 and 7 or -7, "all players receive their own simultaneous pitch command")
        equal(f.transform_writes[entity], 1, "one update for player " .. entity)
    end
    equal(f.writes, 17, "the player count does not introduce extra writes to the shared glider")
    f.players = {}
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "an empty world releases the native pitch override")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, 0, 0
    f.delta_seconds, f.players = 0.1, {11, 12}
    f.player_states[12] = {pitch_mode=1, pitch_input=-0.5}
    f:set_pitch(11, -20)
    for _ = 1, 8 do f:tick() end
    close(f:pitch(11), -20, "neutral pitch-speed input holds this player's current pitch")
    close(f:pitch(12), -45, "target-angle input reaches its own input.z times ninety degree target")
    f.player_states[12].pitch_input = 0
    f:tick()
    close(f:pitch(11), -20, "the target-angle player's input cannot affect the speed-mode player's orientation")
    close(f:pitch(12), -38, "neutral target-angle input returns toward zero at the original rate")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "mixed control modes share only the fixed native hold rate")
    f.player_states[12].pitch_mode = "UnknownPitchMode"
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "an unsupported player prevents a shared hold from freezing its controls")
    close(f:pitch(11), -20, "the blocked group does not apply partial pitch commands")
    close(f:pitch(12), -38, "unsupported input does not steer the affected player")
    equal(f.state, "waiting")
    f.player_states[12].pitch_mode = "YawSpeedPitchTargetAngle"
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "valid input resumes the shared hold with the original baseline")
    close(f:pitch(12), -31, "resumed target-angle steering does not replay missed movement")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds = 0.1
    local half_yaw = math.rad(90) / 2
    f.orientations[11] = {x=0, y=math.sin(half_yaw), z=0, w=math.cos(half_yaw)}
    f:tick()
    local q = f.orientations[11]
    local forward_x = 2 * (q.x*q.z + q.w*q.y)
    local forward_y = 2 * (q.y*q.z - q.w*q.x)
    local forward_z = 1 - 2 * (q.x*q.x + q.y*q.y)
    close(math.deg(math.asin(-forward_y)), -7, "pitch steering follows the player's heading instead of a fixed world X axis")
    close(forward_z, 0, "pitch steering preserves the current horizontal heading")
    close(forward_x, math.cos(math.rad(7)), "pitch steering keeps the original yaw")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds, f.players = 0.1, {11, 12}
    f.player_states[12] = {pitch_input=1}
    f:tick()
    f.input_busy = 12
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "an unreadable player's input releases its shared native hold")
    close(f:pitch(11), -7, "a shared group is not partly steered when another player's input is unavailable")
    close(f:pitch(12), 7, "an unreadable input cannot authorize orientation writes")
    f.input_busy = false
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0)
    f.actor_busy = 12
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "an unreadable player's actor cannot be silently excluded from shared-resource admission")
    close(f:pitch(11), -14)
    close(f:pitch(12), 14)
    f.actor_busy = false
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds = 0.1
    f:tick()
    f.glider.equipment.gliderConfig.pitchAngleSpeed = 95
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 95, "an external pitch rate change is preserved")
    close(f:pitch(11), -7, "an unowned native hold cannot authorize another movement step")
    f.settings.unlimitedFlying = false
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 95, "disable preserves the external resource change")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds, f.players = 0.1, {11, 12}
    f.player_states[12] = {pitch_input=1}
    f.component_busy = 11
    f:tick()
    close(f:pitch(11), 0, "a busy player transform is not changed")
    close(f:pitch(12), 7, "one player's busy write does not block the other player's command")
    equal(f.state, "waiting", "busy movement remains retryable")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "a failed movement step leaves no negative shared rate")
    f.component_busy = false
    f:tick()
    close(f:pitch(11), -7, "the next update retries from the current pose without replaying missed movement")
    close(f:pitch(12), 14, "the other player's steering continues independently")
    f.compare_conflict = 11
    f:tick()
    close(f:pitch(11), 12, "a compare conflict preserves the engine's intervening orientation")
    close(f:pitch(12), 21, "a compare conflict is isolated to the affected player")
    f:unload()
    close(f:pitch(11), 12, "unload never restores a stale movement orientation")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70)
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.pitch_degrees, f.delta_seconds = -80, 0.65
    f:tick()
    close(f:pitch(11), -85, "a slow update cannot move past the sampled upward limit")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "Lua pauses cannot leave a persistent negative native rate")
    f:set_pitch(11, 0)
    f:tick()
    close(f:pitch(11), -7, "a long interval produces at most one 100 ms movement step")
    f.delta_seconds = 1/60
    f:set_pitch(11, 0)
    f:tick()
    close(f:pitch(11), -70 / 60, "a shorter update uses its current measured interval")
    local pitch = f:pitch(11)
    f.delta_seconds = 0
    f:tick()
    equal(f:pitch(11), pitch, "a zero interval cannot authorize movement")
    f.delta_seconds = 0/0
    f:tick()
    equal(f:pitch(11), pitch, "a nonfinite interval cannot authorize movement")
    f.active_item = 0
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "landing restores the original rate after a slow interval")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.pitch_degrees = -100
    f:tick()
    close(f:pitch(11), -100, "inversion cannot be mistaken for an upright climb")
    assert(f.detail:find("inverted recovery", 1, true), "recovery is reported")
    f:set_pitch(11, -80)
    f:tick()
    close(f:pitch(11), -80, "holding climb cannot immediately restart rotation after inversion")
    f.pitch_input = 0
    f:tick()
    close(f:pitch(11), -80, "releasing climb while upright clears recovery without moving")
    f.pitch_input = -1
    f:tick()
    close(f:pitch(11), -80 - 70/60, "a new climb command resumes this player's bounded steering")
    f:unload()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "unload restores the shared native rate")
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    f.delta_seconds, f.players = 0.1, {11, 12}
    f.player_states[11] = {pitch_degrees=-100}
    f:tick()
    close(f:pitch(11), -100, "the inverted player waits for recovery")
    close(f:pitch(12), -7, "another player's inversion does not stop an upright player's climb")
    f:set_pitch(11, -80)
    f:tick()
    close(f:pitch(11), -80, "the recovery latch remains bound to the affected player")
    close(f:pitch(12), -14, "other players continue climbing during recovery")
    f.player_states[11].active_item = 0
    f:tick()
    f.player_states[11].active_item = f.item_id
    f:set_pitch(11, 0)
    f:tick()
    close(f:pitch(11), -7, "a new flight has no stale recovery latch")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode = f.item_id, "YawSpeedPitchSpeed"
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0)
    f.settings.unlimitedFlying, f.read_busy = false, true
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 0, "busy restoration retains ownership of the original pitch baseline")
    f.read_busy = false
    f:tick()
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70, "restoration retries after the resource gate becomes available")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode = f.item_id, "YawSpeedPitchSpeed"
    f:tick()
    equal(f.writes, 1, "neutral flight only installs the shared zero rate")
    f.session = f.session + 1
    f:tick()
    equal(f.state, "write-failed", "the pitch baseline cannot be restored into a new world")
    equal(f.writes, 1, "a stale pitch owner cannot write the later session")
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item, f.pitch_mode, f.pitch_input = f.item_id, "YawSpeedPitchSpeed", -1
    runtime.has = function(operation) return operation ~= "runtime.ecs.write" end
    f:tick()
    equal(f.writes, 0, "a client replica cannot steer authoritative player transforms")
    equal(f.state, "waiting")
    equal(f.glider.equipment.gliderConfig.pitchAngleSpeed, 70)
    f:unload()
end

do
    local f = fixture()
    f:tick()
    equal(f.writes, 0, "defaults do not modify original inputs")
    f.settings.unlimitedFlying = true
    f.settings.unlimitedGliderStamina = true
    f.settings.turnSpeed = 200
    f.settings.moveSpeed = 150
    f.settings.liftSpeed = 200
    f.settings.descendSpeed = 50
    f.active_item = f.item_id
    f:tick()
    equal(f.attributes[11].StaminaDepletionMod_Glider, 0, "original glider stamina input")
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 110, "turn multiplier")
    equal(f.glider.equipment.gliderConfig.accelerationForward, 1.5, "move multiplier")
    equal(f.glider.equipment.gliderConfig.airResistanceLongitudinal, string.unpack("f", string.pack("f", 0.15500000119209 * 2)), "lift coefficient multiplier")
    equal(f.glider.equipment.gliderConfig.airResistanceVertical, 1.5, "descend coefficient multiplier")
    local applied_writes = f.writes
    f:tick()
    equal(f.writes, applied_writes, "does not compound or rewrite unchanged settings")

    f.settings.turnSpeed = 100
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 55, "returning a slider to 100 percent restores baseline")
    f.glider.equipment.gliderConfig.accelerationForward = 1.7
    f:tick()
    equal(f.glider.equipment.gliderConfig.accelerationForward, 1.7, "external resource update is preserved")
    f.settings = {}
    f:tick()
    equal(f.attributes[11].StaminaDepletionMod_Glider, 0.35, "stamina baseline restored")
    equal(f.glider.equipment.gliderConfig.airResistanceLongitudinal, string.unpack("f", string.pack("f", 0.155)), "lift baseline restored")
    equal(f.glider.equipment.gliderConfig.airResistanceVertical, 3, "descend baseline restored")
    equal(f.glider.equipment.gliderConfig.accelerationForward, 1.7, "external resource update retained on disable")
    f:unload()
end

do
    local f = fixture()
    f.active_item = f.item_id
    f.settings.turnSpeed = 200
    f.block_after_write = true
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 110, "confirmed write survives a later busy read")
    equal(f.writes, 1)
    assert(f.state ~= "write-failed", f.detail)
    f:tick()
    equal(f.writes, 1, "busy reads preserve the original ownership baseline")
    f.read_busy, f.block_after_write = false, false
    f:tick()
    equal(f.writes, 1, "read recovery does not multiply the already modified value")
    f.settings.turnSpeed = 300
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 165, "changed multiplier still uses the original 55 baseline")
    f.settings.turnSpeed = 100
    f.read_busy = true
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 165, "busy restore retains ownership")
    f.read_busy = false
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 55, "restore retries the preserved original baseline")
    f:unload()
end

do
    local f = fixture()
    f.players = {11}
    f.settings.unlimitedFlying = true
    f.settings.unlimitedGliderStamina = true
    f.settings.turnSpeed = 200
    f.active_item = f.item_id
    f:tick()
    equal(f.attributes[11].StaminaDepletionMod_Glider, 0, "stamina setting works before a glider action")
    equal(f.writes, 3)
    f.session = f.session + 1
    f:tick()
    equal(f.state, "write-failed", "shared item setting cannot cross sessions")
    equal(f.writes, 3, "no write is authorized against the old session")
end

do
    local f = fixture()
    local source = game.assets.get_loaded_resource("80c9eaeb-7fa0-480d-a55f-91c2c51b383e", "keen::ItemInfo", 0)
    local target = assert(runtime.overrides.resource(source, "equipment.gliderConfig.yawAngleSpeed"))
    local observed = assert(target.read())
    f.session = f.session + 1
    local current, reason = target.read()
    equal(current, nil, "generic resource target remains bound to its creation world")
    assert(reason)
    local applied = target.write(observed, 110)
    equal(applied, false, "a retained target cannot restore or change a later world's resource")
    equal(f.writes, 0, "generic target session guard runs before the writer")
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 55)
end
