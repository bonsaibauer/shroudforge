local PlayerInput = game.types.get("keen::ecs::PlayerInput")
local DynamicLocomotion = game.types.get("keen::ecs::DynamicLocomotion")
local DynamicFallDamage = game.types.get("keen::ecs::DynamicFallDamage")

if PlayerInput == nil or DynamicLocomotion == nil then
    error("Required flight runtime types are unavailable")
end

local original_states = {}
local warned = false
local prevent_fall_damage = shroudforge.settings.get("preventFallDamage")
local allow_descent = shroudforge.settings.get("allowDescent")

local function apply_flight()
    local entities, reason = runtime.ecs.query(PlayerInput, DynamicLocomotion)
    if entities == nil then
        if not warned then
            shroudforge.log.warn("Flight unavailable: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false

    for _, entity in ipairs(entities) do
        local locomotion = runtime.ecs.read(entity, DynamicLocomotion)
        if locomotion then
            if original_states[entity] == nil then
                original_states[entity] = {
                    state = locomotion.state,
                    previousState = locomotion.previousState,
                }
            end
            local changed = locomotion.previousState ~= locomotion.state
                or locomotion.state ~= "Flying"
            locomotion.previousState = locomotion.state
            locomotion.state = "Flying"
            if not allow_descent and locomotion.inputVelocity.z < 0 then
                locomotion.inputVelocity.z = 0
                changed = true
            end
            if changed then
                runtime.ecs.write(entity, DynamicLocomotion, locomotion)
            end
        end

        if prevent_fall_damage and DynamicFallDamage ~= nil then
            local fall = runtime.ecs.read(entity, DynamicFallDamage)
            if fall then
                local changed = fall.wasFalling
                    or fall.detectedFallDistance ~= 0
                    or fall.detectedFallDamagePercentage ~= 0
                if changed then
                    fall.wasFalling = false
                    fall.detectedFallDistance = 0
                    fall.detectedFallDamagePercentage = 0
                    runtime.ecs.write(entity, DynamicFallDamage, fall)
                end
            end
        end
    end
end

local function restore_states()
    for entity, original in pairs(original_states) do
        local locomotion = runtime.ecs.read(entity, DynamicLocomotion)
        if locomotion then
            locomotion.state = original.state
            locomotion.previousState = original.previousState
            runtime.ecs.write(entity, DynamicLocomotion, locomotion)
        end
    end
    original_states = {}
end

shroudforge.ui.on_action("resetFlight", restore_states)

return {
    update_interval_ms = 33,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("Flight active through keen::ecs::DynamicLocomotion")
    end,
    on_update = apply_flight,
    on_unload = restore_states,
}
