-- Native capture, placement and dedicated-server operation coordination.
return function(editor)
    function editor.current_session()
        return runtime.world and runtime.world.session_id and runtime.world.session_id() or 0
    end

    function editor.read_prop(handle, expected_session)
        local session = expected_session or editor.current_session()
        if not session or session == 0 or editor.current_session() ~= session then
            return nil, "world-session-changed"
        end
        if not handle then return nil, "entity-handle-missing" end
        local prop, reason = runtime.world.entity.get_transform(handle)
        if editor.current_session() ~= session then return nil, "world-session-changed" end
        if prop then return prop end
        -- A bool-only native provider and transient read errors cannot prove
        -- deletion. Only the session-bound membership status means absent.
        return nil, reason or "entity-state-unavailable"
    end

    function editor.execution_backend()
        if editor.editor_is_server then return "direct" end
        -- Route only from the native ECS context source. The client EXE
        -- can be local or joined to a dedicated server, so process role and general
        -- runtime readiness are not sufficient evidence for selecting a writer.
        if not runtime.world or not runtime.world.context_kind then return "unknown" end
        local session = editor.current_session()
        if not session or session == 0 then return "unknown" end
        local kind = runtime.world.context_kind()
        if kind == "direct" then return "direct" end
        if kind == "client-read-only" then return "p2p" end
        return "unknown"
    end

    function editor.execution_backend_label(backend)
        if backend == "direct" then return "native-direct" end
        if backend == "p2p" then return "server-p2p" end
        return "detecting"
    end

    local unavailable_native_mutations = {
        ["runtime.world.voxel.write"] = "terrain write",
        ["runtime.world.entity.spawn"] = "prop spawn",
        ["runtime.world.entity.destroy"] = "prop removal",
        ["runtime.world.entity.place"] = "prop placement",
        ["runtime.world.entity.finish_building"] = "building completion",
    }

    function editor.native_mutation_unavailable_reason(operation, status)
        local label = unavailable_native_mutations[operation]
        if not label or editor.execution_backend() ~= "direct" then return nil end
        status = status or runtime.status(operation)
        -- Availability is a live, nonblocking Scene observation. A missed gate
        -- is not evidence that the registered adapter is absent. In particular,
        -- has() and status() can see different readiness on consecutive calls.
        if status and (status.available or editor.world_feature_pending(status.reason)) then return nil end
        return "No verified native ECS " .. label .. " operation is available for this world session. No world changes were sent."
    end

    function editor.require_execution_backend(backend, operation)
        if backend ~= "unknown" then return true end
        local message = "Automatic world target detection is waiting for a validated world session. " ..
            tostring(operation or "No world change was sent.")
        editor.set_editor_message(nil, message)
        shroudforge.log.warn("World Editor blocked " .. editor.format_keybind_hint(tostring(operation or "world change")) ..
            ": native world target is unknown, no local write or P2P request was sent")
        return false
    end

    function editor.get_voxel_grid_spec()
        if editor.voxel_grid_spec then return editor.voxel_grid_spec end
        if not runtime.has("runtime.world.voxel.grid_spec") then
            local status = runtime.status("runtime.world.voxel.grid_spec")
            -- These are two separate native observations. A busy Scene can
            -- become ready between them, so a ready status means retry.
            if status and status.available then return nil, "voxel-grid-pending" end
            return nil, status and status.reason or "native voxel grid metadata is unavailable"
        end
        local spec, reason = runtime.world.voxel.get_grid_spec("voxel")
        if not spec then
            -- ABI-12 bridges with the bool-only metadata export cannot carry
            -- Pending separately. Keep this one known voxel-grid rejection
            -- inside the same bounded Capture and save blueprint deadline, without guessing metadata.
            if reason == "Grid is unavailable. This backend currently exposes 'voxel'" then
                return nil, "voxel-grid-pending"
            end
            return nil, reason
        end
        editor.voxel_grid_spec = spec
        return spec
    end

    function editor.unavailable_world_reason(operation)
        if runtime.has(operation) then return nil end
        local status = runtime.status(operation)
        if status and status.available then return "world operation is not ready" end
        return status and status.reason or (operation .. " is unavailable")
    end

    function editor.query_is_pending(reason)
        return type(reason) == "string" and (
            reason == "voxel-read-pending" or reason == "voxel-grid-pending" or
            reason == "voxel read is waiting for the Scene gate or original cache readiness" or
            reason:find("still scanning", 1, true) ~= nil or
            reason:find("has not published a live sample", 1, true) ~= nil or
            reason:find("waiting for the live Keen ECS world", 1, true) ~= nil or
            reason:find("KFC Runtime is not ready", 1, true) ~= nil or
            reason:find("world operation is not ready", 1, true) ~= nil)
    end

    function editor.world_feature_pending(reason)
        return editor.query_is_pending(reason) or type(reason) == "string" and (
            reason:find("world operation is not ready", 1, true) ~= nil or
            reason:find("active voxel world context is not available", 1, true) ~= nil or
            reason == "runtime provider is not ready for this operation")
    end

    function editor.defer_world_action(operation, action, elapsed, guarded_read)
        if editor.pending_world_action then
            editor.set_editor_message(nil, "A world operation is already running. Wait for it to finish, then try again.")
            shroudforge.log.debug("World Editor is already waiting for the live world context")
            return false
        end
        local session = editor.current_session()
        if session == 0 then
            editor.set_editor_message("ready", "The world session is unavailable. Wait for loading to finish, then retry the action.")
            return false
        end
        editor.pending_world_action = {operation = operation, action = action, session = session,
            elapsed = elapsed or 0, guarded_read = guarded_read == true}
        editor.set_editor_message(nil, "Waiting for the live world to become ready. The requested action will continue automatically.")
        if not elapsed or elapsed == 0 then
            shroudforge.log.debug("World Editor is waiting for the validated world context. The requested action will continue automatically.")
        end
        return false
    end

    function editor.require_world_feature(operation, action, elapsed)
        if runtime.has(operation) then return true end
        local status = runtime.status(operation)
        local unavailable = editor.native_mutation_unavailable_reason(operation, status)
        if unavailable then
            editor.set_editor_message("selected", unavailable)
            shroudforge.log.warn("World Editor blocked " .. operation .. ": " .. unavailable)
            return false
        end
        local reason = status and status.reason or (operation .. " is unavailable")
        if (status and status.available) or editor.world_feature_pending(reason) then
            return editor.defer_world_action(operation, action, elapsed)
        end
        editor.feature(operation)
        return false
    end

    function editor.resolve_placeable_items()
        if editor.placeable_items and editor.prop_recipes_registered then return editor.placeable_items end
        local read_index = 0
        local function read_recipes(get_resources)
            read_index = 0
            local resources = get_resources("keen::ItemInfo")
            if type(resources) ~= "table" or #resources == 0 then
                error("ItemInfo query did not return a nonempty resource snapshot")
            end
            local resolved = {}
            local item_ids, placed_entities, valid_bounds_count, feedback_count = 0, 0, 0, 0
            for resource_index, resource in ipairs(resources) do
                read_index = resource_index
                -- Reading `.data` and its nested fields performs the actual runtime
                -- read and reflection decoding. Keep the whole snapshot protected.
                local data = resource.data
                if data == nil then error("ItemInfo resource returned no data") end
                local equipment = data.equipment
                local id = editor.item_id(data.itemId)
                local reference = equipment and equipment.placedEntity
                local high, low = editor.guid_halves(reference)
                local valid_id = id and id > 0 and id % 1 == 0
                if valid_id then item_ids = item_ids + 1 end
                if valid_id and reference ~= nil then placed_entities = placed_entities + 1 end
                local minimum, maximum = equipment and equipment.placementAABBmin, equipment and equipment.placementAABBmax
                local bounds = minimum and maximum and {
                    tonumber(minimum.x), tonumber(minimum.y), tonumber(minimum.z),
                    tonumber(maximum.x), tonumber(maximum.y), tonumber(maximum.z),
                }
                local valid_bounds = bounds ~= nil
                if valid_bounds then
                    for index = 1, 6 do
                        if not editor.finite_number(bounds[index]) then valid_bounds = false; break end
                    end
                end
                if valid_bounds then
                    for axis = 1, 3 do if bounds[axis] > bounds[axis + 3] then valid_bounds = false end end
                end
                if valid_id and reference ~= nil and valid_bounds then
                    valid_bounds_count = valid_bounds_count + 1
                    local feedback
                    for _, collider in ipairs(equipment.placementColliders or {}) do
                        for _, entry in ipairs(collider.dataArray or {}) do
                            -- KFC variants expose their contained typed value as `.value`.
                            -- `$value` exists only in the raw JSON representation.
                            local candidate = entry.value.materialFeedbackId
                            feedback = editor.item_id(candidate)
                            if feedback and feedback ~= 0 then break end
                        end
                        if feedback and feedback ~= 0 then break end
                    end
                    if feedback and feedback ~= 0 then feedback_count = feedback_count + 1 end
                    if feedback and feedback ~= 0 then
                        resolved[id] = {
                            id = id, uuidHigh = high, uuidLow = low, feedback = feedback,
                            bounds = bounds,
                            name = data.debugName or tostring(id),
                        }
                    end
                end
            end
            return resolved, #resources, item_ids, placed_entities, valid_bounds_count, feedback_count
        end
        local source = "loaded-runtime"
        local ok, resolved, resource_count, item_ids, placed_entities, valid_bounds_count, feedback_count =
            pcall(read_recipes, game.assets.get_loaded_resources_by_type)
        if not ok then
            local runtime_failure = string.format("resource index=%d: %s", read_index, tostring(resolved))
            shroudforge.log.warn("World Editor could not read the complete loaded ItemInfo snapshot (" ..
                runtime_failure .. "). Falling back to KFC file assets")
            source = "kfc-file-fallback"
            ok, resolved, resource_count, item_ids, placed_entities, valid_bounds_count, feedback_count =
                pcall(read_recipes, game.assets.get_resources_by_type)
            if not ok then
                shroudforge.log.warn("World Editor could not read a complete ItemInfo snapshot. loaded-runtime: " ..
                    runtime_failure .. ". kfc-file-fallback: resource index=" .. read_index .. ": " .. tostring(resolved))
                return nil
            end
        end
        local count = 0
        for _ in pairs(resolved) do count = count + 1 end
        if count == 0 then
            shroudforge.log.warn(string.format(
                "World Editor found no supported ItemInfo placement recipes (source=%s resources=%d validItemIds=%d withPlacedEntity=%d withValidBounds=%d withFeedback=%d)",
                source, resource_count, item_ids, placed_entities, valid_bounds_count, feedback_count))
            return nil
        end
        local native_recipes = {}
        for id, recipe in pairs(resolved) do
            native_recipes[#native_recipes + 1] = {itemId = id, bounds = recipe.bounds, feedback = recipe.feedback}
        end
        local registered, reason = runtime.world.entity.register_prop_recipes(native_recipes)
        if not registered then
            shroudforge.log.warn("World Editor could not register native ItemInfo placement bounds: " .. tostring(reason))
            return nil
        end
        editor.placeable_items = resolved
        editor.prop_recipes_registered = true
        shroudforge.log.info("World Editor registered " .. count .. " native ItemInfo placement recipes from " .. source)
        return resolved
    end

    function editor.capture_region_props(region)
        local unavailable = editor.unavailable_world_reason("runtime.world.entity.query_props_in_bounds")
        if unavailable then return nil, unavailable end
        local recipes = editor.resolve_placeable_items()
        if not recipes then return nil end
        local bounds, anchor = region.queryBounds, region.anchor
        if not bounds then
            local grid, grid_reason = editor.get_voxel_grid_spec()
            if not grid then return nil, grid_reason or "native voxel grid metadata is unavailable" end
            bounds = {editor.grid_to_world(region.x, 1, grid), editor.grid_to_world(region.y, 2, grid), editor.grid_to_world(region.z, 3, grid),
                editor.grid_to_world(region.x + region.sx, 1, grid), editor.grid_to_world(region.y + region.sy, 2, grid), editor.grid_to_world(region.z + region.sz, 3, grid)}
            anchor = {bounds[1], bounds[2], bounds[3]}
        end
        anchor = anchor or {bounds[1], bounds[2], bounds[3]}
        local live_props, reason = runtime.world.entity.query_props_in_bounds(bounds)
        if not live_props then
            if not editor.query_is_pending(reason) then
                shroudforge.log.warn("World Editor prop enumeration failed: " .. tostring(reason))
            end
            return nil, reason
        end
        local maximum = editor.maximum_copyable_props()
        if #live_props > maximum then
            return nil, string.format("selection contains %d props, above the configured maximum of %d. Increase Maximum props per blueprint in mod settings", #live_props, maximum)
        end
        local props = {}
        for _, live_prop in ipairs(live_props) do
            local transform = live_prop.transform
            local position = transform and editor.world_position(transform.position)
            local id = editor.item_id(live_prop.itemId)
            local recipe = id and recipes[id]
            if position and recipe then
                local rotation = transform.orientation or {}
                local scale = transform.scale or {x = 1, y = 1, z = 1}
                local qx, qy, qz, qw = tonumber(rotation.x) or 0, tonumber(rotation.y) or 0,
                    tonumber(rotation.z) or 0, tonumber(rotation.w) or 1
                local sx, sy, sz = tonumber(scale.x) or 1, tonumber(scale.y) or 1, tonumber(scale.z) or 1
                local rotation_norm = qx*qx + qy*qy + qz*qz + qw*qw
                local valid = editor.finite_number(qx) and editor.finite_number(qy) and editor.finite_number(qz) and editor.finite_number(qw) and
                    editor.finite_number(sx) and editor.finite_number(sy) and editor.finite_number(sz) and
                    editor.finite_number(rotation_norm) and rotation_norm > 1e-12
                if not valid then
                    shroudforge.log.warn("World Editor prop capture stopped: invalid transform data for item " .. tostring(id))
                    return nil
                end
                editor.live_prop_cache[live_prop.handle] = {
                    itemId = id, position = {position.x, position.y, position.z},
                    qx = qx, qy = qy, qz = qz, qw = qw, sx = sx, sy = sy, sz = sz,
                }
                props[#props + 1] = {
                    itemId = id, entityHandle = live_prop.handle,
                    templateUuidHighHex = live_prop.templateUuidHighHex,
                    templateUuidLowHex = live_prop.templateUuidLowHex,
                    expected_transform = live_prop,
                    x = position.x - anchor[1], y = position.y - anchor[2], z = position.z - anchor[3],
                    qx = qx, qy = qy, qz = qz, qw = qw,
                    sx = sx, sy = sy, sz = sz,
                }
            end
        end
        if #props > maximum then
            return nil, string.format("capture produced %d props, above the configured maximum of %d", #props, maximum)
        end
        return props
    end

    function editor.request_prop_capture(region, callback)
        if editor.pending_prop_capture then
            shroudforge.log.debug("World Editor is already waiting for a live prop query")
            return
        end
        shroudforge.log.debug("World Editor capture: resolving recipes and starting bounded prop scan")
        local props, reason = editor.capture_region_props(region)
        if props then callback(props); return end
        if editor.query_is_pending(reason) then
            editor.pending_prop_capture = {region = region, callback = callback, elapsed = 0}
            shroudforge.log.debug("World Editor capture: prop scan pending, continuing on subsequent updates, native watchdog checks scan progress")
            return
        end
        editor.set_editor_message(editor.editor_stage == "capturing" and "ready" or nil,
            "Could not read props in the selected area. Check the runtime status and try again.")
        shroudforge.log.warn("World Editor could not capture props: " .. tostring(reason or "live prop query unavailable"))
        editor.finish_process("error", "Props could not be read: " .. tostring(reason or "live prop query unavailable"), "warn")
    end

    function editor.cancel_pending_queries(message, stage, save_phase)
        local pending = editor.pending_world_action or editor.pending_cursor_action or editor.pending_prop_capture
        local operation = editor.process_progress.operation
        local capture = operation == "Capture and save blueprint" or operation == "Capture blueprint data" or
            operation == "Capture props only"
        -- Drop every continuation before publishing the terminal state. None of
        -- these read-side requests may finish a capture after cancellation.
        editor.pending_world_action, editor.pending_cursor_action, editor.pending_prop_capture = nil, nil, nil
        if editor.save_progress.state == "saving" then
            editor.save_step(editor.save_progress.completed, save_phase or "Capture cancelled", "error")
        end
        if message then editor.set_editor_message(stage, message) end
        if editor.process_progress.state == "running" and (pending or capture) then
            editor.finish_process("error", message or "The pending world query was cancelled.", "warn")
        end
    end

    function editor.update_pending_queries(delta)
        if (editor.pending_world_action or editor.pending_cursor_action or editor.pending_prop_capture) and editor.current_session() == 0 then
            editor.cancel_pending_queries("The world session became unavailable. Capture cancelled. Mark the selection again after loading.", "ready")
            return
        end
        if editor.pending_world_action then
            local pending = editor.pending_world_action
            pending.elapsed = pending.elapsed + math.max(0, tonumber(delta) or 0)
            local session = editor.current_session()
            if session ~= pending.session or pending.elapsed >= editor.world_wait_timeout then
                local reason = session ~= pending.session and "World session changed or became unavailable" or
                    ("Native operation " .. pending.operation .. " remained unavailable within the retry window")
                editor.cancel_pending_queries(reason .. ". Action cancelled before its next world change.", "ready")
                shroudforge.log.warn("World Editor cancelled " .. pending.operation .. ": " .. reason)
            elseif pending.guarded_read or runtime.has(pending.operation) then
                editor.pending_world_action = nil
                pending.action(pending.elapsed)
            else
                local status = runtime.status(pending.operation)
                local reason = status and status.reason
                local mutation_unavailable = editor.native_mutation_unavailable_reason(pending.operation, status)
                if mutation_unavailable then
                    editor.cancel_pending_queries(mutation_unavailable, "selected")
                    shroudforge.log.warn("World Editor stopped " .. pending.operation .. ": " .. mutation_unavailable)
                elseif not (status and status.available) and not editor.world_feature_pending(reason) then
                    editor.cancel_pending_queries("Action cancelled: " .. tostring(reason or "World API unavailable"),
                        editor.editor_stage == "capturing" and "ready" or nil)
                    shroudforge.log.warn("World Editor world query stopped: " .. tostring(reason or "world operation unavailable"))
                end
            end
        end
        if editor.pending_cursor_action then
            local point, reason = editor.cursor_point()
            if point then
                local action = editor.pending_cursor_action
                editor.pending_cursor_action = nil
                action(point)
            elseif not editor.query_is_pending(reason) then
                editor.pending_cursor_action = nil
                editor.editor_hint = "Could not read the cursor: " .. tostring(reason or "cursor position unavailable")
                editor.publish_editor_state()
                shroudforge.log.warn("World Editor cursor query stopped: " .. tostring(reason))
                editor.finish_process("error", "Cursor lookup stopped: " .. tostring(reason), "warn")
            end
        end
        if editor.pending_prop_capture then
            local pending = editor.pending_prop_capture
            pending.elapsed = pending.elapsed + math.max(0, tonumber(delta) or 0)
            -- A single busy Scene sample cannot invalidate an in-flight scan.
            -- capture_region_props returns the current reason and the native
            -- scan watchdog still bounds missing progress.
            local props, reason = editor.capture_region_props(pending.region)
            if props then
                editor.pending_prop_capture = nil
                shroudforge.log.info("World Editor capture: prop scan completed, " .. tostring(#props) .. " props")
                pending.callback(props)
            elseif editor.world_feature_pending(reason) and pending.elapsed >= editor.world_wait_timeout and
                   not (type(reason) == "string" and reason:find("still scanning", 1, true)) then
                editor.cancel_pending_queries("The prop API did not become available within the retry window. Action cancelled.",
                    editor.editor_stage == "capturing" and "ready" or nil)
            elseif not editor.query_is_pending(reason) then
                editor.pending_prop_capture = nil
                if editor.editor_stage == "capturing" then
                    editor.editor_stage = "ready"
                    editor.editor_hint = "Prop capture failed: " .. tostring(reason or "live prop query unavailable")
                    editor.publish_editor_state()
                end
                shroudforge.log.warn("World Editor prop query stopped: " .. tostring(reason or "live prop query unavailable"))
                editor.finish_process("error", "Prop scanning could not finish: " .. tostring(reason or "live prop query unavailable"), "warn")
            end
        end
    end

    function editor.finish_capture(region, cells, props, save_and_select, anchor, extent, voxel_offset)
        local maximum = editor.maximum_copyable_props()
        if #(props or {}) > maximum then
            editor.set_editor_message("ready", string.format("Capture stopped. It found %d props, above your limit of %d. Raise the limit in mod settings or select a smaller area.", #props, maximum))
            shroudforge.log.warn(string.format("World Editor refused capture: %d props exceed the configured maximum of %d", #props, maximum))
            editor.finish_process("error", string.format("Capture found %d props, above the configured limit of %d.", #props, maximum), "warn")
            return
        end
        local rotation_axis = region and region.rotationAxis or editor.setting("rotationAxis")
        if rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z" then rotation_axis = "y" end
        local blueprint_region = {rotationAxis = rotation_axis}
        if region then blueprint_region.sx, blueprint_region.sy, blueprint_region.sz = region.sx, region.sy, region.sz end
        editor.clipboard = {region = blueprint_region, cells = cells, props = props,
            hasVoxels = cells ~= nil, extent = extent, voxelOffset = voxel_offset,
            cellSize = cells and region and (region.cellSize or {0.5, 0.5, 0.5}) or nil}
        if cells then
            editor.clipboard.coverage = {}
            for index, value in ipairs(cells) do editor.clipboard.coverage[index] = value == 0 and 1 or 2 end
        end
        if editor.process_progress.operation == "Capture and save blueprint" then
            editor.process_step("Encoding blueprint", 3, 5,
                string.format("Captured %d terrain cells and %d props, preparing the blueprint file.", #(cells or {}), #props))
        else
            editor.finish_process("complete", string.format("Capture complete: %d terrain cells and %d props are ready.", #(cells or {}), #props))
        end
        editor.placement_preview = nil
        local occupied = 0
        for _, value in ipairs(cells or {}) do if value ~= 0 then occupied = occupied + 1 end end
        if cells then
            shroudforge.log.info(string.format("World Editor captured %d verified voxel cells (%d occupied) and %d resolved props from %d,%d,%d",
                #cells, occupied, #props, region.x, region.y, region.z))
        else
            shroudforge.log.info(string.format("World Editor captured a props-only region (%d props, extent %.3f, %.3f, %.3f)",
                #props, extent[1], extent[2], extent[3]))
        end
        if save_and_select then
            editor.save_capture()
        elseif editor.process_progress.operation == "Capture blueprint data" or
           editor.process_progress.operation == "Capture props only" then
            editor.process_step("Capture complete", 1, 1,
                string.format("Captured %d props for the active blueprint.", #props))
            editor.finish_process("complete", "The captured blueprint data is ready in the editor.")
        end
    end

    function editor.copy_voxels(save_and_select, retry_elapsed)
        if not save_and_select and editor.process_progress.state ~= "running" then
            editor.begin_process("Capture blueprint data", "Reading terrain", 2,
                "Reading terrain data for the selected region.")
        end
        if not editor.export_ready_for_capture(save_and_select) then return end
        local retry = function(elapsed) editor.copy_voxels(save_and_select, elapsed) end
        if not editor.require_world_feature("runtime.world.voxel.read", retry, retry_elapsed) then
            if not editor.pending_world_action then
                editor.cancel_pending_queries("Terrain capture stopped: " ..
                    tostring(editor.unavailable_world_reason("runtime.world.voxel.read") or "the terrain reader is unavailable"), "ready")
            end
            return
        end
        local region, reason = editor.voxel_region(true)
        if not region then
            if editor.query_is_pending(reason) then
                editor.defer_world_action("runtime.world.voxel.read", retry, retry_elapsed, true)
                return
            end
            editor.set_editor_message("ready", "Capture stopped. " .. tostring(reason) .. " Choose a smaller or valid selection, then try again.")
            shroudforge.log.warn("World Editor: " .. reason)
            editor.finish_process("error", "Terrain capture stopped: " .. tostring(reason), "warn")
            return
        end
        local capture_session = editor.current_session()
        local cells, read_reason = runtime.world.voxel.read(region.x, region.y, region.z,
            region.sx, region.sy, region.sz)
        if capture_session == 0 or editor.current_session() ~= capture_session then
            editor.cancel_pending_queries("The world changed while terrain was being read. Capture cancelled.", "ready")
            return
        end
        if not cells then
            if editor.query_is_pending(read_reason) then
                editor.defer_world_action("runtime.world.voxel.read", retry, retry_elapsed, true)
                return
            end
            editor.set_editor_message("ready", "Capture stopped because terrain data could not be read. Check runtime status, then try {{keybind:captureAndSave}} again.")
            runtime.report_effect("waiting", read_reason or "voxel read failed")
            shroudforge.log.warn("World Editor voxel copy failed: " .. tostring(read_reason))
            editor.finish_process("error", "Terrain could not be read: " .. tostring(read_reason), "warn")
            return
        end
        if save_and_select then
            editor.process_step("Scanning props", 2, 5,
                string.format("Read %d terrain cells. Scanning props in the selected area.", #cells))
        end
        local grid = editor.get_voxel_grid_spec()
        if not grid then
            editor.set_editor_message("ready", "Capture stopped because voxel grid information is unavailable. Check runtime status, then try again.")
            shroudforge.log.warn("World Editor could not get voxel grid metadata")
            editor.finish_process("error", "Voxel grid information is unavailable.", "warn")
            return
        end
        local anchor, extent
        if editor.selection_a and editor.selection_b then
            anchor = {math.min(editor.selection_a.x, editor.selection_b.x), math.min(editor.selection_a.y, editor.selection_b.y), math.min(editor.selection_a.z, editor.selection_b.z)}
            local far = {math.max(editor.selection_a.x, editor.selection_b.x), math.max(editor.selection_a.y, editor.selection_b.y), math.max(editor.selection_a.z, editor.selection_b.z)}
            extent = {far[1] - anchor[1], far[2] - anchor[2], far[3] - anchor[3]}
        else
            anchor = {editor.grid_to_world(region.x, 1, grid), editor.grid_to_world(region.y, 2, grid), editor.grid_to_world(region.z, 3, grid)}
            extent = {region.sx * grid.cellSize[1], region.sy * grid.cellSize[2], region.sz * grid.cellSize[3]}
        end
        local bounds = {editor.grid_to_world(region.x, 1, grid), editor.grid_to_world(region.y, 2, grid), editor.grid_to_world(region.z, 3, grid),
            editor.grid_to_world(region.x + region.sx, 1, grid), editor.grid_to_world(region.y + region.sy, 2, grid), editor.grid_to_world(region.z + region.sz, 3, grid)}
        local props_region = {queryBounds = bounds, anchor = anchor}
        local voxel_offset = {bounds[1] - anchor[1], bounds[2] - anchor[2], bounds[3] - anchor[3]}
        region.rotationAxis, region.cellSize = editor.setting("rotationAxis"), grid.cellSize
        editor.request_prop_capture(props_region, function(props)
            editor.finish_capture(region, cells, props, save_and_select, anchor, extent, voxel_offset)
        end)
    end

    function editor.capture_and_save()
        if editor.save_progress.state == "saving" or editor.pending_prop_capture then
            shroudforge.log.warn("World Editor: wait for the current blueprint save to finish")
            return
        end
        if not editor.selection_a then
            editor.clipboard, editor.active_blueprint_name, editor.placement_preview = nil, nil, nil
            editor.editor_stage = "need_a"
            editor.editor_hint = "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press {{keybind:markCursorNext}}."
            editor.publish_editor_state()
            shroudforge.log.debug(editor.format_keybind_hint("World Editor help: select the building hammer, choose a single voxel, aim at the first corner, then press {{keybind:markCursorNext}}."))
            return
        end
        if not editor.selection_b then
            editor.editor_stage = "need_b"
            editor.editor_hint = "Corner A is marked. Aim at the opposite corner and press {{keybind:markCursorNext}} again."
            editor.publish_editor_state()
            shroudforge.log.debug(editor.format_keybind_hint("World Editor help: aim at the opposite corner and press {{keybind:markCursorNext}} before capturing with {{keybind:captureAndSave}}."))
            return
        end
        if not editor.editor_is_server and editor.execution_backend() == "p2p" then
            editor.capture_remote()
            return
        end
        editor.editor_stage = "capturing"
        editor.editor_hint = "Capturing props and voxels, creating the blueprint file, and adding it to the library…"
        editor.publish_editor_state()
        editor.begin_process("Capture and save blueprint", "Reading terrain", 5,
            "Reading the selected terrain region.")
        editor.save_step(0, "Capturing blueprint")
        editor.copy_voxels(true)
        if editor.editor_stage == "capturing" and not editor.pending_prop_capture and not editor.pending_world_action then
            editor.cancel_pending_queries("Blueprint was not saved. Check the runtime status and export folder, then try {{keybind:captureAndSave}} again.", "ready")
        end
    end

    function editor.copy_props_only(save_and_select)
        if not editor.export_ready_for_capture(save_and_select) then return end
        if not editor.selection_a or not editor.selection_b then
            shroudforge.log.warn("World Editor props-only capture requires cursor selection corners A and B")
            return
        end
        local anchor = {math.min(editor.selection_a.x, editor.selection_b.x), math.min(editor.selection_a.y, editor.selection_b.y), math.min(editor.selection_a.z, editor.selection_b.z)}
        local far = {math.max(editor.selection_a.x, editor.selection_b.x), math.max(editor.selection_a.y, editor.selection_b.y), math.max(editor.selection_a.z, editor.selection_b.z)}
        local extent = {far[1] - anchor[1], far[2] - anchor[2], far[3] - anchor[3]}
        if extent[1] <= 0 or extent[2] <= 0 or extent[3] <= 0 then
            shroudforge.log.warn("World Editor props-only capture needs a non-zero selection extent on every axis")
            return
        end
        if not save_and_select then
            editor.begin_process("Capture props only", "Scanning props", 1,
                "Scanning props inside the marked selection.")
        end
        local region = {anchor = anchor, queryBounds = {anchor[1], anchor[2], anchor[3], far[1], far[2], far[3]},
            rotationAxis = editor.setting("rotationAxis")}
        editor.request_prop_capture(region, function(props)
            editor.finish_capture(region, nil, props, save_and_select, anchor, extent, nil)
        end)
    end

    function editor.paste_voxels(use_current_cursor, cursor_override, captured_props, target_override, retry_elapsed, preparation)
        if not editor.clipboard then
            editor.set_editor_message("need_a", "No blueprint is active. Press {{keybind:markCursorNext}} to create one or select a saved blueprint.")
            shroudforge.log.warn("World Editor: capture or load a blueprint before pasting")
            return
        end
        if editor.pending_world_action then
            editor.set_editor_message(nil, "Waiting for the live world to become ready. The requested action will continue automatically.")
            return
        end
        if editor.remote_release_pending then
            editor.set_editor_message("recovery", "The server has not confirmed release of the previous operation. Press {{keybind:resetEditor}} to resend the release request; {{keybind:pasteAtCursor}} stays paused until confirmation.")
            return
        end
        if editor.remote_recovery_required then
            editor.set_editor_message("recovery", "The server has an unfinished World Editor operation. Press {{keybind:resetEditor}} to abandon it and release the server before starting another placement.")
            return
        end
        if editor.remote_request_pending or editor.remote_active_request or (editor.p2p_bridge and editor.p2p_bridge.pending()) then
            editor.set_editor_message("building", "The World Editor is waiting for the server response. No second request was sent.")
            return
        end
        local maximum = editor.maximum_copyable_props()
        local prop_count = #(editor.clipboard.props or {})
        if prop_count > maximum then
            editor.set_editor_message("selected", string.format("Placement stopped. This blueprint has %d props, above your limit of %d. Raise the limit in mod settings or choose a smaller blueprint.", prop_count, maximum))
            shroudforge.log.warn(string.format("World Editor: paste refused because the blueprint has %d props and the configured maximum is %d", prop_count, maximum))
            return
        end
        if editor.undo_state and editor.undo_state.recovery_required then
            editor.set_editor_message("recovery", "Placement is paused. Press {{keybind:undoVoxels}} to finish undoing the previous placement first.")
            shroudforge.log.warn(editor.format_keybind_hint("World Editor: restore the incomplete previous placement with {{keybind:undoVoxels}} before starting another paste"))
            return
        end
        local backend = editor.execution_backend()
        if not editor.require_execution_backend(backend, "{{keybind:pasteAtCursor}} placement") then return end
        if backend == "p2p" then
            if editor.paste_remote then editor.paste_remote(use_current_cursor, cursor_override) end
            return
        end
        preparation = preparation or {features = {}, session = editor.current_session()}
        if editor.process_progress.state ~= "running" or editor.process_progress.operation ~= "Place blueprint" then
            editor.begin_process("Place blueprint", "Checking world APIs", 5,
                "Checking the native APIs before preparing the placement. No world changes have been sent.")
        end
        local blueprint = editor.clipboard
        local function retry(elapsed)
            if editor.clipboard ~= blueprint then
                editor.cancel_pending_queries("Placement cancelled because the active blueprint changed.", "selected")
                editor.finish_process("error", "The active blueprint changed while placement was waiting.", "warn")
                return
            end
            editor.paste_voxels(use_current_cursor, cursor_override, captured_props,
                preparation.context or target_override, elapsed, preparation)
        end
        local function require_placement_feature(operation)
            -- A successful readiness sample does not reserve the Scene gate.
            -- Retain this preflight result across later read-only waits rather
            -- than requiring all independent samples to succeed in one frame.
            -- Every actual native operation still acquires and validates its
            -- own original gates and the current session before dispatch.
            if preparation.features[operation] then return true end
            if editor.require_world_feature(operation, retry, retry_elapsed) then
                preparation.features[operation] = true
                return true
            end
            if editor.pending_world_action then
                editor.process_step("Waiting for world APIs", 0, 5,
                    "Waiting for " .. operation .. ". Placement will continue automatically before any world change.", "debug")
            else
                editor.finish_process("error", "Required native placement feature is unavailable: " .. operation, "warn")
            end
            return false
        end
        if editor.clipboard.hasVoxels and not require_placement_feature("runtime.world.voxel.write") then return end
        local context, reason
        if target_override then context = target_override
        elseif editor.placement_preview and editor.placement_preview.blueprint == editor.clipboard and
           editor.placement_preview.turns == editor.rotation_turns() then
            context = editor.placement_preview
        else
            local anchor
            anchor, reason = editor.target_anchor(use_current_cursor, cursor_override)
            if not anchor and use_current_cursor and editor.query_is_pending(reason) then
                editor.request_cursor_action(function(point) editor.paste_voxels(true, point) end)
                return
            end
            if not anchor then
                editor.set_editor_message("selected", "Placement stopped. " .. tostring(reason) .. " Aim at a valid target and try again.")
                shroudforge.log.warn("World Editor: " .. tostring(reason))
                editor.finish_process("error", "The placement target is unavailable: " .. tostring(reason), "warn")
                return
            end
            local turns = editor.rotation_turns()
            local rotated = editor.rotated_blueprint(editor.clipboard, turns)
            local axis = editor.clipboard.region.rotationAxis or editor.setting("rotationAxis")
            if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
            local target
            target, reason = editor.voxel_target_region(rotated, anchor)
            if editor.clipboard.hasVoxels and not target then
                if editor.query_is_pending(reason) then
                    editor.defer_world_action("runtime.world.voxel.read", retry, retry_elapsed, true)
                    return
                end
                editor.set_editor_message("selected", "Placement stopped. " .. tostring(reason) .. " Aim at a grid-aligned target and try again.")
                shroudforge.log.warn("World Editor: " .. tostring(reason))
                editor.finish_process("error", "The placement target is not valid: " .. tostring(reason), "warn")
                return
            end
            context = {anchor = anchor, plan = rotated, target = target,
                bounds = editor.plan_world_bounds(rotated, anchor), turns = turns, axis = axis, blueprint = editor.clipboard}
        end
        local anchor, target = context.anchor, context.target
        local rotated = context.plan
        if not anchor or not rotated then
            editor.set_editor_message("selected", "Placement stopped because its plan is incomplete. Aim again, then press {{keybind:pasteAtCursor}}.")
            shroudforge.log.warn("World Editor: placement plan is incomplete")
            editor.finish_process("error", "The placement plan is incomplete.", "warn")
            return
        end
        local turns = editor.rotation_turns()
        local axis = editor.clipboard.region.rotationAxis or editor.setting("rotationAxis")
        if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
        if context.blueprint ~= editor.clipboard or context.turns ~= turns or context.axis ~= axis or
           context.plan.hasVoxels ~= editor.clipboard.hasVoxels then
            editor.set_editor_message("selected", "Placement settings changed while preparing. Aim again and press {{keybind:pasteAtCursor}}.")
            shroudforge.log.warn("World Editor: placement settings changed while the prop query was pending. Prepare the preview again.")
            editor.finish_process("error", "Placement settings changed during preparation.", "warn")
            return
        end
        preparation.context = context
        if not editor.clipboard.hasVoxels and #rotated.props == 0 and editor.setting("targetPropMode") ~= "replace" then
            editor.set_editor_message("selected", "This blueprint has no props to place. Enable target prop replacement or choose another blueprint.")
            shroudforge.log.warn("World Editor: empty props-only blueprint has nothing to paste unless target props are set to replace")
            editor.finish_process("error", "The blueprint has no props to place.", "warn")
            return
        end
        editor.process_step("Checking target and saving undo data", 0, 5,
            string.format("Preparing %s with %d props.", editor.active_blueprint_name or "the active blueprint", #rotated.props))
        local recipes = (#rotated.props > 0 or editor.setting("targetPropMode") == "replace") and editor.resolve_placeable_items() or {}
        local scale_required = false
        for _, prop in ipairs(rotated.props or {}) do
            if not editor.prop_scale_is_unit({prop.sx, prop.sy, prop.sz}) then scale_required = true end
        end
        if #rotated.props > 0 and (not require_placement_feature("runtime.world.entity.spawn") or
           (scale_required and not require_placement_feature("runtime.world.entity.set_scale"))) then
            return
        end
        for _, prop in ipairs(rotated.props or {}) do
            local maximum_scale = 3.402823466e38
            if not editor.finite_number(prop.sx) or not editor.finite_number(prop.sy) or not editor.finite_number(prop.sz) or
               math.abs(prop.sx) > maximum_scale or math.abs(prop.sy) > maximum_scale or math.abs(prop.sz) > maximum_scale then
                editor.set_editor_message("selected", "Placement stopped because a prop has invalid scale data. Check the blueprint file.")
                shroudforge.log.warn("World Editor: blueprint prop scale is outside the native transform range")
                editor.finish_process("error", "A blueprint prop has an invalid scale.", "warn")
                return
            end
            if not recipes or not recipes[prop.itemId] then
                editor.set_editor_message("selected", "Placement stopped because a prop has no current placement recipe. Check the blueprint or game assets.")
                shroudforge.log.warn("World Editor: blueprint prop has no current native ItemInfo placement recipe")
                editor.finish_process("error", "A blueprint prop has no valid placement recipe.", "warn")
                return
            end
        end
        local previous_cells, paste_cells
        if editor.clipboard.hasVoxels then
            local read_reason
            previous_cells, read_reason = runtime.world.voxel.read(target.x, target.y, target.z, target.sx, target.sy, target.sz)
            if not previous_cells then
                if editor.query_is_pending(read_reason) then
                    editor.defer_world_action("runtime.world.voxel.read", retry, retry_elapsed, true)
                    editor.process_step("Waiting for target terrain", 0, 5,
                        "Waiting for the target terrain snapshot before making any world change.", "debug")
                    return
                end
                editor.set_editor_message("selected", "Placement stopped because the target terrain could not be backed up. Aim at another target and retry.")
                shroudforge.log.warn("World Editor could not snapshot the target before paste: " .. tostring(read_reason))
                editor.finish_process("error", "Could not save the target terrain for undo: " .. tostring(read_reason), "warn")
                return
            end
            paste_cells = {}
            local additive = editor.setting("pasteVoxelMode") == "add"
            for index, value in ipairs(rotated.cells) do
                local state = rotated.coverage[index]
                if state == 0 or (additive and state == 1) then
                    paste_cells[index] = previous_cells[index]
                elseif state == 1 then
                    paste_cells[index] = 0
                else
                    paste_cells[index] = value
                end
            end
        end
        editor.process_step("Undo data saved", 1, 5,
            editor.clipboard.hasVoxels and "Saved the target terrain snapshot for a safe undo." or "Prepared prop state for undo.")
        local removed_props = {}
        if editor.setting("targetPropMode") == "replace" then
            if not require_placement_feature("runtime.world.entity.destroy") or not require_placement_feature("runtime.world.entity.spawn") then
                return
            end
            if not captured_props then
                local query_region = {queryBounds = context.bounds, anchor = anchor}
                editor.process_step("Checking target props", 2, 5, "Scanning props that will be replaced.")
                editor.request_prop_capture(query_region, function(props)
                    editor.paste_voxels(false, nil, props, context, retry_elapsed, preparation)
                end)
                return
            end
            local props = captured_props
            editor.process_step("Removing target props", 2, 5,
                string.format("Verified %d target props for replacement.", #props))
            for _, prop in ipairs(props) do
                local recipe = recipes and recipes[prop.itemId]
                if not recipe then
                    shroudforge.log.warn("World Editor cannot replace a target prop without a current ItemInfo recipe")
                    editor.finish_process("error", "A target prop has no valid replacement recipe.", "warn")
                    return
                end
                local live_prop = editor.read_prop(prop.entityHandle)
                if not live_prop or live_prop.itemId ~= prop.itemId or
                   live_prop.templateUuidHighHex ~= prop.templateUuidHighHex or
                   live_prop.templateUuidLowHex ~= prop.templateUuidLowHex then
                    shroudforge.log.warn("World Editor refused replace-props paste because the captured entity handle is stale")
                    editor.finish_process("error", "A target prop changed while the replacement plan was being prepared.", "warn")
                    return
                end
                local live_transform = live_prop.transform
                local live_position = live_transform and editor.world_position(live_transform.position)
                local live_rotation = live_transform and (live_transform.orientation or live_transform.rotation)
                local live_scale = live_transform and live_transform.scale
                if not live_position or not live_rotation or not live_scale then
                    shroudforge.log.warn("World Editor cannot snapshot the exact target prop transform for undo")
                    editor.finish_process("error", "A target prop could not be safely recorded for undo.", "warn")
                    return
                end
                local sx, sy, sz = tonumber(live_scale.x), tonumber(live_scale.y), tonumber(live_scale.z)
                if not editor.finite_number(sx) or not editor.finite_number(sy) or not editor.finite_number(sz) then
                    shroudforge.log.warn("World Editor cannot replace a target prop with invalid scale data")
                    editor.finish_process("error", "A target prop has invalid scale data.", "warn")
                    return
                end
                removed_props[#removed_props + 1] = {
                    recipe = recipe, position = {live_position.x, live_position.y, live_position.z},
                    rotation = {live_rotation.x, live_rotation.y, live_rotation.z, live_rotation.w}, entityHandle = prop.entityHandle,
                    templateUuidHighHex = prop.templateUuidHighHex,
                    templateUuidLowHex = prop.templateUuidLowHex,
                    scale = {sx, sy, sz},
                    expected_transform = live_prop,
                }
            end
        end
        for _, prop in ipairs(removed_props) do
            if not editor.prop_scale_is_unit(prop.scale) and
               not require_placement_feature("runtime.world.entity.set_scale") then return end
        end
        local session = runtime.world.session_id and runtime.world.session_id() or 0
        if session == 0 or session ~= preparation.session then
            editor.set_editor_message("recovery", "World session is unavailable. No placement was sent.")
            editor.finish_process("error", "The active world session is unavailable. No placement was sent.", "warn")
            return
        end
        editor.undo_state = {
            session = session,
            region = target, cells = previous_cells, expected_cells = previous_cells,
            entities = {}, entity_index = 0, removed_props = {}, removed_prop_index = 0,
            voxel_written = false, recovery_required = true, hasVoxels = editor.clipboard.hasVoxels,
        }
        editor.editor_stage = "placing"
        editor.set_editor_message("placing", "Placing " .. (editor.active_blueprint_name or "the blueprint") .. ". Keep the target clear until placement finishes.")
        for _, prop in ipairs(removed_props) do
            -- Retain the original before attempting deletion. A native call can
            -- succeed while its readback is unavailable, so that outcome must
            -- remain recoverable without guessing whether to spawn a duplicate.
            prop.removal_pending = true
            editor.undo_state.removed_props[#editor.undo_state.removed_props + 1] = prop
            editor.undo_state.removed_prop_index = #editor.undo_state.removed_props
            local removed, remove_reason = runtime.world.entity.destroy(prop.entityHandle)
            local still_present, read_reason = editor.read_prop(prop.entityHandle, editor.undo_state.session)
            local handle_gone = still_present == nil and read_reason == "entity-absent"
            if handle_gone then
                prop.removal_pending = nil
            end
            if not handle_gone then
                runtime.report_effect("write-failed", read_reason or remove_reason or "target prop removal was not verified")
                shroudforge.log.warn("World Editor stopped before voxel paste because a target prop could not be safely removed")
                editor.rollback_partial_paste()
                return
            end
        end
        editor.process_step("Target props removed", 2, 5,
            string.format("Removed %d verified target props.", #editor.undo_state.removed_props))
        if editor.clipboard.hasVoxels then
            -- A native write can partially modify the grid even when it reports
            -- failure. Mark the snapshot as applied before calling it so automatic
            -- rollback always attempts to restore the original region.
            editor.undo_state.voxel_written = true
            editor.undo_state.expected_cells = paste_cells
            local ok, write_reason = runtime.world.voxel.write(target.x, target.y, target.z,
                target.sx, target.sy, target.sz, paste_cells)
            if not ok then
                local partial_cells = runtime.world.voxel.read(target.x, target.y, target.z, target.sx, target.sy, target.sz)
                if partial_cells and not editor.same_cells(partial_cells, previous_cells) then
                    editor.undo_state.expected_cells = partial_cells
                    editor.undo_state.voxel_written = true
                end
                runtime.report_effect("write-failed", write_reason or "voxel write failed")
                shroudforge.log.error("World Editor voxel paste failed: " .. tostring(write_reason))
                editor.rollback_partial_paste()
                return
            end
            editor.undo_state.expected_cells = paste_cells
            editor.undo_state.voxel_written = true
            editor.process_step("Terrain written", 3, 5,
                string.format("Wrote %d terrain cells and confirmed the native write.", #paste_cells))
        else
            editor.process_step("Terrain skipped", 3, 5, "This props-only blueprint has no terrain channel.")
        end
        editor.process_step("Placing props", 4, 5, string.format("Placing %d blueprint props.", #rotated.props))
        local spawned = {}
        for _, prop in ipairs(rotated.props or {}) do
            local recipe = recipes[prop.itemId]
            local position = {anchor[1] + prop.x, anchor[2] + prop.y, anchor[3] + prop.z}
            local rotation = {prop.qx, prop.qy, prop.qz, prop.qw}
            local entity_handle, spawn_reason = runtime.world.entity.spawn(prop.templateUuidHighHex or recipe.uuidHigh,
                prop.templateUuidLowHex or recipe.uuidLow,
                position, rotation, recipe.id, 0)
            if not entity_handle then
                runtime.report_effect("write-failed", spawn_reason or "entity spawn was not verified")
                shroudforge.log.error("World Editor stopped after partial paste. Entity spawn failed for item " ..
                    tostring(prop.itemId) .. ": " .. tostring(spawn_reason))
                editor.undo_state.entities, editor.undo_state.entity_index = spawned, #spawned
                editor.rollback_partial_paste()
                return
            end
            local scale = {prop.sx, prop.sy, prop.sz}
            local tracked = {recipe = recipe, position = position, rotation = rotation, scale = scale,
                entityHandle = entity_handle, expected_transform = editor.intended_spawn_transform(position, rotation, scale,
                    recipe.id, prop.templateUuidHighHex or recipe.uuidHigh, prop.templateUuidLowHex or recipe.uuidLow)}
            spawned[#spawned + 1] = tracked
            editor.undo_state.entities, editor.undo_state.entity_index = spawned, #spawned
            local scaled, scale_reason = true, nil
            if not editor.prop_scale_is_unit(scale) then
                scaled, scale_reason = runtime.world.entity.set_scale(entity_handle, scale)
            end
            if not scaled then
                local current = editor.read_prop(entity_handle, editor.undo_state.session)
                if current then tracked.expected_transform = current end
                runtime.report_effect("write-failed", scale_reason or "spawned prop scale could not be set")
                shroudforge.log.error("World Editor stopped after spawn because native prop scale could not be set: " .. tostring(scale_reason))
                editor.rollback_partial_paste()
                return
            end
            local created = editor.read_prop(entity_handle, editor.undo_state.session)
            if not created or created.itemId ~= recipe.id or not editor.prop_scale_matches(created, scale) then
                tracked.expected_transform = created or tracked.expected_transform
                local detail = "spawn returned an entity handle that did not resolve to the requested live prop"
                runtime.report_effect("write-failed", detail)
                shroudforge.log.error("World Editor stopped after spawn because it could not track the new prop: " .. tostring(detail))
                editor.rollback_partial_paste()
                return
            end
            tracked.expected_transform = created
            tracked.entityId = created.entityId
        end
        editor.undo_state.entities, editor.undo_state.entity_index = spawned, #spawned
        editor.undo_state.recovery_required = false
        editor.placement_preview = nil
        editor.set_editor_message("selected", "Blueprint placed successfully. Press {{keybind:undoVoxels}} to undo it or {{keybind:pasteAtCursor}} to place it again.")
        runtime.report_effect("write-confirmed", string.format("Wrote and verified %d voxel cells and %d new props in live ECS. Save persistence is not verified.",
            #(editor.clipboard.cells or {}), #spawned))
        shroudforge.log.info(string.format("World Editor placed blueprint with %d voxel cells and %d props at %.3f,%.3f,%.3f",
            #(editor.clipboard.cells or {}), #spawned, anchor[1], anchor[2], anchor[3]))
        editor.process_step("Placement verified", 5, 5,
            string.format("Verified %d terrain cells and %d props in the live world. Save persistence is not verified.",
                #(editor.clipboard.cells or {}), #spawned))
        editor.finish_process("complete", "Placement is confirmed in the live world. Save persistence is not verified.")
    end

    function editor.list_props()
        local region, reason = editor.voxel_region(true)
        if not region then shroudforge.log.warn("World Editor: " .. tostring(reason)); return end
        editor.request_prop_capture(region, function(props)
        shroudforge.log.debug(string.format("World Editor found %d placeable props with known ItemInfo recipes in the selected region", #props))
            for index = 1, math.min(#props, 100) do
                local prop = props[index]
                local recipe = editor.resolve_placeable_items()[prop.itemId]
            shroudforge.log.trace(string.format("World Editor prop %d: handle=%d %s item=%d at local %.3f, %.3f, %.3f",
                    index, prop.entityHandle, recipe.name, prop.itemId, prop.x, prop.y, prop.z))
            end
        if #props > 100 then shroudforge.log.debug("World Editor prop listing is truncated at 100 entries") end
        end)
    end

    function editor.direct_edit_allowed()
        local backend = editor.execution_backend()
        if backend == "unknown" then
            editor.require_execution_backend(backend, "direct entity operation")
            return false
        end
        if backend == "p2p" or editor.remote_request_pending or (editor.p2p_bridge and editor.p2p_bridge.pending()) then
            editor.set_editor_message("selected","Use blueprint paste and {{keybind:undoVoxels}} undo for server edits.")
            return false
        end
        return true
    end

    function editor.destroy_selected_prop()
        if not editor.direct_edit_allowed() then return end
        if not editor.feature("runtime.world.entity.query_props") or not editor.feature("runtime.world.entity.destroy") then return end
        local handle = tonumber(editor.setting("entityHandle"))
        if not handle or handle < 1 or handle % 1 ~= 0 then
            shroudforge.log.warn("World Editor: enter a live prop handle from the List props output")
            return
        end
        local selected = editor.live_prop_cache[handle]
        local item = selected and selected.itemId
        local position = selected and {x = selected.position[1], y = selected.position[2], z = selected.position[3]}
        local recipe = item and editor.resolve_placeable_items() and editor.resolve_placeable_items()[item]
        if not position or not recipe then
            shroudforge.log.warn("World Editor: the selected handle is stale or has no verified ItemInfo placement recipe")
            return
        end
        local live = editor.read_prop(handle)
        if not live or live.itemId ~= item then
            shroudforge.log.warn("World Editor cannot safely delete this selection: its handle is stale")
            return
        end
        local ok, reason = runtime.world.entity.destroy(handle)
        local current, read_reason = editor.read_prop(handle)
        if current or read_reason ~= "entity-absent" then
            local detail = read_reason or reason or "the selected live ECS handle remains after the destroy call"
            runtime.report_effect("write-failed", detail)
            shroudforge.log.error("World Editor could not verify removal of selected prop handle " .. tostring(handle) .. ": " .. tostring(detail))
            return
        end
        runtime.report_effect("write-confirmed", "The selected live ECS handle disappeared. Save persistence is not verified.")
        shroudforge.log.info("World Editor removed the selected prop at its live transform. Persistence is not verified.")
    end

    function editor.report_recipe_catalog()
        local recipes = editor.resolve_placeable_items()
        if not recipes then return end
        local count = 0
        for _ in pairs(recipes) do count = count + 1 end
        shroudforge.log.debug("World Editor recipe catalog is ready from this build's ItemInfo assets: " .. count .. " placeable item recipes")
    end

    function editor.entity_transform()
        return {tonumber(editor.setting("entityX")), tonumber(editor.setting("entityY")), tonumber(editor.setting("entityZ"))},
            {0, 0, 0, 1}
    end

    function editor.entity_bounds()
        return {
            tonumber(editor.setting("boundsMinX")), tonumber(editor.setting("boundsMinY")), tonumber(editor.setting("boundsMinZ")),
            tonumber(editor.setting("boundsMaxX")), tonumber(editor.setting("boundsMaxY")), tonumber(editor.setting("boundsMaxZ")),
        }
    end

    function editor.spawn_entity()
        if not editor.direct_edit_allowed() then return end
        if not editor.feature("runtime.world.entity.spawn") then return end
        local tracking = tonumber(editor.setting("trackingId"))
        if not tracking or tracking < 1 or tracking % 1 ~= 0 then
            shroudforge.log.warn("World Editor: enter a nonzero placement tracking ID")
            return
        end
        local position, rotation = editor.entity_transform()
        local entity_handle, reason = runtime.world.entity.spawn(editor.setting("templateUuidHigh"), editor.setting("templateUuidLow"),
            position, rotation, tracking, 0)
        if not entity_handle then
            runtime.report_effect("waiting", reason or "native spawn was not dispatched")
            shroudforge.log.warn("World Editor spawn failed: " .. tostring(reason))
            return
        end
        local prop = editor.read_prop(entity_handle)
        if not prop or prop.itemId ~= tracking then
            runtime.report_effect("write-failed", "spawn returned a handle that did not resolve to the requested live prop")
            shroudforge.log.error("World Editor could not resolve the spawned entity handle")
            return
        end
        runtime.report_effect("write-confirmed", "Spawn returned live ECS entity handle " .. tostring(entity_handle) .. ". Save persistence is not verified.")
        shroudforge.log.debug("World Editor verified spawned entity handle=" .. tostring(entity_handle))
    end

    function editor.placement_operation(destroy)
        if not editor.direct_edit_allowed() then return end
        local operation = destroy and "runtime.world.entity.destroy" or "runtime.world.entity.place"
        if not editor.feature(operation) or not editor.feature("runtime.world.entity.finish_building") then return end
        if not editor.feature("runtime.world.entity.query_props") then return end
        local tracking, feedback = tonumber(editor.setting("trackingId")), tonumber(editor.setting("feedbackId"))
        local bounds = editor.entity_bounds()
        if not tracking or tracking < 1 or tracking % 1 ~= 0 or
           (not destroy and (not feedback or feedback < 1 or feedback % 1 ~= 0)) then
            shroudforge.log.warn(destroy and "World Editor: tracking ID must be a nonzero integer" or
                "World Editor: tracking and material feedback IDs must be nonzero integers")
            return
        end
        for _, value in ipairs(bounds) do
            if not value or value ~= value or value == math.huge or value == -math.huge then
                shroudforge.log.warn("World Editor: all placement bounds must be finite numbers")
                return
            end
        end
        local position, rotation = editor.entity_transform()
        local before_count, before_handles = editor.count_prop_at(tracking, position)
        if before_count == nil then
            shroudforge.log.warn("World Editor cannot dispatch this operation without a readable pre-operation prop count: " .. tostring(before_handles))
            return
        end
        if destroy and before_count == 0 then
            runtime.report_effect("no-target", "No matching live prop exists at these coordinates")
            shroudforge.log.warn("World Editor found no matching live prop to destroy")
            return
        end
        if destroy and before_count ~= 1 then
            shroudforge.log.warn("World Editor requires one unambiguous prop handle at the configured transform")
            return
        end
        local ok, reason
        if destroy then
            ok, reason = runtime.world.entity.destroy(before_handles[1])
        else
            ok, reason = runtime.world.entity.place(position, rotation, bounds, tracking, feedback)
        end
        if destroy then
            local current, read_reason = editor.read_prop(before_handles[1])
            if not current and read_reason == "entity-absent" then
                runtime.report_effect("write-confirmed", "Matching live prop count decreased. Save persistence is not verified.")
                shroudforge.log.info("World Editor verified exact-handle removal in the live ECS")
            else
                runtime.report_effect("write-failed", read_reason or reason or "native destroy call did not reduce the matching live prop count")
                shroudforge.log.error("World Editor destroy did not remove a matching live prop")
            end
        else
            local after_count, after_reason = editor.count_prop_at(tracking, position)
            if after_count == nil then
                runtime.report_effect("no-change", "Native operation returned " .. tostring(ok) .. ". Live ECS result could not be read: " .. tostring(after_reason))
                shroudforge.log.warn("World Editor operation is unconfirmed because the live ECS could not be read: " .. tostring(after_reason))
                return
            end
            if after_count > before_count then
                runtime.report_effect("write-confirmed", "Matching live prop count increased after placement. Save persistence is not verified.")
                shroudforge.log.info("World Editor verified native placement in the live ECS")
            else
                runtime.report_effect("no-change", reason or "placement returned without a matching live prop appearing")
                shroudforge.log.warn("World Editor placement was dispatched but no matching live prop was observed")
            end
        end
    end

    function editor.configured_server_steam_id()
        local configured_fallback = ""
        if runtime.network and runtime.network.status then
            local ok, status = pcall(runtime.network.status)
            local discovered = ok and status and tostring(
                status.local_dedicated_server_steam_id or status.remote_dedicated_server_steam_id or "") or ""
            if #discovered >= 16 and #discovered <= 20 and discovered:match("^%d+$") and discovered ~= "0" then
                return discovered, true
            end
            configured_fallback = ok and status and tostring(status.configured_server_steam_id or "") or ""
        end
        local value = configured_fallback:gsub("%s", "")
        if value == "" or #value < 16 or #value > 20 or not value:match("^%d+$") or value == "0" then
            return nil
        end
        return value, false
    end

    function editor.configured_client_steam_ids()
        local ids, seen = {}, {}
        local configured = ""
        if runtime.network and runtime.network.status then
            local ok, status = pcall(runtime.network.status)
            if ok and status and type(status.allowed_client_steam_ids) == "table" then
                configured = table.concat(status.allowed_client_steam_ids, ",")
            end
        end
        for candidate in configured:gmatch("[^,;]+") do
            local peer = candidate:gsub("%s", "")
            if #peer >= 16 and #peer <= 20 and peer:match("^%d+$") and peer ~= "0" and not seen[peer] then
                seen[peer] = true
                ids[#ids + 1] = peer
            end
        end
        return ids
    end

    function editor.log_p2p_target_if_changed()
        if editor.editor_is_server then return end
        local peer, discovered = editor.configured_server_steam_id()
        local source = discovered and "automatic" or (peer and "network-setting" or "unavailable")
        local target_key = table.concat({peer or "", source}, "|")
        if target_key == editor.last_logged_p2p_target then return end
        editor.last_logged_p2p_target = target_key
        if peer then
            shroudforge.log.info("World Editor effective P2P target=" .. peer .. " source=" .. source)
        elseif editor.execution_backend() == "p2p" then
            shroudforge.log.warn("World Editor effective P2P target unavailable (automatic Network discovery has not reported a server and the Network fallback is empty or invalid)")
        else
            shroudforge.log.debug("World Editor P2P target is unavailable while the active target is local or not yet validated")
        end
    end

    function editor.valid_peer_id(value)
        return type(value) == "string" and #value >= 16 and #value <= 20 and
            value:match("^%d+$") ~= nil and value ~= "0"
    end

    function editor.server_peer_ids()
        local configured = editor.configured_client_steam_ids()
        if #configured > 0 then return configured end
        if not editor.editor_is_server or not runtime.network or not runtime.network.connected_peers then return {} end
        local peers, reason = runtime.network.connected_peers()
        if type(peers) ~= "table" then
            if reason then shroudforge.log.debug("World Editor could not read authenticated server players: " .. tostring(reason)) end
            return {}
        end
        local ids, seen = {}, {}
        for _, peer in ipairs(peers) do
            if editor.valid_peer_id(peer) and not seen[peer] then
                ids[#ids + 1], seen[peer] = peer, true
            end
        end
        return ids
    end

    function editor.on_p2p_result(operation, ok, transaction, reason, peer, request_id, uncertain, content)
        editor.remote_request_pending = false
        if operation == "capture" then
            local request = editor.remote_active_request
            editor.remote_active_request = nil
            editor.remote_stalled = false
            if not request or request.operation ~= "capture" or request.request_id ~= request_id or
               request.session ~= editor.current_session() then
                ok, reason = false, "world-session-changed-during-capture"
            end
            if ok then
                editor.process_step("Validating server blueprint", 3, 5,
                    "Received the complete server capture. Checking its format and placement recipes.")
                local parsed, loaded = pcall(editor.load_blueprint_content, "server-capture", content)
                if parsed and loaded then
                    editor.save_capture()
                    return
                end
                reason = parsed and "server-blueprint-validation-failed" or tostring(loaded)
            end
            editor.cancel_pending_queries("Server capture was not saved: " .. tostring(reason), "ready")
            editor.finish_process("error", "Server capture was not saved: " .. tostring(reason), "warn")
            shroudforge.log.warn("World Editor P2P capture failed: " .. tostring(reason))
        elseif operation == "abandon" then
            if ok then
                editor.remote_undo = nil
                editor.remote_recovery_required, editor.remote_release_pending = false, false
                editor.remote_active_request = nil
                editor.persist_server_undo_token()
                editor.set_editor_message("selected", "The server confirmed that the previous operation was abandoned and its World Editor lock released. Partial changes may remain; you can start a new operation.")
                shroudforge.log.info("World Editor server confirmed the idempotent abandon-and-release request")
                editor.finish_process("complete", "The server released the previous World Editor operation. Partial world changes may remain.")
            else
                editor.remote_release_pending = true
                editor.set_editor_message("recovery", "The server has not confirmed the release. {{keybind:pasteAtCursor}} stays paused. Press {{keybind:resetEditor}} to resend the same release request. " .. tostring(reason))
                shroudforge.log.warn("World Editor server release is not confirmed: " .. tostring(reason))
                editor.finish_process("error", editor.format_keybind_hint("The server has not confirmed release. Press {{keybind:resetEditor}} to retry: ") .. tostring(reason), "warn")
            end
        elseif operation == "paste" then
            if uncertain then
                editor.remote_active_request = {peer = peer, request_id = request_id, operation = operation}
                editor.remote_recovery_required = true
            else
                editor.remote_stalled = false
                if editor.remote_active_request and editor.remote_active_request.request_id == request_id then
                    editor.remote_active_request = nil
                end
                if ok then editor.remote_recovery_required = false end
            end
            if transaction then
                editor.remote_undo = {peer = peer, transaction = transaction}
                editor.persist_server_undo_token()
            end
            if uncertain then
                editor.set_editor_message("recovery", "The server did not answer within 60 seconds. The placement result is unknown. Press {{keybind:resetEditor}} to ask the server to abandon this operation; {{keybind:pasteAtCursor}} stays paused until release is confirmed.")
                shroudforge.log.warn(editor.format_keybind_hint("World Editor P2P paste timed out; outcome is unknown and {{keybind:resetEditor}} can request release"))
                editor.finish_process("error", editor.format_keybind_hint("The server did not answer. Press {{keybind:resetEditor}} to request release; the placement result may be partial."), "warn")
            elseif ok then
                editor.set_editor_message("selected", "Server placed the blueprint through its native world runtime. Press {{keybind:undoVoxels}} to undo.")
                shroudforge.log.info("World Editor P2P paste confirmed by dedicated server, undo token=" .. tostring(transaction))
                editor.finish_process("complete", "The server confirmed the placement in its live world. Save persistence is not verified.")
            else
                if transaction then editor.remote_recovery_required = true end
                editor.set_editor_message(transaction and "recovery" or "selected",
                    transaction and "Server placement needs recovery. Press {{keybind:undoVoxels}} to retry the server undo, or {{keybind:resetEditor}} to abandon this operation and release the server. Partial changes may remain; {{keybind:pasteAtCursor}} waits for confirmation." or
                        ("Server refused blueprint placement: " .. tostring(reason)))
                shroudforge.log.warn("World Editor P2P paste failed: " .. tostring(reason))
                editor.finish_process("error", "The server did not complete the placement: " .. tostring(reason), "warn")
            end
        elseif operation == "undo" then
            if uncertain then
                editor.remote_active_request = {peer = peer, request_id = request_id, operation = operation}
                editor.remote_recovery_required = true
            else
                editor.remote_stalled = false
                if editor.remote_active_request and editor.remote_active_request.request_id == request_id then
                    editor.remote_active_request = nil
                end
            end
            if uncertain then
                editor.set_editor_message("recovery", "The server did not answer within 60 seconds. The undo result is unknown. Press {{keybind:resetEditor}} to ask the server to abandon this operation; {{keybind:pasteAtCursor}} stays paused until release is confirmed.")
                shroudforge.log.warn(editor.format_keybind_hint("World Editor P2P undo timed out; outcome is unknown and {{keybind:resetEditor}} can request release"))
                editor.finish_process("error", editor.format_keybind_hint("The server did not answer. Press {{keybind:resetEditor}} to request release; undo may be partial."), "warn")
            elseif ok then
                editor.remote_undo = nil
                editor.remote_recovery_required = false
                editor.persist_server_undo_token()
                editor.set_editor_message("selected", "The server confirmed undo through its native world runtime.")
                shroudforge.log.info("World Editor P2P undo confirmed by dedicated server")
                editor.finish_process("complete", "The server confirmed that undo restored the previous live world state.")
            else
                editor.remote_recovery_required = true
                editor.set_editor_message("recovery", "Server undo is incomplete. Press {{keybind:undoVoxels}} to retry, or {{keybind:resetEditor}} to abandon this operation and release the server. Partial changes may remain; {{keybind:pasteAtCursor}} waits for confirmation. " .. tostring(reason))
                shroudforge.log.warn("World Editor P2P undo is incomplete: " .. tostring(reason))
                editor.finish_process("error", "Server undo is incomplete and can be retried: " .. tostring(reason), "warn")
            end
        end
    end

    function editor.on_p2p_progress(operation, state, completed, total, phase, message, peer)
        if state ~= "running" then return end
        if operation == "capture" then
            editor.remote_stalled = false
            editor.process_update("running", "Capture and save blueprint", phase, completed, total,
                "Server " .. tostring(peer or "") .. ": " .. tostring(message), "info")
            editor.set_editor_message("capturing", "The dedicated server is capturing the selection: " .. tostring(message))
            return
        end
        if editor.remote_stalled then
            editor.remote_stalled = false
            editor.set_editor_message("placing", operation == "undo" and
                "The dedicated server is responding again and is undoing the placement: " .. tostring(message) or
                "The dedicated server is responding again and is placing the blueprint: " .. tostring(message))
        end
        local label = operation == "undo" and "Undo placement on dedicated server" or "Place blueprint on dedicated server"
        editor.process_update("running", label, phase, completed, total,
            "Server " .. tostring(peer or "") .. ": " .. tostring(message), "info")
        if operation == "paste" then
            editor.set_editor_message("placing", "The dedicated server is working: " .. tostring(message))
        else
            editor.set_editor_message("placing", "The dedicated server is undoing the placement: " .. tostring(message))
        end
    end

    function editor.on_p2p_stalled(operation, peer)
        if operation == "capture" then
            editor.set_editor_message("capturing", "Waiting for the server capture. {{keybind:resetEditor}} cancels this read-only request.")
            return
        end
        local label = operation == "undo" and "undo" or "placement"
        editor.remote_stalled = true
        editor.set_editor_message("recovery", "The server has not reported progress for 10 seconds while " .. label .. " the blueprint. The result may be unknown. Press {{keybind:resetEditor}} to abandon the operation and request release; {{keybind:pasteAtCursor}} stays paused until the server confirms.")
        editor.finish_process("error", editor.format_keybind_hint("The server has not reported progress for 10 seconds. Press {{keybind:resetEditor}} to abandon the operation and request release; {{keybind:pasteAtCursor}} stays paused until confirmation."), "warn")
        shroudforge.log.warn("World Editor P2P " .. label .. " made no progress for 10 seconds with server " .. tostring(peer) .. editor.format_keybind_hint("; {{keybind:resetEditor}} offers abandon and release"))
    end

    function editor.capture_remote()
        if not editor.p2p_bridge or editor.remote_request_pending or editor.p2p_bridge.pending() then
            editor.set_editor_message(nil, "Wait for the active server request before starting another capture.")
            return
        end
        local peer = editor.configured_server_steam_id()
        if not peer then
            editor.set_editor_message("ready", "The dedicated server has not been discovered. Wait for its Steam P2P connection, then retry {{keybind:captureAndSave}}.")
            return
        end
        local session = editor.current_session()
        if session == 0 then return end
        editor.editor_stage = "capturing"
        editor.begin_process("Capture and save blueprint", "Requesting server capture", 5,
            "Sending the marked selection to the dedicated server for native terrain and prop capture.")
        editor.save_step(0, "Capturing blueprint on server")
        if not editor.export_ready_for_capture(true) then
            editor.cancel_pending_queries("Server capture stopped because local blueprint export is unavailable.", "ready")
            return
        end
        local a, b = editor.selection_a, editor.selection_b
        local ok, request = editor.p2p_bridge.start_capture(peer, {
            a={a.x,a.y,a.z}, b={b.x,b.y,b.z}, axis=editor.setting("rotationAxis")})
        if not ok then
            editor.cancel_pending_queries("Server capture could not be requested: " .. tostring(request), "ready")
            return
        end
        editor.remote_request_pending = true
        editor.remote_active_request = {peer=peer,request_id=request,operation="capture",session=session}
        editor.set_editor_message("capturing", "The server is reading the selection. The complete blueprint will be saved locally.")
        shroudforge.log.info("World Editor queued P2P capture request " .. request .. " to server " .. peer)
    end

    function editor.server_begin_remote_capture(peer, request_id, metadata, done, progress)
        if not editor.editor_is_server then done(nil, "server-runtime-not-active"); return end
        if editor.server_remote_capture or editor.server_remote_paste or editor.server_remote_undo or
           editor.pending_world_action or editor.pending_prop_capture then
            done(nil, "server-world-editor-is-busy")
            return
        end
        local a, b = metadata.a, metadata.b
        local before = {clipboard=editor.clipboard,name=editor.active_blueprint_name,
            selection_a=editor.selection_a,selection_b=editor.selection_b,preview=editor.placement_preview,
            override=editor.remote_setting_override}
        editor.server_remote_capture = {peer=peer,request_id=request_id,done=done,progress=progress,
            session=editor.current_session(),elapsed=0,before=before,started=false}
        editor.clipboard, editor.active_blueprint_name, editor.placement_preview = nil, nil, nil
        editor.selection_a, editor.selection_b = {x=a[1],y=a[2],z=a[3]}, {x=b[1],y=b[2],z=b[3]}
        editor.remote_setting_override = {rotationAxis=metadata.axis}
        editor.remote_progress_callback = progress
        editor.begin_process("Capture blueprint data", "Reading terrain", 2,
            "Reading the authorized client's selection. No world changes are requested.")
        shroudforge.log.info("World Editor received P2P capture request " .. request_id .. " from " .. peer)
    end

    function editor.finish_server_remote_capture(job, content, reason)
        if editor.server_remote_capture ~= job then return end
        editor.server_remote_capture, editor.remote_progress_callback = nil, nil
        editor.clipboard, editor.active_blueprint_name = job.before.clipboard, job.before.name
        editor.placement_preview = job.before.preview
        editor.remote_setting_override = job.before.override
        if job.session == editor.current_session() then
            editor.selection_a, editor.selection_b = job.before.selection_a, job.before.selection_b
        else
            editor.selection_a, editor.selection_b, editor.placement_preview = nil, nil, nil
        end
        shroudforge.log.info("World Editor P2P capture " .. job.request_id .. ": " ..
            (content and ("returning " .. #content .. " bytes") or tostring(reason)))
        job.done(content, reason)
    end

    function editor.update_server_remote_capture(delta_seconds)
        local job = editor.server_remote_capture
        if not job then return end
        job.elapsed = job.elapsed + math.max(0, tonumber(delta_seconds) or 0)
        local session = editor.current_session()
        if (job.session ~= 0 and session ~= job.session) or job.elapsed >= 45 then
            local reason = session ~= job.session and "server-world-session-changed-during-capture" or "server-capture-timeout"
            editor.cancel_pending_queries(reason, "ready")
            editor.finish_server_remote_capture(job, nil, reason)
            return
        end
        if not job.started then
            if session == 0 then return end
            job.session, job.started = session, true
            editor.copy_voxels(false)
        end
        if editor.pending_world_action or editor.pending_prop_capture then return end
        local content, reason
        if editor.process_progress.state == "complete" and editor.clipboard then
            content, reason = editor.encode_blueprint(editor.clipboard)
        else
            reason = editor.editor_hint or "server-native-capture-was-not-confirmed"
        end
        editor.finish_server_remote_capture(job, content, reason)
    end

    function editor.server_begin_remote_paste(peer, request_id, content, metadata, done, progress,
        validated_blueprint, validated_cell_count, validated_prop_count)
        if not editor.editor_is_server then done(false, nil, "server-runtime-not-active"); return end
        local retained = editor.server_remote_transactions[peer]
        if retained and retained.undo.recovery_required then
            -- The client may have timed out or lost the original paste reply after
            -- the server created its undo journal. Return the existing token with
            -- the refusal so the client can recover it and retry Undo last paste.
            done(false, retained.transaction,
                "server-has-an-unresolved-world-editor-transaction, press {{keybind:undoVoxels}} to resume its undo")
            return
        end
        if editor.server_remote_paste or editor.server_remote_transaction or editor.undo_state then
            done(false, nil, "server-has-an-unresolved-world-editor-transaction")
            return
        end
        local retained_count = 0
        for _ in pairs(editor.server_remote_transactions) do retained_count=retained_count+1 end
        if not retained and retained_count >= 32 then done(false,nil,"server-undo-journal-limit-reached");return end
        editor.remote_progress_callback = progress
        editor.begin_process("Place blueprint from client", "Validating blueprint", 5,
            "Received the complete blueprint from the authorized client.")
        editor.clipboard, editor.active_blueprint_name = nil, nil
        local blueprint_name = "remote-" .. tostring(request_id):gsub("[^%w_-]", "")
        editor.load_blueprint(blueprint_name, content,
            validated_blueprint, validated_cell_count, validated_prop_count)
        if not editor.clipboard then
            done(false, nil, editor.editor_hint or "server-could-not-parse-blueprint")
            editor.finish_process("error", "The uploaded blueprint could not be parsed.", "warn")
            editor.remote_progress_callback = nil
            return
        end
        local configured_rotation = math.floor(tonumber(shroudforge.settings.get("rotationQuarterTurns")) or 0) % 4
        editor.active_rotation_turns, editor.observed_rotation_setting = metadata.turns, configured_rotation
        local anchor = metadata.anchor
        -- The transport validated this exact world-space anchor. Grid metadata,
        -- alignment, rotation and bounds belong to the canonical placement,
        -- where a busy original Scene is retried before any mutation.
        editor.remote_setting_override = {pasteVoxelMode = metadata.voxelMode, targetPropMode = metadata.targetPropMode}
        editor.server_remote_paste = {peer = peer, request_id = request_id, metadata = metadata, progress = progress,
            done = done, cursor = {x=anchor[1], y=anchor[2], z=anchor[3]}, elapsed = 0, started = false,
            session = editor.current_session(), initial_undo = editor.undo_state}
        shroudforge.log.debug(string.format("World Editor P2P received validated V8 blueprint %s from Steam peer %s (%d bytes, server applies it natively)",
            blueprint_name, peer, #content))
        shroudforge.log.info("World Editor: dedicated server received and validated the blueprint. Native placement is starting.")
    end

    function editor.server_begin_remote_undo(peer, request_id, transaction, done, progress)
        local journal = editor.server_remote_transactions[peer]
        if not journal or journal.peer ~= peer or journal.transaction ~= transaction then
            done(false, "server-undo-token-is-stale-or-belongs-to-another-peer")
            return
        end
        if editor.server_remote_undo then done(false, "server-undo-is-already-running"); return end
        editor.server_remote_transaction, editor.undo_state = journal, journal.undo
        editor.remote_progress_callback = progress
        editor.begin_process("Undo placement from client", "Checking undo journal", 4,
            "Received the client's request to restore its previous world state.")
        editor.server_remote_undo = {peer = peer, request_id = request_id, transaction = transaction, progress = progress,
            done = done, elapsed = 0, session = journal.session}
    end

    function editor.server_begin_remote_abandon(peer, abandon_id, transaction, target_request_id, done)
        if not editor.editor_is_server then done(false, "server-runtime-not-active"); return end
        local journal = editor.server_remote_transactions[peer]
        if not journal then
            -- A repeated abandon may arrive after the server already released the
            -- transaction but its first acknowledgement was lost.
            done(true, "server-operation-already-released")
            return
        end
        local matches_transaction = transaction and journal.transaction == transaction
        local matches_request = target_request_id and journal.request_id == target_request_id
        if not matches_transaction and not matches_request then
            done(false, "abandon-request-does-not-match-the-server-operation")
            return
        end
        -- The P2P bridge serializes this behind active paste/undo work. Reaching
        -- this callback means no native edit step is still running for this peer.
        editor.pending_cursor_action, editor.pending_prop_capture, editor.pending_world_action = nil, nil, nil
        editor.server_remote_paste, editor.server_remote_undo = nil, nil
        editor.server_remote_transaction, editor.undo_state = nil, nil
        editor.server_remote_transactions[peer] = nil
        editor.remote_setting_override, editor.remote_progress_callback = nil, nil
        editor.placement_preview = nil
        editor.editor_stage = "need_a"
        editor.editor_hint = "An authorized client abandoned the previous World Editor operation and the server released its lock. Partial terrain or props may remain."
        if editor.process_progress.state == "running" then
            editor.finish_process("error", "World Editor operation abandoned by its authorized client. Partial world changes may remain.", "warn")
        end
        editor.publish_editor_state()
        shroudforge.log.warn("World Editor server released abandoned transaction " .. tostring(journal.transaction) ..
            " for authorized Steam peer " .. peer .. " at request " .. tostring(abandon_id) .. "; partial changes may remain")
        done(true, "server-operation-abandoned-and-lock-released")
    end

    editor.abandon_remote = function()
        if not editor.p2p_bridge then
            editor.set_editor_message("recovery", "Steam P2P is unavailable. The server release could not be requested; {{keybind:pasteAtCursor}} stays paused.")
            return false
        end
        local request = editor.remote_active_request
        local peer = editor.remote_undo and editor.remote_undo.peer or (request and request.peer)
        local transaction = editor.remote_undo and editor.remote_undo.transaction or nil
        if not peer then peer = editor.configured_server_steam_id() end
        if not peer then
            editor.set_editor_message("recovery", "The server connection is unavailable. {{keybind:resetEditor}} cannot request release yet; {{keybind:pasteAtCursor}} stays paused until the server confirms.")
            return false
        end
        local target_request_id = request and request.request_id or nil
        local ok, request_id = editor.p2p_bridge.start_abandon(peer, transaction, target_request_id)
        if not ok then
            editor.set_editor_message("recovery", "Could not request server release: " .. tostring(request_id) .. ". {{keybind:pasteAtCursor}} stays paused.")
            return false
        end
        local already_waiting = editor.remote_release_pending
        editor.remote_release_pending = true
        editor.remote_request_pending = true
        if not already_waiting then
            editor.begin_process("Release server World Editor operation", "Requesting server release", 1,
                "Asking the dedicated server to discard the old recovery and confirm that its lock is released.")
        end
        editor.set_editor_message("recovery", "{{keybind:resetEditor}} sent the release request. Waiting for server confirmation; {{keybind:pasteAtCursor}} stays paused until the server confirms. Press {{keybind:resetEditor}} again to resend if the reply is lost.")
        shroudforge.log.info("World Editor queued idempotent abandon request " .. tostring(request_id) ..
            " for server peer " .. peer .. " targeting transaction " .. tostring(transaction or target_request_id or "unknown"))
        return true
    end

    function editor.network_api()
        return {network = runtime.network, world = runtime.world, is_server = editor.editor_is_server,
            log = shroudforge.log}
    end

    function editor.create_p2p_bridge()
        if not runtime.network then return nil end
        return require("p2p")(editor.network_api(), {
            is_allowed_peer = function(peer)
                for _, allowed in ipairs(editor.server_peer_ids()) do
                    if allowed == peer then return true end
                end
                return false
            end,
            on_result = editor.on_p2p_result,
            on_progress = editor.on_p2p_progress,
            on_stalled = editor.on_p2p_stalled,
            validate_blueprint = editor.validate_blueprint_content,
            on_paste = editor.server_begin_remote_paste,
            on_undo = editor.server_begin_remote_undo,
            on_abandon = editor.server_begin_remote_abandon,
            on_capture = editor.server_begin_remote_capture,
        })
    end

    editor.paste_remote = function(use_current_cursor, cursor_override)
        if not editor.p2p_bridge then
            editor.set_editor_message("selected", "Steam P2P is unavailable in this client runtime.")
            return
        end
        -- The persisted token can outlive a successful server-side undo when its
        -- acknowledgement is lost. The server owns the authoritative journal and
        -- rejects Paste at cursor while that journal is genuinely unresolved, so a stale client
        -- token must not permanently lock placement.
        local peer, discovered_local_server = editor.configured_server_steam_id()
        if not peer then
            editor.process_progress.sequence = editor.process_progress.sequence + 1
            shroudforge.log.warn(string.format("World Editor operation #%d (place-blueprint): blocked. Dedicated Server SteamID64 is missing, request=not-sent.", editor.process_progress.sequence))
            editor.set_editor_message("selected", "Set the dedicated server's SteamID64 in World Editor → Server P2P before using {{keybind:pasteAtCursor}} online.")
            return
        end
        local function start_at(anchor)
            if not editor.clipboard then return end
            local content, reason = editor.encode_blueprint(editor.clipboard)
            if not content then editor.set_editor_message("selected", tostring(reason)); return end
            editor.begin_process("Place blueprint on dedicated server", "Sending blueprint", 0,
                string.format("Sending %s to the dedicated server.", editor.active_blueprint_name or "the active blueprint"))
            local ok, request = editor.p2p_bridge.start_paste(peer, content, {
                anchor = anchor, turns = editor.rotation_turns(),
                voxelMode = editor.setting("pasteVoxelMode") == "add" and "add" or "replace",
                targetPropMode = editor.setting("targetPropMode") == "replace" and "replace" or "keep",
            })
            if not ok then
                editor.set_editor_message("selected", tostring(request))
                editor.finish_process("error", "The client could not send the blueprint: " .. tostring(request), "warn")
                return
            end
            editor.remote_request_pending = true
            editor.remote_active_request = {peer = peer, request_id = request, operation = "paste"}
            editor.set_editor_message("building", "Sending the validated V8 blueprint to the dedicated server. Its runtime performs the placement. Wait for confirmation before pressing {{keybind:undoVoxels}}.")
            shroudforge.log.debug("World Editor queued P2P blueprint request " .. tostring(request) .. " to server Steam peer " .. peer ..
                (discovered_local_server and " (discovered from the running Dedicated Server)" or " (configured fallback)"))
            shroudforge.log.info("World Editor: blueprint placement request sent to the dedicated server. Waiting for its result.")
        end
        local anchor, reason = editor.target_anchor(use_current_cursor, cursor_override)
        if not anchor and use_current_cursor and editor.query_is_pending(reason) then
            editor.request_cursor_action(function(point) start_at({point.x, point.y, point.z}) end,
                "Reading the cursor before sending the blueprint to the server…")
            return
        end
        if not anchor then
            editor.set_editor_message("selected", "Server placement stopped. " .. tostring(reason) .. " Aim at a valid target and retry.")
            return
        end
        start_at(anchor)
    end

    function editor.finish_server_remote_paste(job, ok, transaction, reason)
        if editor.server_remote_paste ~= job then return end
        editor.server_remote_paste = nil
        editor.remote_setting_override = nil
        editor.remote_progress_callback = nil
        if not ok and editor.process_progress.state == "running" then
            editor.finish_process("error", "Server placement did not complete: " .. tostring(reason), "warn")
        end
        job.done(ok, transaction, reason)
    end

    function editor.update_server_remote_paste(delta_seconds)
        local job = editor.server_remote_paste
        if not job then return end
        job.elapsed = job.elapsed + math.max(0, tonumber(delta_seconds) or 0)
        local session = editor.current_session()
        if job.session ~= 0 and session ~= 0 and session ~= job.session then
            editor.finish_server_remote_paste(job, false, nil, "server-world-session-changed-before-placement")
            return
        end
        if not job.started then
            if session == 0 then
                editor.process_step("Waiting for server world session", 0, 0,
                    "Waiting for the server's native world session to become available.", "debug")
                if job.elapsed >= 45 then editor.finish_server_remote_paste(job, false, nil, "server-world-runtime-did-not-become-ready"); return end
                return
            end
            -- The canonical placement owns feature requirements and bounded
            -- readiness continuations. Repeating them here would require all
            -- independent Scene samples to succeed together and would demand
            -- a scale writer even for props spawned at their original size.
            job.session = session
            job.started = true
            editor.remote_progress_callback = job.progress
            editor.process_step("Preparing server placement", 1, 5,
                "The server world is ready. Preparing the target and undo data.")
            local before = editor.undo_state
            editor.paste_voxels(true, job.cursor)
            job.initial_undo = before
        end
        if editor.undo_state and editor.undo_state ~= job.initial_undo then
            local transaction = "tx_" .. tostring(job.request_id):gsub("[^%w_-]", "")
            editor.server_remote_transactions[job.peer] = {peer = job.peer, transaction = transaction,
                request_id = job.request_id, session = job.session, undo = editor.undo_state}
            local recovery_required = editor.undo_state.recovery_required
            editor.undo_state = nil
            if recovery_required then
                editor.finish_server_remote_paste(job, false, transaction, "placement-partially-applied. Press {{keybind:undoVoxels}} for server recovery")
            else
                editor.finish_server_remote_paste(job, true, transaction, "placement-confirmed")
            end
        elseif editor.pending_prop_capture or editor.pending_world_action then
            return
        else
            editor.finish_server_remote_paste(job, false, nil, editor.editor_hint or "server-native-placement-was-not-confirmed")
        end
    end
end
