-- XHL Vein Mining v1.0.33
-- Chained terrain mining is provided by this mod's standalone Native Sidecar.
-- This EML patch sets the reach of all seven pickaxes to 10 meters.
-- The native DLL reads the local ClientCursorInput after the game resolves the terrain material.

runtime.require("game.assets.write")

local TAG = "[XHL-Vein-Mining]"
local DEFAULT_WHITELIST_COUNT = 34
local PICKAXE_MAX_DISTANCE = 10.0
local EXPECTED_PICKAXE_COUNT = 7

local PICKAXE_ITEM_GUIDS = {
    ["59dba0f3-81b8-4ac5-88d7-ba97f4d2928c"] = true, -- T0 Stone Pickaxe
    ["46ea9b46-445a-4e36-bf84-20cab7585f48"] = true, -- T1 Scrap Pickaxe
    ["70ecc7fb-6ed1-416c-a51e-0362b2ebdd1a"] = true, -- T2 Copper Pickaxe
    ["c03c05ce-4cc0-40fd-b201-58d05c730767"] = true, -- T3 Bronze Pickaxe
    ["2e4b3166-4960-48b1-ad1f-f3bb9d0d981a"] = true, -- T4 Iron Pickaxe
    ["aaff198b-9139-47f5-8e41-b1ce0262abc8"] = true, -- T5 Steel Pickaxe
    ["aee6b3ba-adeb-4264-b117-25f974a08cb8"] = true, -- T6 Steel/Gold Pickaxe
}

local function get_field(object, field)
    local ok, value = pcall(function()
        return object[field]
    end)
    if ok then
        return value
    end
    return nil
end

local function patch_pickaxe_reach()
    local resources = game.assets.get_resources_by_type("keen::ItemInfo")
    local matched = 0
    local changed = 0

    for _, resource in ipairs(resources) do
        local resource_guid = tostring(get_field(resource, "guid"))
        if PICKAXE_ITEM_GUIDS[resource_guid] then
            local data = get_field(resource, "data")
            local equipment = data ~= nil and get_field(data, "equipment") or nil
            if equipment ~= nil then
                matched = matched + 1
                local before = tonumber(get_field(equipment, "maxDistance"))
                equipment.maxDistance = PICKAXE_MAX_DISTANCE
                if before ~= PICKAXE_MAX_DISTANCE then
                    changed = changed + 1
                end
            end
        end
    end

    return matched, changed
end

local matched, changed = patch_pickaxe_reach()
if matched ~= EXPECTED_PICKAXE_COUNT then
    print(TAG .. " warning: expectedPickaxes=" ..
        tostring(EXPECTED_PICKAXE_COUNT) .. ", matched=" .. tostring(matched))
end

print(TAG .. " v1.0.33 active; defaultWhitelist=" ..
    tostring(DEFAULT_WHITELIST_COUNT) .. "; materialConfig=config.txt" ..
    "; holdKey=VK_OEM_3; radius=5m-sphere; diameter=10m; pickaxeReach=10m; pickaxes=" ..
    tostring(matched) .. "; reachChanged=" .. tostring(changed) ..
    "; cursorSource=stock-local-post-resolution; privateRay=off; cursorMirrorScan=off; materialBias=off; colors=config-shared-green-other-gray; namedMaterials=112; rangeGeometry=off; crosshair=17px-4px; payout=stock-rate-split; chainTarget=actual-pickaxe-sphere-probe; selection=all-struck-material; multiplayer=stock-PlayerInput-message-XHLC-one-shot; sidecar=triggerEmote-host-strip-all-XHLC; requestRefresh=200ms; requestTTL=1000ms; senderBinding=reflected-playerId-tristate-full-u32; cache=nonce-mod24-dedup-consume-once; terraform=direct-12B-trampoline; patchOrder=unpack-terraform-pack; patchWindow=peer-threads-suspended; recordScan=off; bit63=off; ecsPlayerInputLookup=off; nativePluginHost=abi-v1; keyboardFallback=on; bothMachines=v1.0.33")
