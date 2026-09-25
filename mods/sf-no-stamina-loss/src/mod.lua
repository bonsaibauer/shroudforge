local PlayerInput = game.types.get("keen::ecs::PlayerInput")
local StaminaDepletion = game.types.get("keen::ecs::StaminaDepletion")
local NetworkStamina = game.types.get("keen::ecs::NetworkStamina")

if PlayerInput == nil or StaminaDepletion == nil or NetworkStamina == nil then
    error("Required stamina runtime types are unavailable")
end

local warned = false
local warned_write = false

local function write_component(entity, component, value)
    local ok, reason = runtime.ecs.write(entity, component, value)
    if not ok and not warned_write then
        shroudforge.log.warn("No stamina loss write failed: " .. (reason or tostring(component)))
        warned_write = true
    elseif ok then
        warned_write = false
    end
    return ok
end

local function update_stamina()
    local prevent_depletion = shroudforge.settings.get("preventDepletion")
    local refill_to_maximum = shroudforge.settings.get("refillToMaximum")
    if not prevent_depletion and not refill_to_maximum then
        runtime.report_effect("no-change", "Both stamina options are disabled in this mod's settings")
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
        runtime.report_effect("waiting", reason or "ECS query did not complete")
        if not warned then
            shroudforge.log.warn("No stamina loss unavailable: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false
    local writes = 0
    local failures = 0

    for _, entity in ipairs(entities) do
        if prevent_depletion then
            local depletion = runtime.ecs.read(entity, StaminaDepletion)
            if depletion and depletion.accumulatedValue ~= 0 then
                depletion.accumulatedValue = 0
                if write_component(entity, StaminaDepletion, depletion) then writes = writes + 1 else failures = failures + 1 end
            end
        end

        if refill_to_maximum then
            local stamina = runtime.ecs.read(entity, NetworkStamina)
            if stamina and stamina.stamina < stamina.staminaMax then
                stamina.stamina = stamina.staminaMax
                if write_component(entity, NetworkStamina, stamina) then writes = writes + 1 else failures = failures + 1 end
            end
        end
    end
    if failures > 0 then
        runtime.report_effect("write-failed", failures .. " ECS write(s) failed")
    elseif writes > 0 then
        runtime.report_effect("write-confirmed", writes .. " ECS write(s) succeeded; gameplay effect is not independently observed")
    elseif #entities == 0 then
        runtime.report_effect("no-target", "No entity matched PlayerInput and the enabled stamina components")
    else
        runtime.report_effect("no-change", #entities .. " matching player entity/entities; no stamina field needed a change")
    end
end

return {
    update_interval_ms = 16,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("No stamina loss active")
    end,
    on_update = update_stamina,
}
