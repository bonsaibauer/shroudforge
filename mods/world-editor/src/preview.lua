-- Editor preview, selection and progress presentation.
return function(editor)
    local last_publish_failure

    function editor.process_update(state, operation, phase, completed, total, message, level)
        local next_operation = operation or editor.process_progress.operation
        local next_phase = phase or ""
        local next_completed = math.max(0, tonumber(completed) or 0)
        local next_total = math.max(0, tonumber(total) or 0)
        if editor.process_progress.state == state and editor.process_progress.operation == next_operation and
           editor.process_progress.phase == next_phase and editor.process_progress.completed == next_completed and
           editor.process_progress.total == next_total then return end
        editor.process_progress.state = state
        editor.process_progress.operation = next_operation
        editor.process_progress.phase = next_phase
        editor.process_progress.completed = next_completed
        editor.process_progress.total = next_total
        if state == "running" and message then
            local log_message = string.format("World Editor operation #%d (%s): %s — %s",
                editor.process_progress.sequence, editor.process_progress.operation, editor.process_progress.phase, tostring(message))
            local logger = shroudforge.log[level or "info"] or shroudforge.log.info
            logger(log_message)
            if editor.remote_progress_callback then
                pcall(editor.remote_progress_callback, editor.process_progress.phase, editor.process_progress.completed,
                    editor.process_progress.total, message)
            end
        elseif state ~= "running" and message then
            local log_message = string.format("World Editor operation #%d (%s): %s — %s",
                editor.process_progress.sequence, editor.process_progress.operation,
                state == "complete" and "completed" or state, tostring(message))
            local logger = shroudforge.log[level or (state == "error" and "error" or "info")] or shroudforge.log.info
            logger(log_message)
        end
        if editor.publish_editor_state then editor.publish_editor_state() end
    end

    function editor.begin_process(operation, phase, total, message)
        editor.process_progress.sequence = editor.process_progress.sequence + 1
        editor.process_progress.state = "idle"
        editor.process_update("running", operation, phase, 0, total, message, "info")
    end

    function editor.process_step(phase, completed, total, message, level)
        editor.process_update("running", editor.process_progress.operation, phase, completed, total, message, level or "info")
    end

    function editor.finish_process(state, message, level)
        editor.process_update(state, editor.process_progress.operation, editor.process_progress.phase,
            editor.process_progress.completed, editor.process_progress.total, message, level)
    end

    function editor.save_step(completed, phase, state)
        if completed == 0 then editor.save_progress.sequence = editor.save_progress.sequence + 1 end
        editor.save_progress.completed, editor.save_progress.phase = completed, phase
        editor.save_progress.state = state or "saving"
        if editor.publish_editor_state then editor.publish_editor_state() end
    end

    function editor.key_pressed(key)
        local pressed = shroudforge.input.was_keybind_pressed(key)
        editor.previous_key_state[key] = true
        if pressed then
            shroudforge.log.debug("World Editor input: action=" .. key .. " key=" .. tostring(shroudforge.input.keybind_label(key)))
        end
        return pressed
    end

    editor.publish_editor_state = function()
        if not editor.editor_ui_available then return true end
        if editor.save_progress.state == "saving" and editor.editor_stage == "ready" then
            editor.save_progress.state, editor.save_progress.phase = "error", "Save stopped"
        end
        local axis = editor.clipboard and editor.clipboard.region and editor.clipboard.region.rotationAxis or editor.setting("rotationAxis")
        if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
        local panel_width = tonumber(editor.setting("panelWidth")) or 1180
        local panel_height = tonumber(editor.setting("panelHeight")) or 190
        local panel_position_x = tonumber(editor.setting("panelPositionX")) or -1
        local panel_position_y = tonumber(editor.setting("panelPositionY")) or 92
        if panel_width ~= panel_width then panel_width = 1180 end
        if panel_height ~= panel_height then panel_height = 190 end
        if not editor.finite_number(panel_position_x) then panel_position_x = -1 end
        if not editor.finite_number(panel_position_y) then panel_position_y = 92 end
        panel_width = math.floor(math.min(1920, math.max(760, panel_width)))
        panel_height = math.floor(math.min(400, math.max(140, panel_height)))
        panel_position_x = math.floor(math.min(8192, math.max(-1, panel_position_x)))
        panel_position_y = math.floor(math.min(8192, math.max(0, panel_position_y)))
        local lines = {
            "saveState=" .. editor.save_progress.state,
            "saveCompleted=" .. tostring(editor.save_progress.completed),
            "savePhase=" .. editor.save_progress.phase,
            "saveSequence=" .. tostring(editor.save_progress.sequence),
            "processState=" .. editor.process_progress.state,
            "processOperation=" .. editor.process_progress.operation,
            "processPhase=" .. editor.process_progress.phase,
            "processCompleted=" .. tostring(editor.process_progress.completed),
            "processTotal=" .. tostring(editor.process_progress.total),
            "processSequence=" .. tostring(editor.process_progress.sequence),
            "stage=" .. tostring(editor.editor_stage),
            "hint=" .. editor.format_keybind_hint(editor.editor_hint),
            "selected=" .. tostring(editor.active_blueprint_name or ""),
            "axis=" .. axis,
            "turns=" .. tostring(editor.rotation_turns()),
            "panelWidth=" .. tostring(panel_width),
            "panelHeight=" .. tostring(panel_height),
            "panelPositionX=" .. tostring(panel_position_x),
            "panelPositionY=" .. tostring(panel_position_y),
            "processRole=" .. (runtime.is_server and "dedicated-server" or "client"),
            "executionBackend=" .. editor.execution_backend_label(editor.execution_backend()),
            "runtimeReadiness=" .. editor.native_readiness_state,
            "runtimeReadinessReason=" .. editor.native_readiness_reason,
            "sessionGeneration=" .. tostring(editor.current_session()),
            "archivedSessions=" .. tostring(#editor.retired_sessions),
        }
        local keybind_keys = {}
        for key in pairs(editor.previous_key_state) do keybind_keys[#keybind_keys + 1] = key end
        table.sort(keybind_keys)
        for _, key in ipairs(keybind_keys) do
            -- The native panel resolves these currently applied settings with SDL.
            lines[#lines + 1] = "keybind." .. key .. "=" .. tostring(editor.setting(key))
        end
        local text = table.concat(lines, "\n") .. "\n"
        if text == editor.last_published_editor_state then return true end
        local ok, reason = pcall(io.export, "world-editor/editor-state.txt", text)
        if not ok then
            local failure = tostring(reason)
            if failure ~= last_publish_failure then
                shroudforge.log.warn("World Editor could not publish its on-screen helper state: " .. failure)
                last_publish_failure = failure
            end
            return false
        end
        last_publish_failure = nil
        editor.last_published_editor_state = text
        return true
    end

    editor.set_editor_message = function(stage, message)
        if stage then editor.editor_stage = stage end
        if editor.save_progress.state == "saving" and (stage == "ready" or stage == "need_a") then
            editor.save_progress.state, editor.save_progress.phase = "error", "Save stopped"
        end
        editor.editor_hint = message
        if editor.publish_editor_state then editor.publish_editor_state() end
    end

    function editor.cursor_point()
        local reason = editor.unavailable_world_reason("runtime.world.cursor.get")
        if reason then return nil, reason end
        local snapshot, reason = runtime.world.cursor.get()
        if not snapshot then return nil, reason end
        local value = snapshot.value
        local transform = value and value.primaryTransform
        local position = transform and editor.cursor_world_position(transform.position)
        if position then return position end
        return nil, "native cursor snapshot has no readable primary transform"
    end

    function editor.request_cursor_action(action, waiting_hint)
        if editor.pending_cursor_action then
            editor.set_editor_message(nil, "A cursor request is already running. Wait for its instruction to update, then try again.")
            shroudforge.log.debug("World Editor is already waiting for its live cursor query")
            return
        end
        local point, reason = editor.cursor_point()
        if point then action(point); return end
        if editor.query_is_pending(reason) then
            editor.pending_cursor_action = action
            editor.set_editor_message(nil, waiting_hint or "Reading the aimed cursor position…")
            shroudforge.log.debug("World Editor is resolving the live cursor. The requested action will continue automatically.")
        else
            editor.set_editor_message(nil, "Could not read the aimed cursor position. Aim at a surface and try again.")
            shroudforge.log.warn("World Editor cursor is unavailable: " .. tostring(reason))
        end
    end

    function editor.mark_cursor(which)
        editor.request_cursor_action(function(point)
            if which == "a" then editor.selection_a = point
            elseif which == "b" then editor.selection_b = point
            else editor.selection_target = point end
            if which == "a" then
                editor.editor_stage = "need_b"
                editor.editor_hint = "Corner A is marked. Aim at the opposite corner and press {{keybind:markCursorNext}} again."
            elseif which == "b" then
                editor.editor_stage = "ready"
                editor.editor_hint = "Both corners are marked. Press {{keybind:captureAndSave}} to capture and save the blueprint."
            end
            editor.publish_editor_state()
            shroudforge.log.debug(string.format("World Editor cursor selection %s = %.3f, %.3f, %.3f",
                which == "target" and "TARGET" or which:upper(), point.x, point.y, point.z))
        end)
    end

    function editor.preview_paste_at(point)
        if not editor.clipboard then
            editor.set_editor_message("need_a", "No blueprint is active. Press {{keybind:markCursorNext}} to create one or select a saved blueprint.")
            shroudforge.log.warn("World Editor: copy or load a blueprint before previewing")
            return
        end
        editor.begin_process("Preview placement", "Checking cursor target", 2,
            "Checking the aimed position and voxel-grid alignment. No world changes will be made.")
        local turns = editor.rotation_turns()
        local axis = editor.clipboard.region.rotationAxis or editor.setting("rotationAxis")
        if axis ~= "x" and axis ~= "y" and axis ~= "z" then axis = "y" end
        local plan = editor.rotated_blueprint(editor.clipboard, turns)
        local anchor = {point.x, point.y, point.z}
        local target, reason = editor.voxel_target_region(plan, anchor)
        if editor.clipboard.hasVoxels and not target then
            editor.set_editor_message("selected", "Preview could not use that target. " .. tostring(reason) .. " Aim at another point.")
            shroudforge.log.warn("World Editor preview rejected: " .. tostring(reason))
            editor.finish_process("error", "Preview could not use the target: " .. tostring(reason), "warn")
            return
        end
        local occupied = 0
        for _, cell in ipairs(plan.cells or {}) do if cell ~= 0 then occupied = occupied + 1 end end
        editor.placement_preview = {blueprint = editor.clipboard, target = target, anchor = anchor, turns = turns, axis = axis,
            plan = plan, bounds = editor.plan_world_bounds(plan, anchor)}
        editor.set_editor_message("selected", "Preview ready. No world changes were made. Press {{keybind:pasteAtCursor}} to place the blueprint.")
        shroudforge.log.info(string.format(
            "World Editor placement plan prepared: axis=%s turns=%d anchor=%.3f,%.3f,%.3f voxels=%s size=%d,%d,%d occupied=%d props=%d. No world changes were made.",
            axis, turns, anchor[1], anchor[2], anchor[3], tostring(plan.hasVoxels), plan.sx, plan.sy, plan.sz, occupied, #plan.props))
        editor.process_step("Placement plan ready", 2, 2,
            string.format("Preview contains %d occupied cells and %d props. No world changes were made.", occupied, #plan.props))
        editor.finish_process("complete", "Preview ready. Nothing was written to the world.")
    end

    function editor.preview_paste()
        if not editor.clipboard then
            editor.set_editor_message("need_a", "No blueprint is active. Press {{keybind:markCursorNext}} to create one or select a saved blueprint.")
            shroudforge.log.warn("World Editor: copy or load a blueprint before previewing")
            return
        end
        editor.request_cursor_action(editor.preview_paste_at, "Reading the cursor to prepare a placement preview…")
    end

    function editor.clear_cursor_selection()
        editor.selection_a, editor.selection_b, editor.selection_target = nil, nil, nil
        shroudforge.log.debug("World Editor cursor marks cleared. Manual coordinates are active.")
    end

    function editor.mark_cursor_next()
        if editor.editor_stage == "capturing" or editor.editor_stage == "placing" then
            shroudforge.log.warn("World Editor: wait for the current capture or placement to finish")
            return
        end
        local starting_new_capture = editor.editor_stage ~= "need_b"
        if starting_new_capture then
            editor.clipboard, editor.active_blueprint_name, editor.placement_preview = nil, nil, nil
            editor.selection_a, editor.selection_b = nil, nil
            editor.editor_stage = "need_a"
            editor.editor_hint = "New blueprint selected. Aim at the first corner and press {{keybind:markCursorNext}} to mark it."
            editor.publish_editor_state()
        end
        editor.request_cursor_action(function(point)
            if starting_new_capture then
                editor.selection_a, editor.selection_b = point, nil
                editor.editor_stage = "need_b"
                editor.editor_hint = "Corner A is marked. Aim at the opposite corner and press {{keybind:markCursorNext}} again."
                editor.publish_editor_state()
                shroudforge.log.debug(editor.format_keybind_hint(string.format("World Editor help: first corner marked at %.3f, %.3f, %.3f. Aim at the opposite corner and press {{keybind:markCursorNext}} again.",
                    point.x, point.y, point.z)))
            else
                editor.selection_b = point
                editor.editor_stage = "ready"
                editor.editor_hint = "Both corners are marked. Press {{keybind:captureAndSave}} to capture and save the blueprint."
                editor.publish_editor_state()
                shroudforge.log.debug(editor.format_keybind_hint(string.format("World Editor help: second corner marked at %.3f, %.3f, %.3f. Selection is ready. Press {{keybind:captureAndSave}} to capture and save.",
                    point.x, point.y, point.z)))
            end
        end, starting_new_capture and "Reading the first selection corner…" or "Reading the second selection corner…")
    end

    function editor.start_new_blueprint()
        editor.pending_cursor_action, editor.pending_prop_capture, editor.pending_world_action = nil, nil, nil
        editor.clipboard, editor.active_blueprint_name, editor.placement_preview = nil, nil, nil
        editor.selection_a, editor.selection_b = nil, nil
        editor.set_editor_message("need_a", "New blueprint selected.")
        editor.editor_stage = "need_a"
        editor.editor_hint = "New blueprint selected. Aim at the first corner and press {{keybind:markCursorNext}} to mark it."
        editor.publish_editor_state()
        shroudforge.log.debug(editor.format_keybind_hint("World Editor: new blueprint capture selected. Press {{keybind:markCursorNext}} to mark the first corner."))
    end
end
