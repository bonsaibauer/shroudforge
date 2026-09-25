local ClientPlayerInput = game.types.get("keen::ecs::ClientPlayerInput")
local ServerConsumedPlayerInput = game.types.get("keen::ecs::ServerConsumedPlayerInput")
local Inventory = game.types.get("keen::ecs::Inventory")
local UsedItem = game.types.get("keen::ecs::UsedItem")

if ClientPlayerInput == nil or ServerConsumedPlayerInput == nil or Inventory == nil then
    error("Required item-use runtime types are unavailable")
end

local pending = {}
local restored_versions = {}
local warned = false
local warned_write = false

local function write_component(entity, component, value)
    local ok, reason = runtime.ecs.write(entity, component, value)
    if not ok and not warned_write then
        shroudforge.log.warn("Infinite item use write failed: " .. (reason or "ECS write failed"))
        warned_write = true
    elseif ok then
        warned_write = false
    end
    return ok
end
local excluded_item_ids = {}
local excluded_items_key = nil

local function refresh_excluded_item_ids()
    local configured = shroudforge.settings.get("excludedItemIds") or ""
    if configured == excluded_items_key then return end
    excluded_item_ids = {}
    for value in string.gmatch(configured, "[^,%s]+") do
        local item_id = tonumber(value)
        if item_id ~= nil then excluded_item_ids[item_id] = true end
    end
    excluded_items_key = configured
end

local function snapshot_action(player, action, consumed_version)
    local entity = action.inventorySlotId.entityId.id
    local slot_index = action.inventorySlotId.slotIndex
    local handle = runtime.ecs.resolve(entity)
    if handle == nil then return nil end
    local inventory = runtime.ecs.read(handle, Inventory)
    if inventory == nil then return nil end
    local slot = inventory.slots[slot_index + 1]
    if slot == nil then return nil end

    local item_id = slot.id
    if item_id == 0 and UsedItem ~= nil then
        local used = runtime.ecs.read(player, UsedItem)
        if used then item_id = used.itemId end
    end

    return {
        version = action.versionData.version,
        entity = entity,
        handle = handle,
        slot = slot_index,
        id = item_id,
        count = slot.data.count,
        pide = slot.data.pide.id,
        alreadyConsumed = consumed_version == action.versionData.version,
    }
end

local function restore_consumed_item(item)
    local handle = runtime.ecs.resolve(item.entity)
    if handle == nil then return false end
    local inventory = runtime.ecs.read(handle, Inventory)
    if inventory == nil then return false end
    local slot = inventory.slots[item.slot + 1]
    if slot == nil then return false end

    if slot.id ~= 0 and slot.id ~= item.id then return false end
    if slot.data.pide.id ~= 0 and slot.data.pide.id ~= item.pide then return false end
    if item.restoreCount == nil then
        item.beforeRestore = slot.data.count
        item.restoreCount = item.alreadyConsumed and (slot.data.count + 1) or item.count
    end
    -- Reconcile an earlier timed-out write before retrying. Never increment twice.
    if slot.id == item.id and slot.data.count == item.restoreCount then return true, false end
    if slot.data.count ~= item.beforeRestore then return false, false end

    slot.data.count = item.restoreCount
    if slot.id == 0 and item.id ~= 0 then slot.id = item.id end
    if slot.data.pide.id == 0 and item.pide ~= 0 then slot.data.pide.id = item.pide end
    local ok = write_component(handle, Inventory, inventory)
    return ok, ok
end

local function update_item_use()
    refresh_excluded_item_ids()
    local players, reason = runtime.ecs.query(ClientPlayerInput, ServerConsumedPlayerInput)
    if players == nil then
        runtime.report_effect("waiting", reason or "ECS query did not complete")
        if not warned then
            shroudforge.log.warn("Infinite item use waiting: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false
    local writes = 0
    local failures = 0

    for _, player in ipairs(players) do
        local input = runtime.ecs.read(player, ClientPlayerInput)
        local consumed = runtime.ecs.read(player, ServerConsumedPlayerInput)
        if input and consumed then
            local action = input.data.itemAction
            local action_version = action.versionData.version
            local consumed_version = consumed.consumedItemAction.version

            if action.type == "Consume"
                and restored_versions[player] ~= action_version
                and (pending[player] == nil or pending[player].version ~= action_version)
            then
                local item = snapshot_action(player, action, consumed_version)
                if item and not excluded_item_ids[item.id] then
                    pending[player] = item
                end
            end

            local item = pending[player]
            if item and item.version == consumed_version then
                local restored, wrote = restore_consumed_item(item)
                if wrote then writes = writes + 1 end
                if not restored and item.restoreCount ~= nil then failures = failures + 1 end
                if restored then
                    restored_versions[player] = item.version
                    pending[player] = nil
                end
            end
        end
    end
    if failures > 0 then
        runtime.report_effect("write-failed", failures .. " item-use restoration(s) could not be reconciled")
    elseif writes > 0 then
        runtime.report_effect("write-confirmed", writes .. " inventory ECS write(s) succeeded; item use is not independently observed")
    elseif #players == 0 then
        runtime.report_effect("no-target", "No entity matched ClientPlayerInput and ServerConsumedPlayerInput")
    else
        runtime.report_effect("no-change", #players .. " player input entity/entities; no confirmed consumable action needed restoration")
    end
end

return {
    update_interval_ms = 16,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("Infinite item use active")
    end,
    on_update = update_item_use,
    on_unload = function() pending = {}; restored_versions = {} end,
}
