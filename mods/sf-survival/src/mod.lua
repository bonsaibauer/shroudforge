local overrides = runtime.overrides.new()
local last_state, last_detail
local unsupported_attributes, unsupported_session, last_attribute_settings = {}, nil, nil
local attribute_snapshots = {}
local signedness_failure = "signedness-dependent program with conflicting resource/storage scalar types"

local attributes = {
    {setting="infiniteStamina", root="Stamina", field="Stamina_Min", maximum="Stamina_Max"},
    {setting="infiniteMana", root="Mana", field="Mana_Min", maximum="Mana_Max"},
    {setting="infiniteUnderwaterBreath", root="Oxygen", field="Oxygen_Min", maximum="Oxygen_Max"},
    {setting="infiniteFrostTimer", root="BodyHeat", field="BodyHeat_Min", maximum="BodyHeat_Max"},
    {setting="fastHealthRegen", root="Health", field="Health_Recharge"},
    {setting="fastManaRegen", root="Mana", field="Mana_Recharge"},
}

local function enabled(key)
    return shroudforge.settings.get(key, false) == true
end

local function close_attribute_snapshots(snapshots)
    for _, roots in pairs(snapshots or {}) do
        for _, context in pairs(roots) do context.active = false end
    end
end

local function reset_unsupported_attributes(session)
    if unsupported_session ~= session then
        unsupported_attributes = {}
        unsupported_session = session
        last_attribute_settings = nil
    end
    local current = {}
    for _, feature in ipairs(attributes) do current[feature.setting] = enabled(feature.setting) end
    if last_attribute_settings then
        for setting, value in pairs(current) do
            if last_attribute_settings[setting] ~= value then
                unsupported_attributes = {}
                break
            end
        end
    else
        unsupported_attributes = {}
    end
    last_attribute_settings = current
end

local function attribute_context(snapshots, entity, root, session)
    local roots = snapshots[entity]
    if not roots then roots = {}; snapshots[entity] = roots end
    local context = roots[root]
    if not context then
        context = {active=true, session=session, entity=entity}
        roots[root] = context
    end
    return context
end

local function permanent_attribute_failure(reason)
    return type(reason) == "string" and reason:find(signedness_failure, 1, true) ~= nil
end

local function report(state, detail)
    if last_state == state and last_detail == detail then return end
    last_state, last_detail = state, detail
    runtime.report_effect(state, detail)
end

local function finite(value)
    return type(value) == "number" and value == value and math.abs(value) < math.huge
end

local function attribute_target(feature, snapshot, baseline)
    if feature.maximum then
        local maximum = snapshot.values[feature.maximum]
        if not finite(maximum) or maximum <= 0 then return nil, "A positive original maximum is required" end
        return maximum
    end
    if baseline <= 0 then return nil, "Natural " .. feature.root .. " regeneration is not active" end
    -- The original Recharge programs clamp this input to 0..65536.
    return math.min(baseline * 100, 65536)
end

local function any_enabled()
    for _, feature in ipairs(attributes) do if enabled(feature.setting) then return true end end
    return enabled("noFallDamage") or enabled("infiniteWeaponDurability")
end

local function update()
    close_attribute_snapshots(attribute_snapshots)
    attribute_snapshots = {}
    local session = runtime.world.session_id()
    reset_unsupported_attributes(session)
    if session == 0 then report("waiting", "Waiting for an authoritative world") return end
    overrides:begin_update()
    overrides:restore_disabled(enabled)
    if overrides.fault then report("write-failed", overrides.fault) return end
    if not any_enabled() then
        unsupported_attributes = {}
        report(overrides:has_ownership() and "waiting" or "no-change", overrides:summary("All Survival settings are off"))
        return
    end
    if not runtime.has("runtime.ecs.read") or not runtime.has("runtime.ecs.write") then
        report("waiting", "Waiting for typed ECS reads and the authoritative write scope") return
    end

    local players, reason = runtime.ecs.query("keen::ecs::PlayerInput")
    if not players then report("waiting", tostring(reason or "Player query is pending")) return end
    if #players == 0 then
        unsupported_attributes = {}
        report("no-target", "No live player entity is available")
        return
    end

    local current_players = {}
    for _, entity in ipairs(players) do current_players[entity] = true end
    for entity in pairs(unsupported_attributes) do
        if not current_players[entity] then unsupported_attributes[entity] = nil end
    end

    local current_snapshots = {}
    attribute_snapshots = current_snapshots
    for _, entity in ipairs(players) do
        for _, feature in ipairs(attributes) do
            if enabled(feature.setting) then
                local entity_failures = unsupported_attributes[entity]
                local failure = entity_failures and entity_failures[feature.field]
                if failure then
                    overrides:waiting(feature.setting .. ": " .. failure)
                else
                    local context = attribute_context(current_snapshots, entity, feature.root, session)
                    local target
                    target, failure = runtime.overrides.attribute(entity, feature.field, context)
                    if target then
                        local applied
                        local fresh_attempt
                        applied, failure, fresh_attempt = overrides:apply(target.key, feature.setting, target, function(snapshot, baseline)
                            return attribute_target(feature, snapshot, baseline)
                        end)
                        if not applied and fresh_attempt and permanent_attribute_failure(failure) then
                            entity_failures = entity_failures or {}
                            entity_failures[feature.field] = failure
                            unsupported_attributes[entity] = entity_failures
                        end
                    else
                        if permanent_attribute_failure(failure) then
                            entity_failures = entity_failures or {}
                            entity_failures[feature.field] = failure
                            unsupported_attributes[entity] = entity_failures
                        end
                        overrides:waiting(feature.setting .. ": " .. tostring(failure))
                    end
                end
            end
        end
        if enabled("noFallDamage") then
            local view, why = runtime.ecs.get_template_component(entity, "keen::ecs::FallDamage")
            if view then
                local target, failure = runtime.overrides.template(view,
                    "fallDamageStartDistance", {"fallDamageLethalDistance"})
                if target then overrides:apply(target.key, "noFallDamage", target, function(snapshot)
                    local lethal = snapshot.values.fallDamageLethalDistance
                    if not finite(lethal) then return nil, "FallDamage lethal distance is not finite" end
                    -- The original calculation disables fall damage at Start >= Lethal.
                    return math.max(snapshot.value, lethal)
                end)
                else overrides:waiting("No Fall Damage: " .. tostring(failure)) end
            else overrides:waiting("No Fall Damage: " .. tostring(why)) end
        end
    end

    if enabled("infiniteWeaponDurability") then
        local service, why = runtime.services.get("ecs::GameSettings")
        if service then
            local target, failure = runtime.overrides.service(service, "enableDurability")
            if target then
                overrides:apply(target.key, "infiniteWeaponDurability", target, function() return false end)
            else overrides:waiting("Infinite Weapon Durability: " .. tostring(failure)) end
        else overrides:waiting("Infinite Weapon Durability: " .. tostring(why)) end
    end

    close_attribute_snapshots(current_snapshots)
    attribute_snapshots = {}

    local state = overrides.fault and "write-failed" or
        (overrides.waiting_count > 0 and "waiting" or (overrides.writes > 0 and "write-confirmed" or "no-change"))
    report(state, overrides:summary("Original Survival inputs are active"))
end

return {
    update_interval_ms = 100,
    on_load = function()
        report("waiting", "SF Survival loaded. Waiting for typed engine access")
    end,
    on_update = update,
    on_unload = function() close_attribute_snapshots(attribute_snapshots) end,
}
