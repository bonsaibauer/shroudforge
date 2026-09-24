local PlayerInput = game.types.get("keen::ecs::PlayerInput")
local StaminaDepletion = game.types.get("keen::ecs::StaminaDepletion")
local NetworkStamina = game.types.get("keen::ecs::NetworkStamina")

if PlayerInput == nil or StaminaDepletion == nil or NetworkStamina == nil then
    error("Required stamina runtime types are unavailable")
end

local warned = false
local prevent_depletion = shroudforge.settings.get("preventDepletion")
local refill_to_maximum = shroudforge.settings.get("refillToMaximum")

local function update_stamina()
    if not prevent_depletion and not refill_to_maximum then
        return
    end

    local entities, reason
    if prevent_depletion and refill_to_maximum then
        entities, reason = runtime.ecs.query(PlayerInput, StaminaDepletion, NetworkStamina)
    elseif prevent_depletion then
        entities, reason = runtime.ecs.query(PlayerInput, StaminaDepletion)
    else
        entities, reason = runtime.ecs.query(PlayerInput, NetworkStamina)
    end
    if entities == nil then
        if not warned then
            shroudforge.log.warn("No stamina loss unavailable: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false

    for _, entity in ipairs(entities) do
        if prevent_depletion then
            local depletion = runtime.ecs.read(entity, StaminaDepletion)
            if depletion and depletion.accumulatedValue ~= 0 then
                depletion.accumulatedValue = 0
                runtime.ecs.write(entity, StaminaDepletion, depletion)
            end
        end

        if refill_to_maximum then
            local stamina = runtime.ecs.read(entity, NetworkStamina)
            if stamina and stamina.stamina < stamina.staminaMax then
                stamina.stamina = stamina.staminaMax
                runtime.ecs.write(entity, NetworkStamina, stamina)
            end
        end
    end
end

return {
    update_interval_ms = 50,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("No stamina loss active")
    end,
    on_update = update_stamina,
}
