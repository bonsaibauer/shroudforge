-- Temporary diagnostic entrypoint for the SF Survival package.
-- Select it from src/mod.lua with: return require("api_return_probe")
-- This probe never requests a value different from the value it just read.

local settings = {
    "noFallDamage",
    "fastHealthRegen",
    "fastManaRegen",
    "infiniteStamina",
    "infiniteUnderwaterBreath",
    "infiniteWeaponDurability",
    "infiniteFrostTimer",
    "infiniteMana",
}

local attributes = {
    {setting="infiniteStamina", field="Stamina_Min", root="Stamina"},
    {setting="infiniteMana", field="Mana_Min", root="Mana"},
    {setting="infiniteUnderwaterBreath", field="Oxygen_Min", root="Oxygen"},
    {setting="infiniteFrostTimer", field="BodyHeat_Min", root="BodyHeat"},
    {setting="fastHealthRegen", field="Health_Recharge", root="Health"},
    {setting="fastManaRegen", field="Mana_Recharge", root="Mana"},
}

local completed = false
local attempts = 0
local prefix = "[SF Survival API probe] "
local unpack_values = table.unpack or unpack

local function safe_field(value, key)
    local ok, result = pcall(function() return value[key] end)
    return ok, result
end

local function describe_userdata(value)
    local fields = {}
    local keys = {"key", "scope", "session_id", "write_supported"}
    for _, key in ipairs(keys) do
        local ok, field = safe_field(value, key)
        if ok and field ~= nil then
            fields[#fields + 1] = key .. "=" .. tostring(field)
        end
    end
    local ok, type_value = safe_field(value, "type")
    if ok and type_value ~= nil then
        local type_ok, qualified_name = safe_field(type_value, "qualified_name")
        if type_ok and type(qualified_name) == "string" then
            fields[#fields + 1] = "type=" .. string.format("%q", qualified_name)
        end
    end
    return "userdata{" .. table.concat(fields, ", ") .. "}"
end

local function describe(value, visited)
    local value_type = type(value)
    if value == nil then return "nil" end
    if value_type == "string" then return string.format("%q", value) end
    if value_type == "boolean" or value_type == "number" then return tostring(value) end
    if value_type == "function" then return "<function>" end
    if value_type == "userdata" then return describe_userdata(value) end
    if value_type ~= "table" then return "<" .. value_type .. ">" end

    if value.key ~= nil and value.claim ~= nil then
        return "RuntimeOverrideTarget{key=" .. describe(value.key) ..
            ", shared=" .. describe(value.shared) ..
            ", root_hash=" .. describe(value.root_hash) ..
            ", component_name=" .. describe(value.component_name) ..
            ", claim=<RuntimeWriteClaim>, read=<function>, write=<function>, normalize=<function>}"
    end

    visited = visited or {}
    if visited[value] then return "<cycle>" end
    visited[value] = true
    local keys = {}
    for key in pairs(value) do keys[#keys + 1] = key end
    table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
    local fields = {}
    for _, key in ipairs(keys) do
        fields[#fields + 1] = "[" .. describe(key, visited) .. "]=" .. describe(value[key], visited)
    end
    visited[value] = nil
    return "{" .. table.concat(fields, ", ") .. "}"
end

local function pack(...)
    return {n=select("#", ...), ...}
end

local function capture(label, callback)
    local result = pack(pcall(callback))
    if not result[1] then
        shroudforge.log.error(prefix .. label .. " raised " .. tostring(result[2]))
        return false
    end
    local returned = {}
    for index = 2, result.n do
        returned[#returned + 1] = describe(result[index])
    end
    shroudforge.log.info(prefix .. label .. " returns (" .. table.concat(returned, ", ") .. ")")
    return true, unpack_values(result, 2, result.n)
end

local function capture_apply(tracker, target, setting, label)
    if not target then return end
    capture(label .. " RuntimeOverrideTarget:read()", function()
        return target.read()
    end)
    capture(label .. " RuntimeOverrides:apply no-op", function()
        return tracker:apply(target.key, setting, target, function(_snapshot, baseline)
            return baseline
        end)
    end)
end

local function capture_player(entity, session, tracker)
    local contexts = {}
    for _, feature in ipairs(attributes) do
        local context = contexts[feature.root]
        if not context then
            context = {active=true, session=session, entity=entity}
            contexts[feature.root] = context
        end
        local ok, target = capture("runtime.overrides.attribute(" .. entity .. ", " .. feature.field .. ")", function()
            return runtime.overrides.attribute(entity, feature.field, context)
        end)
        if ok then
            capture_apply(tracker, target, feature.setting, feature.setting)
        end
    end
    for _, context in pairs(contexts) do context.active = false end

    local fall_view_ok, fall_view = capture(
        "runtime.ecs.get_template_component(" .. entity .. ", keen::ecs::FallDamage)", function()
            return runtime.ecs.get_template_component(entity, "keen::ecs::FallDamage")
        end)
    if fall_view_ok and fall_view then
        local target_ok, target = capture("runtime.overrides.template(fallDamageStartDistance)", function()
            return runtime.overrides.template(fall_view, "fallDamageStartDistance", {"fallDamageLethalDistance"})
        end)
        if target_ok then capture_apply(tracker, target, "noFallDamage", "noFallDamage") end
    end
end

local function capture_runtime(session)
    shroudforge.log.info(prefix .. " starting in a nonzero world session " .. tostring(session))

    for _, key in ipairs(settings) do
        capture("shroudforge.settings.get(" .. key .. ", false)", function()
            return shroudforge.settings.get(key, false)
        end)
    end

    capture('runtime.has("runtime.ecs.read")', function()
        return runtime.has("runtime.ecs.read")
    end)
    capture('runtime.has("runtime.ecs.write")', function()
        return runtime.has("runtime.ecs.write")
    end)
    capture("runtime.world.session_id()", function()
        return runtime.world.session_id()
    end)

    local tracker_ok, tracker = capture("runtime.overrides.new()", function()
        return runtime.overrides.new()
    end)
    if not tracker_ok or not tracker then return end

    capture("RuntimeOverrides:begin_update()", function()
        return tracker:begin_update()
    end)
    capture('RuntimeOverrides:waiting("API probe capture")', function()
        return tracker:waiting("API probe capture")
    end)

    local query_ok, players, query_reason = capture(
        'runtime.ecs.query("keen::ecs::PlayerInput")', function()
            return runtime.ecs.query("keen::ecs::PlayerInput")
        end)
    if query_ok and type(players) == "table" then
        for _, entity in ipairs(players) do capture_player(entity, session, tracker) end
    elseif query_ok then
        shroudforge.log.info(prefix .. " player query reason detail=" .. describe(query_reason))
    end

    local service_ok, service, service_reason = capture(
        'runtime.services.get("ecs::GameSettings")', function()
            return runtime.services.get("ecs::GameSettings")
        end)
    if service_ok and service then
        local target_ok, target = capture("runtime.overrides.service(enableDurability)", function()
            return runtime.overrides.service(service, "enableDurability")
        end)
        if target_ok then capture_apply(tracker, target, "infiniteWeaponDurability", "infiniteWeaponDurability") end
    elseif service_ok then
        shroudforge.log.info(prefix .. " GameSettings service reason detail=" .. describe(service_reason))
    end

    capture("RuntimeOverrides:restore_disabled(probe tracker)", function()
        return tracker:restore_disabled(function() return true end)
    end)
    capture("RuntimeOverrides.fault", function() return tracker.fault end)
    capture("RuntimeOverrides.writes", function() return tracker.writes end)
    capture("RuntimeOverrides.waiting_count", function() return tracker.waiting_count end)
    capture("RuntimeOverrides:has_ownership()", function()
        return tracker:has_ownership()
    end)
    capture('RuntimeOverrides:summary("API probe capture")', function()
        return tracker:summary("API probe capture")
    end)
    capture('runtime.report_effect("no-change", "API return probe completed")', function()
        return runtime.report_effect("no-change", "API return probe completed")
    end)

    shroudforge.log.info(prefix .. " complete. No game value was changed.")
end

return {
    update_interval_ms = 100,
    on_update = function()
        if completed then return end
        attempts = attempts + 1
        local session = runtime.world.session_id()
        if session == 0 then
            if attempts == 1 then
                shroudforge.log.info(prefix .. " waiting for a local world session")
            end
            return
        end

        local readable = runtime.has("runtime.ecs.read")
        if not readable and attempts < 50 then return end
        completed = true
        capture_runtime(session)
    end,
}
