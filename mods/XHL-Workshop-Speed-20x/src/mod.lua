-- 雾锁王国 EML 模组：XHL 20倍工坊生产速度
-- 将所有大于零的配方制作时长除以配置倍率。
-- 游戏仍会应用当前世界的 factoryProductionSpeedFactor，
-- 因此相对于同一存档未安装本 Mod 时，制作速度仍按配置倍率提高。

local TAG = "[XHL-Workshop-Speed-20x]"

local defaults = {
    enabled = true,
    multiplier = 20.0,
}

local function trim(value)
    return (tostring(value):gsub("^%s+", ""):gsub("%s+$", ""))
end

local function parseBoolean(value, fallback)
    value = trim(value):lower()
    if value == "true" or value == "1" or value == "yes" or value == "on" then return true end
    if value == "false" or value == "0" or value == "no" or value == "off" then return false end
    return fallback
end

local function loadConfig()
    local config = {
        enabled = defaults.enabled,
        multiplier = defaults.multiplier,
    }

    if not io.exists("config.txt") then return config end
    local text = io.read_to_string("config.txt")
    if not text then return config end

    for line in text:gmatch("[^\r\n]+") do
        line = trim(line)
        if line ~= "" and not line:match("^[#;]") then
            local key, raw = line:match("^([%w_%-]+)%s*=%s*(.-)%s*$")
            if key and raw then
                raw = trim(raw:match("^[^#;]*") or raw)
                if key == "enabled" then
                    config.enabled = parseBoolean(raw, config.enabled)
                elseif key == "multiplier" then
                    local value = tonumber(raw)
                    if value and value >= 1.0 and value <= 1000.0 then
                        config.multiplier = value
                    else
                        warn(TAG .. " invalid multiplier; using " .. tostring(config.multiplier))
                    end
                end
            end
        end
    end
    return config
end

local function field(object, key)
    local ok, value = pcall(function() return object[key] end)
    if ok then return value end
    return nil
end

local config = loadConfig()
if not config.enabled then
    print(TAG .. " disabled")
    return
end

local registries = game.assets.get_resources_by_type("keen::RecipeRegistryResource")
local registryCount = 0
local recipeCount = 0
local patchedCount = 0
local shortestOld = nil
local shortestNew = nil
local longestOld = 0
local longestNew = 0

for _, resource in ipairs(registries) do
    local data = field(resource, "data") or resource
    local recipes = field(data, "recipes")
    if recipes then
        registryCount = registryCount + 1
        for _, recipe in ipairs(recipes) do
            recipeCount = recipeCount + 1
            local duration = field(recipe, "craftingDuration")
            local oldValue = duration and tonumber(field(duration, "value")) or nil
            if oldValue and oldValue > 0 then
                -- 制作时长以纳秒为单位。四舍五入到最接近的纳秒，
                -- 可以避免产生零时长条目，同时保持配置的制作倍率。
                local newValue = math.max(1, math.floor((oldValue / config.multiplier) + 0.5))
                duration.value = newValue
                patchedCount = patchedCount + 1

                if not shortestOld or oldValue < shortestOld then
                    shortestOld = oldValue
                    shortestNew = newValue
                end
                if oldValue > longestOld then
                    longestOld = oldValue
                    longestNew = newValue
                end
            end
        end
    end
end

if registryCount == 0 or patchedCount == 0 then
    error(TAG .. " no timed recipes found; refusing an empty patch")
end

print(string.format(
    "%s complete registries=%d recipes=%d timedPatched=%d multiplier=%.3fx shortest=%s->%s ns longest=%s->%s ns",
    TAG,
    registryCount,
    recipeCount,
    patchedCount,
    config.multiplier,
    tostring(shortestOld),
    tostring(shortestNew),
    tostring(longestOld),
    tostring(longestNew)
))