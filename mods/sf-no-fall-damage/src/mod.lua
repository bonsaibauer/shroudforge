local PlayerInput = game.types.get("keen::ecs::PlayerInput")
local DynamicFallDamage = game.types.get("keen::ecs::DynamicFallDamage")

if PlayerInput == nil or DynamicFallDamage == nil then
    error("Required fall-damage runtime types are unavailable")
end

local warned = false
local warned_write = false

local function clear_fall_damage()
    local entities, reason = runtime.ecs.query(PlayerInput, DynamicFallDamage)
    if entities == nil then
        runtime.report_effect("waiting", reason or "ECS query did not complete")
        if not warned then
            shroudforge.log.warn("No fall damage unavailable: " .. (reason or "ECS query failed"))
            warned = true
        end
        return
    end
    warned = false
    local writes = 0
    local failures = 0

    for _, entity in ipairs(entities) do
        local fall = runtime.ecs.read(entity, DynamicFallDamage)
        if fall then
            local changed = fall.wasFalling
                or not fall.resetFallAltitudeOnApex
                or fall.fallStartAltitude ~= 0
                or fall.detectedFallDistance ~= 0
                or fall.detectedFallDamagePercentage ~= 0
            if changed then
                fall.wasFalling = false
                fall.resetFallAltitudeOnApex = true
                fall.fallStartAltitude = 0
                fall.detectedFallDistance = 0
                fall.detectedFallDamagePercentage = 0
                local ok, write_reason = runtime.ecs.write(entity, DynamicFallDamage, fall)
                if not ok and not warned_write then
                    shroudforge.log.warn("No fall damage write failed: " .. (write_reason or "ECS write failed"))
                    warned_write = true
                elseif ok then
                    warned_write = false
                end
                if ok then writes = writes + 1 else failures = failures + 1 end
            end
        end
    end
    if failures > 0 then
        runtime.report_effect("write-failed", failures .. " fall-state ECS write(s) failed")
    elseif writes > 0 then
        runtime.report_effect("write-confirmed", writes .. " fall-state ECS write(s) succeeded; damage prevention is not independently observed")
    elseif #entities == 0 then
        runtime.report_effect("no-target", "No entity matched PlayerInput and DynamicFallDamage")
    else
        runtime.report_effect("no-change", #entities .. " matching entity/entities; no fall-state fields needed a change")
    end
end

shroudforge.ui.on_action("clearFallState", clear_fall_damage)

return {
    update_interval_ms = 16,
    on_load = function()
        runtime.require("runtime.lifecycle")
        shroudforge.log.info("No fall damage active")
    end,
    on_update = clear_fall_damage,
}
