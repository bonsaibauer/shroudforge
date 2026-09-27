runtime.require("runtime.lifecycle")

local ClientPlayerInput
local ServerConsumedPlayerInput
local Inventory
local restored_versions = {}
local pending = {}
local split_types = {One = true, Half = true, CustomAmount = true}
local warned_query = false
local warned_write = false
local last_effect = nil

local function report_once(state, detail)
    local key = state .. "\0" .. tostring(detail or "")
    if key == last_effect then return end
    last_effect = key
    runtime.report_effect(state, detail or "")
end

local function read_source_slot(action)
    local slot_id = action and action.sourceSlotId
    local entity_id = slot_id and slot_id.entityId and slot_id.entityId.id
    local slot_index = slot_id and tonumber(slot_id.slotIndex)
    if not entity_id or entity_id == 0 or not slot_index or slot_index < 0 or slot_index % 1 ~= 0 then return nil end
    local handle = runtime.ecs.resolve(entity_id)
    if not handle then return nil end
    local inventory = runtime.ecs.read(handle, Inventory)
    local slot = inventory and inventory.slots[slot_index + 1]
    if not slot or not slot.data or not slot.data.pide then return nil end
    return handle, inventory, slot
end

local function restore_source_amount(player, action)
    local handle, inventory, slot = read_source_slot(action)
    if not handle then return false end

    local version = action.versionData.version
    local amount = tonumber(action.amount)
    if not amount or amount < 1 or amount % 1 ~= 0 then return false end

    local expected = pending[player]
    if expected == nil or expected.version ~= version then
        local before = tonumber(slot.data.count)
        if not before or before < 0 or before + amount > 4294967295 then return false end
        expected = {
            version = version,
            id = slot.id,
            pide = slot.data.pide.id,
            before = before,
            count = before + amount,
        }
        pending[player] = expected
    end
    if slot.id ~= expected.id or slot.data.pide.id ~= expected.pide then return false end
    if slot.data.count == expected.count then return true, false end
    if slot.data.count ~= expected.before then return false end

    slot.data.count = expected.count
    local ok, reason = runtime.ecs.write(handle, Inventory, inventory)
    if not ok then
        if not warned_write then
            shroudforge.log.warn("Infinite Item Split inventory restore failed: " .. tostring(reason))
            warned_write = true
        end
        return false
    end
    warned_write = false
    return true, true
end

local function update_item_split()
    local players, reason = runtime.ecs.query(ClientPlayerInput, ServerConsumedPlayerInput)
    if not players then
        report_once("waiting", reason or "player inventory-transfer query failed")
        if not warned_query then
            shroudforge.log.warn("Infinite Item Split waiting for player input: " .. tostring(reason))
            warned_query = true
        end
        return
    end
    warned_query = false

    local writes = 0
    local failures = 0
    for _, player in ipairs(players) do
        local input = runtime.ecs.read(player, ClientPlayerInput)
        local consumed = runtime.ecs.read(player, ServerConsumedPlayerInput)
        local action = input and input.data and input.data.inventoryTransferAction
        local consumed_action = consumed and consumed.consumedInventoryTransferAction
        if action and consumed_action and action.versionData and
           action.sourceEntityId and action.targetEntityId and
           action.sourceEntityId.id ~= nil and action.targetEntityId.id ~= nil then
            local action_version = action.versionData.version
            local consumed_version = consumed_action.version
            local same_inventory = action.sourceEntityId.id == action.targetEntityId.id

            if same_inventory and split_types[action.type] and action_version == consumed_version and
               restored_versions[player] ~= action_version then
                local restored, wrote = restore_source_amount(player, action)
                if wrote then writes = writes + 1 end
                if not restored then failures = failures + 1 end
                if restored then
                    restored_versions[player] = action_version
                    pending[player] = nil
                end
            end
        end
    end

    if failures > 0 then
        report_once("write-failed", failures .. " split inventory update(s) could not be reconciled")
    elseif writes > 0 then
        report_once("write-confirmed", writes .. " selected split action(s) restored the source stack")
    elseif #players == 0 then
        report_once("no-target", "No entity matched ClientPlayerInput and ServerConsumedPlayerInput")
    elseif failures == 0 then
        last_effect = nil
    end
end

return {
    update_interval_ms = 16,
    on_load = function()
        ClientPlayerInput = game.types.get("keen::ecs::ClientPlayerInput")
        ServerConsumedPlayerInput = game.types.get("keen::ecs::ServerConsumedPlayerInput")
        Inventory = game.types.get("keen::ecs::Inventory")
        if not ClientPlayerInput or not ServerConsumedPlayerInput or not Inventory then
            error("Infinite Item Split requires ClientPlayerInput, ServerConsumedPlayerInput, and Inventory")
        end
        shroudforge.log.info("Infinite Item Split active for all in-game One, Half, and CustomAmount split actions")
    end,
    on_update = update_item_split,
    on_unload = function()
        pending = {}
        restored_versions = {}
    end,
}
