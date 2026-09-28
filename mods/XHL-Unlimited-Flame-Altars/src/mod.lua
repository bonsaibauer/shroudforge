-- XHL Unlimited Flame Altars v3.0.1
--
-- The altarsPerFlameLevel resource type is uint8 and can represent at most 255; it remains only
-- for UI and compatibility display. Unlimited placement is implemented by setting
-- requiresAltarSlot=false on Flame Altar building items. This mod's Native Sidecar handles
-- PlayerBases dynamic capacity and region queries.

local LOG_PREFIX = "[XHL-Unlimited-Flame-Altars]"
local FLAME_ALTAR_ITEM_GUID = "85ad0843-7f32-4026-95d2-96c8e54e2899"
local FLAME_ALTAR_PLACED_ENTITY_GUID = "5bcf6d3e-6067-4e25-a0f9-dab49aed4ae3"
local DISPLAY_LIMIT = 255

local function trim(value)
    return (tostring(value):gsub("^%s+", ""):gsub("%s+$", ""))
end

local function parse_bool(value, fallback)
    value = trim(value):lower()
    if value == "true" or value == "1" or value == "yes" or value == "on" then
        return true
    end
    if value == "false" or value == "0" or value == "no" or value == "off" then
        return false
    end
    return fallback
end

local function read_enabled()
    local enabled = true
    if not io.exists("config.txt") then
        return enabled
    end

    local text = io.read_to_string("config.txt")
    for line in text:gmatch("[^\r\n]+") do
        line = trim(line)
        if line ~= "" and not line:match("^#") and not line:match("^;") then
            local key, value = line:match("^([%w_%-]+)%s*=%s*(.-)%s*$")
            if key == "enabled" then
                enabled = parse_bool(value, enabled)
            end
        end
    end
    return enabled
end

local function get_field(object, field)
    local ok, value = pcall(function()
        return object[field]
    end)
    if ok then
        return value
    end
    return nil
end

local function patch_display_limit()
    local resources = game.assets.get_resources_by_type("keen::BalancingTable")
    local patched_resources = 0
    local patched_entries = 0

    for _, resource in ipairs(resources) do
        local altars = get_field(resource.data, "altarsPerFlameLevel")
        if altars ~= nil then
            patched_resources = patched_resources + 1
            for index, _ in ipairs(altars) do
                altars[index] = DISPLAY_LIMIT
                patched_entries = patched_entries + 1
            end
        end
    end

    return patched_resources, patched_entries
end

local function remove_placement_slot_gate()
    local resources = game.assets.get_resources_by_type("keen::ItemInfo")
    local matched = 0
    local changed = 0

    for _, resource in ipairs(resources) do
        local data = resource.data
        local equipment = get_field(data, "equipment")
        if equipment ~= nil then
            local resource_guid = tostring(get_field(resource, "guid"))
            local placed_entity = tostring(get_field(equipment, "placedEntity"))
            if resource_guid == FLAME_ALTAR_ITEM_GUID or
               placed_entity == FLAME_ALTAR_PLACED_ENTITY_GUID then
                matched = matched + 1
                local before = get_field(equipment, "requiresAltarSlot")
                equipment.requiresAltarSlot = false
                if before ~= false then
                    changed = changed + 1
                end
            end
        end
    end

    return matched, changed
end

if not read_enabled() then
    print(LOG_PREFIX .. " disabled by config")
    return
end

local balancing_resources, balancing_entries = patch_display_limit()
local altar_items, slot_gates_removed = remove_placement_slot_gate()

if altar_items == 0 then
    error(LOG_PREFIX .. " Flame Altar ItemInfo not found; unlimited placement was not enabled")
end

print(LOG_PREFIX ..
    " v3.0.1 active; displayLimit=" .. tostring(DISPLAY_LIMIT) ..
    ", balancingResources=" .. tostring(balancing_resources) ..
    ", balancingEntries=" .. tostring(balancing_entries) ..
    ", altarItems=" .. tostring(altar_items) ..
    ", slotGatesRemoved=" .. tostring(slot_gates_removed))
