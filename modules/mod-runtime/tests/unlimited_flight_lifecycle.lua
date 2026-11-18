local function equal(actual, expected, label)
    assert(actual == expected, (label or "value") .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
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
        patch_available=true, pitch_patch_enabled=false, patch_calls=0,
        action_sets_gliding=false, action_unsets_gliding=false,
        attributes={[11]={StaminaDepletionMod_Glider=0.35}},
        trackers={},
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
        patch={
            available=function(name)
                equal(name, "runtime.patch.glider_pitch")
                return f.patch_available
            end,
            set_enabled=function(name, enabled)
                equal(name, "runtime.patch.glider_pitch")
                f.patch_calls = f.patch_calls + 1
                if not f.patch_available then return false, "native patch is unavailable" end
                f.pitch_patch_enabled = enabled
                return true
            end,
        },
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
                if type_name == "keen::ecs::Actor" then
                    equal(paths[1], "currentAction.triggerContext.usedItem.itemId")
                    return {[paths[1]]={value=f.active_item},
                        currentState=f.active_item > 0 and not f.action_sets_gliding and {"HangGliding"} or {},
                        ["currentAction.playbackContext.state"]=f.active_item > 0 and {"IsActive"} or {},
                        ["currentAction.playbackContext.setStateMask"]=f.action_sets_gliding and {"HangGliding"} or {},
                        ["currentAction.playbackContext.unsetStateMask"]=f.action_unsets_gliding and {"HangGliding"} or {}}
                end
                error("unexpected component " .. type_name)
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
    f.active_item = f.item_id
    f.action_sets_gliding = true
    f:tick()
    equal(f.pitch_patch_enabled, true, "upward gliding enables the native pitch patch")
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 110, "effective action state enables original glider controls")
    f.action_unsets_gliding = true
    f:tick()
    equal(f.pitch_patch_enabled, true, "native pitch control does not depend on one actor's action state")
    f.settings.turnSpeed = 100
    f:tick()
    equal(f.glider.equipment.gliderConfig.yawAngleSpeed, 55, "returning the glider slider restores its original value")
    equal(f.pitch_patch_enabled, true, "the native patch remains enabled while its setting is on")
    f.settings.unlimitedFlying = false
    f:tick()
    equal(f.pitch_patch_enabled, false, "disabling upward gliding restores the native patch")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.active_item = 0
    f:tick()
    equal(f.pitch_patch_enabled, true, "native pitch control does not require an active glider item")
    equal(f.writes, 0, "native pitch control leaves shared glider resource fields unchanged")
    f.session = 0
    f:tick()
    equal(f.pitch_patch_enabled, true, "a temporarily unavailable world does not disable the patch")
    f.settings.unlimitedFlying = false
    f:tick()
    equal(f.pitch_patch_enabled, false, "disabling the setting restores the patch without a world")
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f.patch_available = false
    f:tick()
    equal(f.pitch_patch_enabled, false, "an unavailable native patch is never reported as enabled")
    equal(f.state, "waiting", "unavailable native pitch control is reported as waiting")
    local attempts = f.patch_calls
    f.patch_available = true
    f:tick()
    equal(f.pitch_patch_enabled, true, "native patch activation is retried when the runtime becomes ready")
    equal(f.patch_calls, attempts + 1, "runtime readiness causes one fresh activation attempt")
    f.settings.unlimitedFlying = false
    f:unload()
end

do
    local f = fixture()
    f.settings.unlimitedFlying = true
    f:tick()
    equal(f.pitch_patch_enabled, true, "native patch activation succeeds")
    f:unload()
    equal(f.pitch_patch_enabled, false, "unloading the mod restores the original game code")
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
    equal(f.writes, 2)
    f.session = f.session + 1
    f:tick()
    equal(f.state, "write-failed", "shared item setting cannot cross sessions")
    equal(f.writes, 2, "no write is authorized against the old session")
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
