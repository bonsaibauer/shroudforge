-- These fixtures check Lua control flow against the public API contracts.
-- Original game calculations are checked separately against the game program.
local function equal(actual, expected, label)
    assert(actual == expected, (label or "value") .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
end

local function fixture()
    local f = {
        settings={}, session=41, writes=0, calls={}, players={11, 12},
        fall={fallDamageStartDistance=8, fallDamageLethalDistance=16},
        service={enableDurability=true}, attributes={}, trackers={}, read_counts={},
    }
    local root_ids = {Health=101, Stamina=102, Mana=103, Oxygen=104, BodyHeat=105}
    for _, entity in ipairs(f.players) do
        f.attributes[entity] = {}
        for _, root in ipairs({"Health", "Stamina", "Mana", "Oxygen", "BodyHeat"}) do
            f.attributes[entity][root] = {
                [root]=50, [root .. "_Min"]=0, [root .. "_Max"]=100, [root .. "_Recharge"]=2,
            }
        end
    end
    local function copy(values)
        local result = {}
        for key, value in pairs(values) do result[key] = value end
        return result
    end
    local function write(values, field, value, expected)
        if f.fail_write then
            local why = f.fail_write
            f.fail_write = nil
            return false, why
        end
        equal(values[field], expected, "expected current field")
        values[field] = value
        f.writes = f.writes + 1
        return true
    end
    local function view(key, values)
        local bound_session = f.session
        return {
            key=key, session_id=bound_session,
            type={qualified_name=values == f.fall and "keen::ecs::FallDamage" or "keen::ecs::GameSettings"},
            read_fields=function(_, fields)
                if f.session ~= bound_session then return nil, "view belongs to another session" end
                local selected = {}
                for _, field in ipairs(fields) do selected[field] = values[field] end
                return selected
            end,
            write_field=function(_, field, value, expected, expected_fields)
                if f.session ~= bound_session then return false, "view belongs to another session" end
                if values == f.fall then
                    equal(type(expected_fields), "table", "dependent template expectation")
                    equal(expected_fields.fallDamageLethalDistance, values.fallDamageLethalDistance)
                end
                return write(values, field, value, expected)
            end,
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
    overrides.get_template_claim = claim
    overrides.get_service_claim = claim
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
        values={normalize_field=function(_, _, value)
            if type(value) == "number" then return string.unpack("f", string.pack("f", value)) end
            return value
        end},
        ecs={
            query=function(name)
                equal(name, "keen::ecs::PlayerInput")
                return f.players
            end,
            get_attribute=function(selector)
                local root = selector:match("^(.-)_") or selector
                return {
                    name=selector, root_hash=root_ids[root],
                    calculation_writable=f.attribute_reason == nil,
                    calculation_reason=f.attribute_reason,
                }
            end,
            read_attributes=function(entity, selector)
                local root = selector:match("^(.-)_") or selector
                f.calls[root] = true
                f.read_counts[root] = (f.read_counts[root] or 0) + 1
                return {
                    entity=entity, root_hash=root_ids[root],
                    values=copy(f.attributes[entity][root]),
                    component={qualified_name="keen::ecs::" .. root},
                }
            end,
            evaluate_attributes=function(field, values)
                local normalized = copy(values)
                if f.float_mana and field:match("^Mana_") then
                    normalized[field] = string.unpack("f", string.pack("f", values[field]))
                end
                return {values=normalized}
            end,
            update_attribute=function(entity, field, value, expected)
                local root = field:match("^(.-)_")
                if f.fail_attribute then return nil, f.fail_attribute end
                local values = f.attributes[entity][root]
                equal(type(expected), "table", "attribute expected values")
                for name, old in pairs(expected) do equal(values[name], old, name) end
                if f.float_mana and root == "Mana" then value = string.unpack("f", string.pack("f", value)) end
                local ok, why = write(values, field, value, expected[field])
                if not ok then return nil, why end
                if field:match("_Min$") then
                    equal(expected[root .. "_Max"], values[root .. "_Max"], "guard dependent maximum")
                    values[root] = math.max(values[root], value)
                end
                return {
                    entity=entity, root_hash=root_ids[root], values=copy(values),
                    component={qualified_name="keen::ecs::" .. root},
                }
            end,
            get_template_component=function(_, name)
                equal(name, "keen::ecs::FallDamage")
                return view("shared-template/fall", f.fall)
            end,
        },
        services={get=function(name)
            equal(name, "ecs::GameSettings")
            return view(name, f.service)
        end},
    }
    for name, target in pairs(override_targets_factory(runtime, {})) do overrides[name] = target end
    f.mod = assert(load(survival_source, "survival"))()
    f.mod.on_load()
    function f:tick()
        if self.cleanup_pending then return self:unload() end
        self.mod.on_update()
    end
    function f:unload()
        self.cleanup_pending = true
        if self.mod.on_unload then self.mod.on_unload() end
        -- The loader owns this registry cleanup in production. Its actual
        -- lifecycle transition and retry behavior has separate Rust coverage.
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
    f:tick()
    equal(f.writes, 0, "all settings default off")
    for _, setting in ipairs({"noFallDamage", "fastHealthRegen", "fastManaRegen", "infiniteStamina",
        "infiniteUnderwaterBreath", "infiniteWeaponDurability", "infiniteFrostTimer", "infiniteMana"}) do
        f.settings[setting] = true
    end
    f:tick()
    equal(f.writes, 14, "six roots per player and two shared inputs")
    equal(f.fall.fallDamageStartDistance, 16)
    equal(f.service.enableDurability, false)
    for _, roots in pairs(f.attributes) do
        for _, root in ipairs({"Stamina", "Mana", "Oxygen", "BodyHeat"}) do equal(roots[root][root .. "_Min"], 100, root) end
        equal(roots.Health.Health_Recharge, 200)
        equal(roots.Mana.Mana_Recharge, 200)
    end
    f:tick()
    equal(f.writes, 14, "no compounding or duplicate writes")
    f.attributes[11].Stamina.Stamina_Max = 120
    f:tick()
    equal(f.attributes[11].Stamina.Stamina_Min, 120, "follows changed maximum")
    f.settings = {}
    f:tick()
    equal(f.service.enableDurability, true, "boolean baseline restored")
    equal(f.fall.fallDamageStartDistance, 8, "shared template restored")
    equal(f.attributes[11].Stamina.Stamina_Min, 0)
    equal(f.attributes[11].Health.Health_Recharge, 2)
    f.settings.infiniteStamina = true
    f:tick()
    equal(f.attributes[11].Stamina.Stamina_Min, 120, "setting can be enabled again")
    f:unload()
    equal(f.attributes[11].Stamina.Stamina_Min, 0, "unload restoration")
end

do
    local f = fixture()
    f.players = {11}
    f.settings.infiniteMana = true
    f.fail_write = "original Scene gate is busy"
    f:tick()
    equal(f.writes, 0)
    f:tick()
    equal(f.attributes[11].Mana.Mana_Min, 100, "busy retries without losing baseline")
    f.attributes[11].Mana.Mana_Min = 23
    f:tick()
    equal(f.attributes[11].Mana.Mana_Min, 23, "external update preserved")
    f.settings.infiniteMana = false
    f:tick()
    equal(f.attributes[11].Mana.Mana_Min, 23, "external update preserved on disable")
    f.settings.infiniteMana = true
    f:tick()
    equal(f.attributes[11].Mana.Mana_Min, 100)
    f:unload()
    equal(f.attributes[11].Mana.Mana_Min, 23, "rebinding captures fresh baseline")
end

do
    local f = fixture()
    f.players = {11}
    f.settings.fastHealthRegen = true
    f.attributes[11].Health.Health_Recharge = 0
    f:tick()
    equal(f.writes, 0, "does not invent natural regeneration")
    f.attributes[11].Health.Health_Recharge = 1000
    f:tick()
    equal(f.attributes[11].Health.Health_Recharge, 65536, "original Recharge cap")
    f:unload()
    equal(f.attributes[11].Health.Health_Recharge, 1000)
end

do
    local f = fixture()
    f.players = {11}
    f.float_mana = true
    f.attributes[11].Mana.Mana_Recharge = string.unpack("f", string.pack("f", 0.1))
    local original = f.attributes[11].Mana.Mana_Recharge
    f.settings.fastManaRegen = true
    f:tick()
    equal(f.attributes[11].Mana.Mana_Recharge, 10, "float32 target is compared after original normalization")
    assert(f.state ~= "write-failed", f.detail)
    local writes = f.writes
    f:tick()
    equal(f.writes, writes, "rounded float target does not cause repeated no-op writes")
    f:unload()
    equal(f.attributes[11].Mana.Mana_Recharge, original)
end

do
    local f = fixture()
    f.settings.infiniteWeaponDurability = true
    f:tick()
    f.fail_write = "original Scene gate is busy"
    local ok = pcall(function() f:unload() end)
    equal(ok, false, "busy cleanup prevents disposal of the mod scope")
    f:tick()
    equal(f.service.enableDurability, true, "pending cleanup retries without reapplying active settings")
    f:unload()
    f.mod.on_load()
    f:tick()
    equal(f.service.enableDurability, false, "fresh activation resumes normal updates")
    f:unload()
end

do
    local f = fixture()
    f.players = {11}
    f.settings.infiniteStamina = true
    f.settings.infiniteWeaponDurability = true
    f.fail_write = "uncertain native rollback"
    f:tick()
    equal(f.writes, 0, "no later writes after uncertain outcome")
    f:tick()
    equal(f.writes, 0, "uncertain outcome remains stopped")
    equal(f.state, "write-failed")
end

do
    local f = fixture()
    f.settings.noFallDamage = true
    f:tick()
    local before = f.writes
    f.session = f.session + 1
    f:tick()
    equal(f.writes, before, "never restores an old shared view in a new session")
    equal(f.state, "write-failed")
end

do
    local f = fixture()
    f.players = {11}
    f.settings.infiniteMana = true
    f.settings.fastManaRegen = true
    f:tick()
    equal(f.read_counts.Mana, 1, "same-root settings share one update snapshot")
    equal(f.attributes[11].Mana.Mana_Min, 100)
    equal(f.attributes[11].Mana.Mana_Recharge, 200)
    f:unload()
    equal(f.read_counts.Mana, 3, "cleanup obtains fresh snapshots after the update cache closes")
end

do
    local f = fixture()
    f.players = {11}
    f.settings.infiniteMana = true
    f.fail_attribute = "signedness-dependent program with conflicting resource/storage scalar types"
    f:tick()
    equal(f.read_counts.Mana, 1, "first unsupported attempt resolves the entity root")
    f:tick()
    equal(f.read_counts.Mana, 1, "known unsupported root is not retried every update")
    f.settings.infiniteMana = false
    f:tick()
    f.fail_attribute = nil
    f.settings.infiniteMana = true
    f:tick()
    equal(f.read_counts.Mana, 2, "changing the setting clears the unsupported cache")
    equal(f.attributes[11].Mana.Mana_Min, 100)
    f:unload()
end

do
    local f = fixture()
    f.players = {11}
    f.settings.infiniteMana = true
    f.attribute_reason = "signedness-dependent program with conflicting resource/storage scalar types"
    f:tick()
    equal(f.read_counts.Mana, nil, "catalog-known unsupported root is rejected before native reads")
    f:tick()
    equal(f.read_counts.Mana, nil, "catalog failure remains cached for this runtime environment")
    f:unload()
end
