-- Enshrouded EML mod: XHL Item Stack Limit 65535
-- Changes the stack limit of all stackable vanilla items to the value specified in the config.
-- maxStackSize in the game data is an unsigned 16-bit integer, so its hard limit is 65535.

local LOG_PREFIX = "[XHL-Max-Stack-65535]"
local U16_MAX = 65535

local DEFAULT_CONFIG = {
    enabled = true,
    maxStackSize = U16_MAX,
    preserveSingleStackItems = true,
}

local function trim(value)
    return (value:gsub("^%s+", ""):gsub("%s+$", ""))
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

local function normalize_stack_size(value)
    local parsed = tonumber(value)
    if not parsed or parsed < 1 then
        warn(LOG_PREFIX .. " invalid maxStackSize: " .. tostring(value) .. ", fallback to " .. DEFAULT_CONFIG.maxStackSize)
        return DEFAULT_CONFIG.maxStackSize
    end

    parsed = math.floor(parsed)
    if parsed > U16_MAX then
        warn(LOG_PREFIX .. " maxStackSize " .. parsed .. " exceeds game limit " .. U16_MAX .. ", capped to " .. U16_MAX)
        return U16_MAX
    end

    return parsed
end

local function parse_config()
    local config = {
        enabled = DEFAULT_CONFIG.enabled,
        maxStackSize = DEFAULT_CONFIG.maxStackSize,
        preserveSingleStackItems = DEFAULT_CONFIG.preserveSingleStackItems,
    }

    if not io.exists("config.txt") then
        return config
    end

    local text = io.read_to_string("config.txt")
    for line in text:gmatch("[^\r\n]+") do
        line = trim(line)
        if line ~= "" and not line:match("^#") and not line:match("^;") then
            local key, value = line:match("^([%w_%-]+)%s*=%s*(.-)%s*$")
            if key and value then
                key = trim(key)
                value = trim(value)

                if key == "enabled" then
                    config.enabled = parse_bool(value, config.enabled)
                elseif key == "maxStackSize" then
                    config.maxStackSize = normalize_stack_size(value)
                elseif key == "preserveSingleStackItems" then
                    config.preserveSingleStackItems = parse_bool(value, config.preserveSingleStackItems)
                end
            end
        end
    end

    return config
end

local config = parse_config()

if not config.enabled then
    print(LOG_PREFIX .. " disabled by config")
    return
end

local targetStackSize = normalize_stack_size(config.maxStackSize)
local itemResources = game.assets.get_resources_by_type("keen::ItemInfo")
local patched = 0
local skippedSingle = 0
local skippedMissing = 0

for _, item in ipairs(itemResources) do
    local data = item.data

    local ok, currentStackSize = pcall(function()
        return data.maxStackSize
    end)

    if ok and type(currentStackSize) == "number" then
        if currentStackSize > 1 or not config.preserveSingleStackItems then
            data.maxStackSize = targetStackSize
            patched = patched + 1
        else
            skippedSingle = skippedSingle + 1
        end
    else
        skippedMissing = skippedMissing + 1
    end
end

print(LOG_PREFIX .. " patched " .. patched .. " stackable items to " .. targetStackSize)
print(LOG_PREFIX .. " skipped single-slot items: " .. skippedSingle .. ", items without maxStackSize: " .. skippedMissing)
