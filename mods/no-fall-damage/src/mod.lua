local PlayerInput = game.types.get("keen::ecs::PlayerInput")
local DynamicFallDamage = game.types.get("keen::ecs::DynamicFallDamage")

if PlayerInput == nil or DynamicFallDamage == nil then
    error("Required fall-damage runtime types are unavailable")
end

local warned = false

local function clear_fall_damage()
    local entities, reason = runtime.ecs.query(PlayerInput, DynamicFallDamage)
    if entities == nil then
        if not warned then
            shroudforge.log.warn("No fall damage unavailable: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false

    for _, entity in ipairs(entities) do
        local fall = runtime.ecs.read(entity, DynamicFallDamage)
        if fall then
            fall.wasFalling = false
            fall.resetFallAltitudeOnApex = true
            fall.fallStartAltitude = 0
            fall.detectedFallDistance = 0
            fall.detectedFallDamagePercentage = 0
            runtime.ecs.write(entity, DynamicFallDamage, fall)
        end
    end
end

shroudforge.ui.on_action("clearFallState", clear_fall_damage)

return {
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("No fall damage active")
    end,
    on_update = clear_fall_damage,
}
