local ClientPlayerInput = game.types.get("keen::ecs::ClientPlayerInput")
local ServerConsumedPlayerInput = game.types.get("keen::ecs::ServerConsumedPlayerInput")
local Inventory = game.types.get("keen::ecs::Inventory")

if ClientPlayerInput == nil or ServerConsumedPlayerInput == nil or Inventory == nil then
    error("Required item-split runtime types are unavailable")
end

local split_types = {}
local configured_split_key = ""
local restored_versions = {}
local pending = {}
local warned = false
local warned_write = false

local function write_component(entity, component, value)
    local ok, reason = runtime.ecs.write(entity, component, value)
    if not ok and not warned_write then
        shroudforge.log.warn("Infinite item split write failed: " .. (reason or "ECS write failed"))
        warned_write = true
    elseif ok then
        warned_write = false
    end
    return ok
end

local function restore_subtracted_amount(player, action)
    local entity = action.sourceSlotId.entityId.id
    local slot_index = action.sourceSlotId.slotIndex
    local amount = action.amount
    if entity == 0 or amount == 0 then return false end

    local handle = runtime.ecs.resolve(entity)
    if handle == nil then return false end
    local inventory = runtime.ecs.read(handle, Inventory)
    if inventory == nil then return false end
    local slot = inventory.slots[slot_index + 1]
    if slot == nil then return false end

    local expected = pending[player]
    if expected == nil or expected.version ~= action.versionData.version then
        expected = {version = action.versionData.version, id = slot.id, pide = slot.data.pide.id,
            before = slot.data.count, count = slot.data.count + amount}
        pending[player] = expected
    end
    if slot.id ~= expected.id or slot.data.pide.id ~= expected.pide then return false end
    if slot.data.count == expected.count then return true, false end
    -- Do not overwrite unrelated inventory changes while retrying a timed-out write.
    if slot.data.count ~= expected.before then return false end

    -- Keen already created the right-hand split stack. Restore only the amount
    -- subtracted from the left-hand source stack.
    slot.data.count = expected.count
    local ok = write_component(handle, Inventory, inventory)
    return ok, ok
end

local function update_item_split()
    local configured_split_types = shroudforge.settings.get("splitTypes") or {}
    local split_key_parts = {}
    for index, split_type in ipairs(configured_split_types) do split_key_parts[index] = tostring(split_type) end
    local next_split_key = table.concat(split_key_parts, ",")
    if next_split_key ~= configured_split_key then
        split_types = {}
        for _, split_type in ipairs(configured_split_types) do split_types[split_type] = true end
        configured_split_key = next_split_key
    end

    local players, reason = runtime.ecs.query(ClientPlayerInput, ServerConsumedPlayerInput)
    if players == nil then
        runtime.report_effect("waiting", reason or "ECS query did not complete")
        if not warned then
            shroudforge.log.warn("Infinite item split waiting: " .. (reason or "ECS query failed"))
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
            local action = input.data.inventoryTransferAction
            local action_version = action.versionData.version
            local consumed_version = consumed.consumedInventoryTransferAction.version
            local same_inventory = action.sourceEntityId.id == action.targetEntityId.id

            if same_inventory
                and split_types[action.type]
                and action_version == consumed_version
                and restored_versions[player] ~= action_version
            then
                local restored, wrote = restore_subtracted_amount(player, action)
                if wrote then writes = writes + 1 end
                if not restored and pending[player] ~= nil then failures = failures + 1 end
                if restored then
                    restored_versions[player] = action_version
                    pending[player] = nil
                end
            end
        end
    end
    if failures > 0 then
        runtime.report_effect("write-failed", failures .. " split inventory update(s) could not be reconciled")
    elseif writes > 0 then
        runtime.report_effect("write-confirmed", writes .. " split inventory ECS write(s) succeeded; split preservation is not independently observed")
    elseif #players == 0 then
        runtime.report_effect("no-target", "No entity matched ClientPlayerInput and ServerConsumedPlayerInput")
    else
        runtime.report_effect("no-change", #players .. " player input entity/entities; no eligible confirmed split needed restoration")
    end
end

return {
    update_interval_ms = 16,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("Infinite item split active")
    end,
    on_update = update_item_split,
    on_unload = function() pending = {}; restored_versions = {} end,
}
