local ClientPlayerInput = game.types.get("keen::ecs::ClientPlayerInput")
local ServerConsumedPlayerInput = game.types.get("keen::ecs::ServerConsumedPlayerInput")
local Inventory = game.types.get("keen::ecs::Inventory")

if ClientPlayerInput == nil or ServerConsumedPlayerInput == nil or Inventory == nil then
    error("Required item-split runtime types are unavailable")
end

local split_types = {}
local configured_signature = ""
local restored_versions = {}
local pending = {}
local warned = false

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
    if slot.data.count == expected.count then return true end
    -- Do not overwrite unrelated inventory changes while retrying a timed-out write.
    if slot.data.count ~= expected.before then return false end

    -- Keen already created the right-hand split stack. Restore only the amount
    -- subtracted from the left-hand source stack.
    slot.data.count = expected.count
    return runtime.ecs.write(handle, Inventory, inventory)
end

local function update_item_split()
    local configured_split_types = shroudforge.settings.get("splitTypes") or {}
    local signature_parts = {}
    for index, split_type in ipairs(configured_split_types) do signature_parts[index] = tostring(split_type) end
    local next_signature = table.concat(signature_parts, ",")
    if next_signature ~= configured_signature then
        split_types = {}
        for _, split_type in ipairs(configured_split_types) do split_types[split_type] = true end
        configured_signature = next_signature
    end

    local players, reason = runtime.ecs.query(ClientPlayerInput, ServerConsumedPlayerInput)
    if players == nil then
        if not warned then
            shroudforge.log.warn("Infinite item split waiting: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false

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
                if restore_subtracted_amount(player, action) then
                    restored_versions[player] = action_version
                    pending[player] = nil
                end
            end
        end
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
