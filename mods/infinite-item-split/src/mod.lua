local ClientPlayerInput = game.types.get("keen::ecs::ClientPlayerInput")
local ServerConsumedPlayerInput = game.types.get("keen::ecs::ServerConsumedPlayerInput")
local Inventory = game.types.get("keen::ecs::Inventory")

if ClientPlayerInput == nil or ServerConsumedPlayerInput == nil or Inventory == nil then
    error("Required item-split runtime types are unavailable")
end

local split_types = {}
local configured_split_types = shroudforge.settings.get("splitTypes")
for _, split_type in ipairs(configured_split_types) do
    split_types[split_type] = true
end
local restored_versions = {}
local warned = false

local function restore_subtracted_amount(action)
    local entity = action.sourceSlotId.entityId.id
    local slot_index = action.sourceSlotId.slotIndex
    local amount = action.amount
    if entity == 0 or amount == 0 then return end

    local handle = runtime.ecs.resolve(entity)
    if handle == nil then return end
    local inventory = runtime.ecs.read(handle, Inventory)
    if inventory == nil then return end
    local slot = inventory.slots[slot_index + 1]
    if slot == nil then return end

    -- Keen already created the right-hand split stack. Restore only the amount
    -- subtracted from the left-hand source stack.
    slot.data.count = slot.data.count + amount
    runtime.ecs.write(handle, Inventory, inventory)
end

local function update_item_split()
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
                restore_subtracted_amount(action)
                restored_versions[player] = action_version
            end
        end
    end
end

return {
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("Infinite item split active")
    end,
    on_update = update_item_split,
}
