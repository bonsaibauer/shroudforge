-- Read-only example mod entry point; manifest capability: "runtime".
-- Install both updated loader/provider DLLs and restart before running.
local reported = false

return {
    update_interval_ms = 1000,
    on_update = function()
        if reported then return end
        local registrations, reason = runtime.ecs.get_registry()
        if not registrations then return end -- The world/provider may still be starting.

        local all_types = runtime.types.get_all()
        local template_only, split = 0, 0
        for _, entry in ipairs(registrations.entries) do
            if not entry.runtime_type then template_only = template_only + 1 end
            if entry.runtime_type and entry.template_type then split = split + 1 end
        end
        local code = runtime.functions.list_native(0, 1)
        shroudforge.log.info(string.format(
            "Registry: %d reflected types; %d registrations; %d runtime layouts; %d template-only; %d split layouts; %s native code candidates",
            #all_types, registrations.count, registrations.runtime_type_count,
            template_only, split, code and tostring(code.count) or "unavailable"))

        -- No hash or component ID is hard-coded. Original names resolve to their runtime layouts.
        local component = runtime.ecs.get_component("keen::ecs::ActiveNpcState")
        if component and component.runtime_type then
            shroudforge.log.info(component.qualified_name .. " -> " .. component.runtime_type.qualified_name)
        end
        -- Every registered callback is addressable even when its ABI remains partial.
        for _, registration in ipairs(registrations.entries) do
            for _, callback in ipairs(registration.callbacks) do
                local binding = runtime.functions.get(callback.function_rva)
                if binding then
                    shroudforge.log.info(registration.qualified_name, binding.name,
                        binding.callable, binding.execution or binding.reason)
                end
            end
        end
        reported = true
    end,
}
