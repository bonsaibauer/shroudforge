-- 雾锁王国 EML 模组：XHL 2倍抓钩牵引距离 v1.0.0
-- 只修改实际决定牵引距离的 Impact 配置值。
-- 独立的摆荡距离 Impact 配置值只会被定位和校验，始终不会写入。

local TAG = "[XHL-Grappling-Hook-Pull-2x]"

-- player_update_hookshot_targeting 会读取下面两个 Impact 配置 ID。
local PULL_DISTANCE_CONFIG_ID = 4151084092 -- 0xF76C843C
local SWING_DISTANCE_CONFIG_ID = 3078764024 -- 0xB78235F8

local TARGET_ITEMS = {
    ["70096018-ed26-4697-a13d-0f2f201760a3"] = "Tool_Grappling_Hook",
    ["3b8c8e70-a653-44d1-8061-7cb474c540da"] = "Tool_Grappling_Hook_improved",
    ["281a919a-e117-47b2-bdcb-4453834e3482"] = "Tool_Grappling_Hook_underwater",
}

local DEFAULTS = {
    enabled = true,
    pullDistanceMultiplier = 2.0,
}

local function trim(value)
    return (tostring(value):gsub("^%s+", ""):gsub("%s+$", ""))
end

local function parse_boolean(value, fallback)
    value = trim(value):lower()
    if value == "true" or value == "1" or value == "yes" or value == "on" then return true end
    if value == "false" or value == "0" or value == "no" or value == "off" then return false end
    return fallback
end

local function load_config()
    local config = {
        enabled = DEFAULTS.enabled,
        pullDistanceMultiplier = DEFAULTS.pullDistanceMultiplier,
    }

    if not io.exists("config.txt") then return config end
    local content = io.read_to_string("config.txt")
    if not content then return config end

    for line in content:gmatch("[^\r\n]+") do
        line = trim(line)
        if line ~= "" and not line:match("^[#;]") then
            local key, raw = line:match("^([%w_%-]+)%s*=%s*(.-)%s*$")
            if key and raw then
                raw = trim(raw:match("^[^#;]*") or raw)
                if key == "enabled" then
                    config.enabled = parse_boolean(raw, config.enabled)
                elseif key == "pullDistanceMultiplier" then
                    local value = tonumber(raw)
                    if value and value >= 1.0 and value <= 20.0 then
                        config.pullDistanceMultiplier = value
                    else
                        warn(TAG .. " invalid pullDistanceMultiplier; using " .. tostring(config.pullDistanceMultiplier))
                    end
                end
            end
        end
    end

    return config
end

local function field(object, key)
    if object == nil then return nil end
    local ok, value = pcall(function() return object[key] end)
    if ok then return value end
    return nil
end

local function number_of(value)
    local direct = tonumber(value)
    if direct ~= nil then return direct end
    return tonumber(field(value, "value"))
end

local function unwrap_variant(entry)
    local value = field(entry, "value")
    if value ~= nil and field(value, "configId") ~= nil then
        return value
    end
    return entry
end

local function nearly_equal(a, b)
    return math.abs(a - b) <= 0.0001
end

local config = load_config()
if not config.enabled then
    print(TAG .. " disabled")
    return
end

local resources = game.assets.get_resources_by_type("keen::ItemInfo")
local targetItemsFound = 0
local pullEntriesFound = 0
local swingEntriesFound = 0
local itemSummaries = {}

for _, resource in ipairs(resources) do
    local guid = tostring(field(resource, "guid"))
    local expectedName = TARGET_ITEMS[guid]

    if expectedName ~= nil then
        targetItemsFound = targetItemsFound + 1

        local item = field(resource, "data") or resource
        local actualName = tostring(field(item, "debugName") or "")
        if actualName ~= expectedName then
            error(TAG .. " target identity mismatch guid=" .. guid
                .. " expected=" .. expectedName .. " actual=" .. actualName)
        end

        local impactValues = field(item, "impactValues")
        local simple = impactValues and field(impactValues, "simple")
        if simple == nil then
            error(TAG .. " missing impactValues.simple for " .. actualName)
        end

        local oldPull = nil
        local newPull = nil
        local swingBefore = nil

        for _, entry in ipairs(simple) do
            local impactConfig = unwrap_variant(entry)
            local configId = number_of(field(impactConfig, "configId"))

            if configId == PULL_DISTANCE_CONFIG_ID then
                if oldPull ~= nil then
                    error(TAG .. " duplicate pull-distance config for " .. actualName)
                end

                oldPull = number_of(field(impactConfig, "value"))
                if oldPull == nil or oldPull <= 0 then
                    error(TAG .. " invalid pull distance for " .. actualName)
                end

                newPull = oldPull * config.pullDistanceMultiplier
                impactConfig.value = newPull

                local readback = number_of(field(impactConfig, "value"))
                if readback == nil or not nearly_equal(readback, newPull) then
                    error(TAG .. " pull-distance readback failed for " .. actualName)
                end

                pullEntriesFound = pullEntriesFound + 1
            elseif configId == SWING_DISTANCE_CONFIG_ID then
                if swingBefore ~= nil then
                    error(TAG .. " duplicate swing-distance config for " .. actualName)
                end

                swingBefore = number_of(field(impactConfig, "value"))
                if swingBefore == nil or swingBefore <= 0 then
                    error(TAG .. " invalid swing distance for " .. actualName)
                end

                -- 此处特意不对 impactConfig.value 赋值。
                swingEntriesFound = swingEntriesFound + 1
            end
        end

        if oldPull == nil or swingBefore == nil then
            error(TAG .. " required hookshot distance configs missing for " .. actualName)
        end

        itemSummaries[#itemSummaries + 1] = string.format(
            "%s pull=%.3f->%.3f swing=%.3f unchanged",
            actualName,
            oldPull,
            newPull,
            swingBefore
        )
    end
end

if targetItemsFound ~= 3 or pullEntriesFound ~= 3 or swingEntriesFound ~= 3 then
    error(string.format(
        "%s incomplete patch items=%d/3 pull=%d/3 swingVerified=%d/3",
        TAG,
        targetItemsFound,
        pullEntriesFound,
        swingEntriesFound
    ))
end

print(string.format(
    "%s complete multiplier=%.3fx items=%d pullEntries=%d swingEntriesUnchanged=%d | %s",
    TAG,
    config.pullDistanceMultiplier,
    targetItemsFound,
    pullEntriesFound,
    swingEntriesFound,
    table.concat(itemSummaries, " | ")
))
