-- Enshrouded EML mod: XHL Auto Loot
-- v1.5.0 rebuilt from the early XHL whitelist version.
-- Lua relies only on drop names and their actual ECS component combinations; it does not use
-- other mods' exclusion lists, gathering-node name lists, or configuration keys.
-- The Native Sidecar recognizes the private 200000123-nanosecond marker, skips the vanilla initial random delay,
-- and limits repeated requests for the same player and drop based on vanilla InventoryFull results.

local TAG = "[XHL-Auto-Loot]"
local VERSION = "1.5.0"
local PICKUP_UPDATE_MARKER_NS = 200000123

local defaults = {
    enabled = true,
    collectGravityDrops = true,
    collectMaterialPiles = true,
    collectQuestItems = false,
    pickupRadiusMeters = 5.0,
    tuneGatherNodes = true,
    softenGravityDrops = true,
    removeSpawnDelay = true,
    keepLootSparks = true,
}

local booleanKeys = {
    enabled = true,
    collectGravityDrops = true,
    collectMaterialPiles = true,
    collectQuestItems = true,
    tuneGatherNodes = true,
    softenGravityDrops = true,
    removeSpawnDelay = true,
    keepLootSparks = true,
}

local function strip(text)
    return (tostring(text):gsub("^%s+", ""):gsub("%s+$", ""))
end

local function toBoolean(text, fallback)
    local value = strip(text):lower()
    if value == "true" or value == "1" or value == "yes" or value == "on" then return true end
    if value == "false" or value == "0" or value == "no" or value == "off" then return false end
    return fallback
end

local function loadSettings()
    local result = {}
    for key, value in pairs(defaults) do
        result[key] = value
    end

    if not io.exists("config.txt") then
        return result
    end

    local content = io.read_to_string("config.txt")
    if not content then
        return result
    end

    for rawLine in content:gmatch("[^\r\n]+") do
        local line = strip(rawLine)
        if line ~= "" and not line:match("^[#;]") then
            local key, rawValue = line:match("^([%w_%-]+)%s*=%s*(.-)%s*$")
            if key and rawValue and defaults[key] ~= nil then
                local value = strip(rawValue:match("^[^#;]*") or rawValue)
                if booleanKeys[key] then
                    result[key] = toBoolean(value, result[key])
                elseif key == "pickupRadiusMeters" then
                    local number = tonumber(value)
                    if number then
                        result[key] = number
                    end
                end
            end
        end
    end

    return result
end

local cfg = loadSettings()
if not cfg.enabled then
    print(TAG .. " disabled")
    return
end

cfg.pickupRadiusMeters = math.max(
    1.0,
    math.min(50.0, tonumber(cfg.pickupRadiusMeters) or 5.0)
)

local lootCategoriesEnabled =
    cfg.collectGravityDrops or
    cfg.collectMaterialPiles or
    cfg.collectQuestItems

local types = {
    pickup = "keen::ecs::PickupItem",
    destroy = "keen::ecs::DestroyOnLoot",
    inventory = "keen::ecs::Inventory",
    defaultInventory = "keen::ecs::DefaultInventory",
    zone = "keen::ecs::PickupItemZone",
    shape = "keen::ecs::TriggerShape",
    range = "keen::ecs::IsPlayerInRange",
    interaction = "keen::ecs::InteractionOffer",
    clientInteraction = "keen::ecs::ClientInteractionOffer",
    spawnTime = "keen::ecs::SpawnTime",
    gravity = "keen::ecs::Gravity",
    miningNode = "keen::ecs::MiningNode",
    resourceDrops = "keen::ecs::ResourceNodePickupDrops",
}

local zoneTypes = {
    [types.zone] = true,
    [types.shape] = true,
    [types.range] = true,
}

local zoneOrder = {
    types.zone,
    types.shape,
    types.range,
}

local function field(object, key)
    local ok, value = pcall(function()
        return object[key]
    end)
    if ok then
        return value
    end
    return nil
end

local function writeField(object, key, value)
    if not object then
        return false
    end
    return pcall(function()
        object[key] = value
    end)
end

local function componentIndex(components, wanted)
    for index, component in ipairs(components) do
        if field(component, "type") == wanted then
            return index
        end
    end
    return nil
end

local function has(components, wanted)
    return componentIndex(components, wanted) ~= nil
end

local function hasInventory(components)
    return has(components, types.inventory) or has(components, types.defaultInventory)
end

local function deepCopy(value, seen)
    if type(value) ~= "table" then
        return value
    end

    seen = seen or {}
    if seen[value] then
        return seen[value]
    end

    local output = {}
    seen[value] = output
    for key, item in pairs(value) do
        output[deepCopy(key, seen)] = deepCopy(item, seen)
    end
    return output
end

local function tuneZone(component)
    local kind = field(component, "type")
    local value = field(component, "value")
    if not value then
        return false
    end

    local changed = false
    if kind == types.shape then
        if writeField(value, "rangeX", cfg.pickupRadiusMeters) then changed = true end
        if writeField(value, "rangeY", cfg.pickupRadiusMeters) then changed = true end
        if writeField(value, "rangeZ", cfg.pickupRadiusMeters) then changed = true end
        if writeField(value, "shape", "Range") then changed = true end
    elseif kind == types.range then
        if writeField(value, "minRange", 0.0) then changed = true end
        if writeField(
            value,
            "updateDelay",
            { value = PICKUP_UPDATE_MARKER_NS }
        ) then
            changed = true
        end
    elseif kind == types.zone then
        if writeField(value, "lastAttempt", { value = 0 }) then changed = true end
    end
    return changed
end

local function templateName(data)
    local debugName = field(data, "debugName")
    if debugName and tostring(debugName) ~= "" then
        return tostring(debugName)
    end
    local name = field(data, "name")
    if name then
        return tostring(name)
    end
    return ""
end

local function beginsWith(text, prefix)
    return text:sub(1, #prefix) == prefix
end

local questPrefixes = {
    "LootPickup_gnome_",
    "LootPickup_Collections_",
    "LootPickup_Lore",
    "LootPickup_Quest",
}

local function isQuestName(name)
    for _, prefix in ipairs(questPrefixes) do
        if beginsWith(name, prefix) then
            return true
        end
    end
    return false
end

local function classifyLoot(name, components)
    if not has(components, types.destroy) then
        return nil
    end

    if cfg.collectGravityDrops and
       beginsWith(name, "LootPickup_Gravity_") and
       has(components, types.pickup) then
        return "gravity"
    end

    if cfg.collectMaterialPiles and
       beginsWith(name, "LootPickup_Material_") and
       not has(components, types.pickup) and
       hasInventory(components) then
        return "material"
    end

    if cfg.collectQuestItems and
       isQuestName(name) and
       not has(components, types.pickup) and
       hasInventory(components) then
        return "quest"
    end

    return nil
end

local function eraseAll(components, wanted)
    local count = 0
    for index = #components, 1, -1 do
        if field(components[index], "type") == wanted then
            table.remove(components, index)
            count = count + 1
        end
    end
    return count
end

local dropMotionFields = {
    "velocityMin",
    "velocityMax",
    "yOffsetMin",
    "yOffsetMax",
    "dropRadiusMin",
    "dropRadiusMax",
}

local function stabilizeResourceEmitter(components)
    if not has(components, types.miningNode) or
       not has(components, types.resourceDrops) then
        return 0
    end

    local changedFields = 0
    for _, component in ipairs(components) do
        if field(component, "type") == types.resourceDrops then
            local value = field(component, "value")
            if value then
                for _, key in ipairs(dropMotionFields) do
                    if writeField(value, key, 0.0) then
                        changedFields = changedFields + 1
                    end
                end
            end
        end
    end
    return changedFields
end

local function disableDropGravity(components)
    local changed = 0
    for _, component in ipairs(components) do
        if field(component, "type") == types.gravity then
            local value = field(component, "value")
            if value then
                if writeField(value, "isActive", false) then changed = changed + 1 end
                if writeField(value, "value", 0.0) then changed = changed + 1 end
            end
        end
    end
    return changed
end

local templates = game.assets.get_resources_by_type("keen::ecs::TemplateResource")
local zoneBlueprint = {}

for _, resource in ipairs(templates) do
    local data = field(resource, "data")
    local components = data and field(data, "components") or nil
    if components and templateName(data) == "Base_LootTouch_Orb" then
        for _, component in ipairs(components) do
            local kind = field(component, "type")
            if zoneTypes[kind] and not zoneBlueprint[kind] then
                local copy = deepCopy(component)
                tuneZone(copy)
                zoneBlueprint[kind] = copy
            end
        end
        break
    end
end

local totals = {
    gravity = 0,
    material = 0,
    quest = 0,
    zoneComponentsAdded = 0,
    interactionsRemoved = 0,
    spawnDelaysRemoved = 0,
    sparksRemoved = 0,
    gravityFieldsChanged = 0,
    resourceEmittersChanged = 0,
    resourceFieldsChanged = 0,
}

if cfg.tuneGatherNodes then
    for _, resource in ipairs(templates) do
        local data = field(resource, "data")
        local components = data and field(data, "components") or nil
        if components then
            local changed = stabilizeResourceEmitter(components)
            if changed > 0 then
                totals.resourceEmittersChanged = totals.resourceEmittersChanged + 1
                totals.resourceFieldsChanged = totals.resourceFieldsChanged + changed
            end
        end
    end
end

local blueprintReady =
    zoneBlueprint[types.zone] and
    zoneBlueprint[types.shape] and
    zoneBlueprint[types.range]

if lootCategoriesEnabled and not blueprintReady then
    warn(TAG .. " Base_LootTouch_Orb zone blueprint incomplete; loot templates unchanged")
elseif lootCategoriesEnabled then
    for _, resource in ipairs(templates) do
        local data = field(resource, "data")
        local components = data and field(data, "components") or nil
        if components then
            local category = classifyLoot(templateName(data), components)
            if category then
                for _, kind in ipairs(zoneOrder) do
                    local index = componentIndex(components, kind)
                    if index then
                        tuneZone(components[index])
                    else
                        local copy = deepCopy(zoneBlueprint[kind])
                        tuneZone(copy)
                        table.insert(components, copy)
                        totals.zoneComponentsAdded = totals.zoneComponentsAdded + 1
                    end
                end

                totals.interactionsRemoved =
                    totals.interactionsRemoved + eraseAll(components, types.interaction)

                if cfg.removeSpawnDelay then
                    totals.spawnDelaysRemoved =
                        totals.spawnDelaysRemoved + eraseAll(components, types.spawnTime)
                end

                if not cfg.keepLootSparks then
                    totals.sparksRemoved =
                        totals.sparksRemoved + eraseAll(components, types.clientInteraction)
                end

                if category == "gravity" and cfg.softenGravityDrops then
                    totals.gravityFieldsChanged =
                        totals.gravityFieldsChanged + disableDropGravity(components)
                end

                totals[category] = totals[category] + 1
            end
        end
    end
end

print(string.format(
    "%s v%s complete gravity=%d material=%d quest=%d componentsAdded=%d " ..
    "interactionsRemoved=%d spawnDelaysRemoved=%d sparksRemoved=%d " ..
    "gravityFieldsChanged=%d resourceEmittersChanged=%d " ..
    "resourceFieldsChanged=%d radius=%.1fm update=200ms",
    TAG,
    VERSION,
    totals.gravity,
    totals.material,
    totals.quest,
    totals.zoneComponentsAdded,
    totals.interactionsRemoved,
    totals.spawnDelaysRemoved,
    totals.sparksRemoved,
    totals.gravityFieldsChanged,
    totals.resourceEmittersChanged,
    totals.resourceFieldsChanged,
    cfg.pickupRadiusMeters
))
