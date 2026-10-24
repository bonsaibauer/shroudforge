-- These targets use the same public checked writers available to every mod.
-- The native claims coordinate independent mods by original storage identity.
return function(runtime, assets)
    local api = runtime.overrides
    -- Attribute descriptors are immutable for this mod environment. Live
    -- snapshots and ownership claims remain session-bound.
    local attribute_descriptors, attribute_descriptor_count = {}, 0
    local maximum_attribute_descriptors = 512
    local signedness_failure = "signedness-dependent program with conflicting resource/storage scalar types"
    local function scalar(value)
        return type(value) == "boolean" or type(value) == "string"
            or (type(value) == "number" and value == value and math.abs(value) < math.huge)
    end

    local function valid_attribute_snapshot(snapshot, entity, root_hash, component_name)
        return type(snapshot) == "table"
            and snapshot.entity == entity
            and snapshot.root_hash == root_hash
            and type(snapshot.values) == "table"
            and type(snapshot.component) == "table"
            and type(snapshot.component.qualified_name) == "string"
            and (component_name == nil or snapshot.component.qualified_name == component_name)
    end

    local function attribute(entity, selector, snapshot_context)
        local descriptor = attribute_descriptors[selector]
        if not descriptor then
            local resolved, reason = runtime.ecs.get_attribute(selector)
            if not resolved then return nil, reason end
            descriptor = {
                name=resolved.name,
                root_hash=resolved.root_hash,
                calculation_writable=resolved.calculation_writable,
                calculation_reason=resolved.calculation_reason,
            }
            if attribute_descriptor_count < maximum_attribute_descriptors then
                attribute_descriptors[selector] = descriptor
                attribute_descriptor_count = attribute_descriptor_count + 1
            end
        end
        local name = descriptor.name
        local reason = descriptor.calculation_reason
        if descriptor.calculation_writable == false
            and reason == signedness_failure then
            return nil, reason
        end

        local session = runtime.world.session_id()
        local shared_context = type(snapshot_context) == "table"
            and snapshot_context.active == true
        if shared_context and (snapshot_context.session ~= session
            or snapshot_context.entity ~= entity
            or snapshot_context.root_hash ~= descriptor.root_hash) then
            snapshot_context.session = session
            snapshot_context.entity = entity
            snapshot_context.root_hash = descriptor.root_hash
            snapshot_context.component_name = nil
            snapshot_context.snapshot = nil
        end

        local initial = shared_context and snapshot_context.snapshot or nil
        if initial and not valid_attribute_snapshot(initial, entity, descriptor.root_hash,
            snapshot_context.component_name) then
            snapshot_context.snapshot = nil
            snapshot_context.component_name = nil
            initial = nil
        end
        if not initial then
            local why
            initial, why = runtime.ecs.read_attributes(entity, name)
            if not initial then return nil, why end
            if not valid_attribute_snapshot(initial, entity, descriptor.root_hash) then
                return nil, "Attribute snapshot does not match the selected entity and root"
            end
            if shared_context then
                snapshot_context.component_name = initial.component.qualified_name
                snapshot_context.snapshot = initial
            end
        end

        local component_name = initial.component.qualified_name
        local claim, failure = api.get_component_claim(entity, component_name)
        if not claim then return nil, failure end
        local function normalize(snapshot, value)
            local candidate = {}
            for key, current in pairs(snapshot.values) do candidate[key] = current end
            candidate[name] = value
            local calculated, error = runtime.ecs.evaluate_attributes(name, candidate)
            if not calculated then return nil, error end
            return calculated.values[name]
        end
        local function fresh_snapshot()
            local snapshot, error = runtime.ecs.read_attributes(entity, name)
            if not snapshot then return nil, error end
            if not valid_attribute_snapshot(snapshot, entity, descriptor.root_hash, component_name) then
                return nil, "Attribute snapshot does not match the selected entity, root, and component"
            end
            return snapshot
        end
        -- The full-root expected_values check rejects a change between this
        -- component-resolution read and the ownership claim.
        local first_snapshot = initial
        return {
            key="attribute/" .. session .. "/" .. entity .. "/" .. name,
            root_hash=descriptor.root_hash,
            component_name=component_name,
            claim=claim,
            normalize=normalize,
            read=function()
                local snapshot
                if shared_context and snapshot_context.active == true then
                    if snapshot_context.session == session
                        and snapshot_context.entity == entity
                        and snapshot_context.root_hash == descriptor.root_hash then
                        snapshot = snapshot_context.snapshot
                        if snapshot and not valid_attribute_snapshot(
                            snapshot, entity, descriptor.root_hash, component_name) then
                            snapshot_context.snapshot = nil
                            snapshot = nil
                        end
                    end
                    -- When a shared context is active, only its latest snapshot is valid.
                    first_snapshot = nil
                else
                    snapshot = first_snapshot
                    first_snapshot = nil
                end
                local error
                if not snapshot then
                    snapshot, error = fresh_snapshot()
                    if not snapshot then return nil, error end
                    if shared_context and snapshot_context.active == true then
                        snapshot_context.component_name = component_name
                        snapshot_context.snapshot = snapshot
                    end
                end
                if not valid_attribute_snapshot(snapshot, entity, descriptor.root_hash, component_name) then
                    return nil, "Attribute snapshot does not match the selected entity, root, and component"
                end
                local value = snapshot.values[name]
                if not scalar(value) then return nil, "Attribute is not a finite scalar: " .. name end
                return {value=value, values=snapshot.values}
            end,
            write=function(snapshot, value)
                local canonical, error = normalize(snapshot, value)
                if canonical == nil then return false, nil, error end
                local updated, failure = runtime.ecs.update_attribute(entity, name, value, snapshot.values)
                if not updated then
                    if shared_context and snapshot_context.active == true then
                        snapshot_context.snapshot = nil
                    end
                    return false, nil, failure
                end
                if shared_context and snapshot_context.active == true then
                    snapshot_context.snapshot = valid_attribute_snapshot(
                        updated, entity, descriptor.root_hash, component_name) and updated or nil
                end
                return true, updated.values[name], nil, canonical
            end,
        }
    end

    local function view_target(view, path, expected_paths, claim_factory)
        local claim, reason = claim_factory(view)
        if not claim then return nil, reason end
        local paths = {path}
        for _, extra in ipairs(expected_paths or {}) do
            if extra ~= path then paths[#paths + 1] = extra end
        end
        return {
            key="view/" .. view.session_id .. "/" .. view.key .. "/" .. path,
            claim=claim, shared=true,
            normalize=function(_, value) return runtime.values.normalize_field(view.type, path, value) end,
            read=function()
                local fields, failure = view:read_fields(paths)
                if not fields then return nil, failure end
                if not scalar(fields[path]) then return nil, "Field is not a finite scalar: " .. path end
                return {value=fields[path], values=fields}
            end,
            write=function(snapshot, value)
                local expected
                if expected_paths and #expected_paths > 0 then
                    expected = {}
                    for _, extra in ipairs(expected_paths) do expected[extra] = snapshot.values[extra] end
                end
                local ok, failure
                if expected then
                    ok, failure = view:write_field(path, value, snapshot.value, expected)
                else
                    ok, failure = view:write_field(path, value, snapshot.value)
                end
                if not ok then return false, nil, failure end
                -- write_field confirms the canonical value under the engine gate.
                return true, value
            end,
        }
    end

    local function resource(resource, path)
        local claim, reason = api.get_resource_claim(resource)
        if not claim then return nil, reason end
        local guid, part, ty, session = resource.guid, resource.part, resource.type, resource.session_id
        return {
            key="resource/" .. guid .. "/" .. ty.qualified_name .. "/" .. part .. "/" .. path,
            claim=claim, shared=true,
            normalize=function(snapshot, value)
                local bytes, why = runtime.values.encode(snapshot.type, value)
                if not bytes then return nil, why end
                return runtime.values.decode(snapshot.type, bytes)
            end,
            read=function()
                if runtime.world.session_id() ~= session then return nil, "Resource target belongs to another world session" end
                local fresh, why = assets.get_loaded_resource(guid, ty, part)
                if not fresh then return nil, why end
                if fresh.session_id ~= session then return nil, "Resource snapshot belongs to another world session" end
                local value, failure, field_type = assets.read_loaded_field(fresh, path)
                if value == nil then return nil, failure end
                if not scalar(value) then return nil, "Resource field is not a finite scalar: " .. path end
                return {value=value, resource=fresh, type=field_type}
            end,
            write=function(snapshot, value)
                if runtime.world.session_id() ~= session or snapshot.resource.session_id ~= session then
                    return false, nil, "Resource target belongs to another world session"
                end
                local updated, failure = assets.write_loaded_field(snapshot.resource, path, value, snapshot.value)
                if not updated then return false, nil, failure end
                local actual, why = assets.read_loaded_field(updated, path)
                if actual == nil then return false, nil, "Uncertain resource write: " .. tostring(why) end
                return true, actual
            end,
        }
    end

    local function component(entity, ty, path, expected_paths)
        local claim, reason = api.get_component_claim(entity, ty)
        if not claim then return nil, reason end
        local fields = {path}
        for _, extra in ipairs(expected_paths or {}) do
            if extra ~= path then fields[#fields + 1] = extra end
        end
        return {
            key="component/" .. runtime.world.session_id() .. "/" .. entity .. "/" .. tostring(ty) .. "/" .. path,
            claim=claim,
            normalize=function(_, value) return runtime.values.normalize_field(ty, path, value) end,
            read=function()
                local values, why = runtime.ecs.read_fields(entity, ty, fields)
                if not values then return nil, why end
                if not scalar(values[path]) then return nil, "Component field is not a finite scalar: " .. path end
                return {value=values[path], values=values}
            end,
            write=function(snapshot, value)
                local ok, why, confirmed = runtime.ecs.write_fields(entity, ty, {[path]=value}, snapshot.values)
                if not ok then return false, nil, why end
                if not confirmed or confirmed[path] == nil then
                    return false, nil, "Uncertain component write: confirmed scalar snapshot is unavailable"
                end
                return true, confirmed[path], nil, confirmed[path]
            end,
        }
    end

    return {
        attribute=attribute, resource=resource, component=component,
        service=function(view, path, expected_paths)
            return view_target(view, path, expected_paths, api.get_service_claim)
        end,
        template=function(view, path, expected_paths)
            return view_target(view, path, expected_paths, api.get_template_claim)
        end,
    }
end
