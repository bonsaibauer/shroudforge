-- World Editor integration through the Schematics API.
return function(editor)
    function editor.finish_save_progress()
        local capture_save = editor.process_progress.operation == "Capture and save blueprint"
        editor.process_step("Refreshing blueprint library", capture_save and 5 or 3,
            capture_save and 5 or 3, "Updating the blueprint list.")
        if editor.refresh_blueprint_library and editor.refresh_blueprint_library() then
            editor.save_step(4, "Blueprint saved", "complete")
            editor.finish_process("complete", "The blueprint is saved and ready to use.")
        else
            editor.save_step(3, "Saved, library refresh failed", "error")
            editor.finish_process("error", "The file was saved, but the blueprint list could not be refreshed.", "warn")
        end
    end

    function editor.export_ready_for_capture(save_and_select)
        if not save_and_select then return true end
        local ok, result = pcall(shroudforge.schematics.list)
        if not ok or type(result) ~= "table" then
            shroudforge.log.error(editor.format_keybind_hint("World Editor cannot start {{keybind:captureAndSave}} capture because blueprint export is unavailable: ") .. tostring(result))
            return false
        end
        return true
    end

    -- Both local and server captures use one non-overwriting save path.
    function editor.save_capture()
        local name
        local ok, schematics = pcall(shroudforge.schematics.list)
        if not ok or type(schematics) ~= "table" then
            editor.set_editor_message("ready", "Capture stopped. The schematic library could not be checked. Check the export folder and try {{keybind:captureAndSave}} again.")
            editor.finish_process("error", "Could not inspect the schematic library: " .. tostring(schematics), "error")
            return false
        end
        local existing = {}
        for _, schematic in ipairs(schematics) do
            if type(schematic.name) == "string" then existing[schematic.name] = true end
        end
        for index = 1, 999999 do
            local candidate = "capture-" .. tostring(index)
            if not existing[candidate] then name = candidate; break end
        end
        if not name or not editor.save_blueprint_named(name) then
            editor.set_editor_message("ready", "Capture is ready, but the blueprint could not be saved. Check the export folder and try {{keybind:captureAndSave}} again.")
            shroudforge.log.error("World Editor captured the region but could not save a unique persistent blueprint")
            if editor.process_progress.state == "running" then
                editor.finish_process("error", "The capture is ready, but the blueprint file could not be saved.", "error")
            end
            return false
        end
        editor.active_blueprint_name = name
        editor.editor_stage = "selected"
        editor.set_editor_message("selected", "Blueprint " .. name .. " saved and selected. Press {{keybind:pasteAtCursor}} to place it. Covers can be managed in the library.")
        editor.finish_save_progress()
        shroudforge.log.debug("World Editor help: blueprint saved as '" .. name .. editor.format_keybind_hint("' and selected. Click to select it. Manage it in the library. Press {{keybind:pasteAtCursor}} to place it."))
        return editor.save_progress.state == "complete"
    end

    editor.save_blueprint_named = function(name)
        if not editor.clipboard then
            editor.set_editor_message("need_a", "There is no capture to save. Mark both corners with {{keybind:markCursorNext}} first.")
            shroudforge.log.warn("World Editor: capture a region before saving a blueprint")
            return
        end
        local prop_count = #(editor.clipboard.props or {})
        local maximum = editor.maximum_copyable_props()
        if prop_count > maximum then
            editor.set_editor_message("ready", string.format("Save stopped. This capture has %d props, above your limit of %d. Raise the limit in mod settings or capture a smaller area.", prop_count, maximum))
            shroudforge.log.warn(string.format("World Editor: cannot save %d props because the configured blueprint maximum is %d", prop_count, maximum))
            return false
        end
        if type(name) ~= "string" or not name:match("^[%w_-]+$") then
            editor.set_editor_message("ready", "Save stopped. Use letters, numbers, underscores, or hyphens in the blueprint name.")
            shroudforge.log.warn("World Editor: invalid blueprint name")
            return
        end
        if editor.process_progress.state ~= "running" then
            editor.begin_process("Save blueprint", "Encoding blueprint", 3,
                "Preparing " .. tostring(name) .. " for export.")
        end
        local capture_save = editor.process_progress.operation == "Capture and save blueprint"
        editor.process_step("Encoding blueprint", capture_save and 3 or 1, capture_save and 5 or 3,
            "Encoding " .. tostring(name) .. ".")
        -- A direct save already has a complete capture in the clipboard.
        if editor.save_progress.state ~= "saving" then
            editor.editor_stage = "capturing"
            editor.save_step(0, "Preparing blueprint")
        end
        editor.save_step(1, "Encoding blueprint")
        local content, encode_reason = editor.encode_blueprint(editor.clipboard)
        if not content then
            editor.set_editor_message("ready", "Save stopped. " .. tostring(encode_reason))
            shroudforge.log.warn("World Editor: " .. tostring(encode_reason))
            editor.finish_process("error", "Blueprint encoding failed: " .. tostring(encode_reason), "warn")
            return
        end
        if type(content) ~= "string" or #content > editor.maximum_blueprint_bytes then
            shroudforge.log.warn("World Editor: blueprint is not text or exceeds the 256 MiB file limit")
            editor.finish_process("error", "The encoded blueprint exceeds the supported file limit.", "warn")
            return false
        end
        editor.process_step("Saving blueprint", capture_save and 4 or 2, capture_save and 5 or 3,
            string.format("Saving %s (%d bytes) to the local library.", name, #content))
        editor.save_step(2, "Saving blueprint")
        local ok, err = pcall(shroudforge.schematics.save, name, content)
        if not ok then
            editor.set_editor_message("ready", "Save failed. Check that export storage is enabled, then try again.")
            shroudforge.log.error("World Editor could not save schematic through the Modloader API: " .. tostring(err))
            editor.finish_process("error", "Could not save the schematic: " .. tostring(err), "error")
            return false
        end
        editor.save_step(3, "Updating library")
        shroudforge.log.info("World Editor saved persistent schematic: " .. name)
        editor.process_step("Blueprint file written", capture_save and 4 or 2, capture_save and 5 or 3,
            "The blueprint was saved. Refreshing the list.")
        return true
    end

    function editor.save_blueprint()
        local name = editor.setting("blueprintName")
        if editor.save_blueprint_named(name) then
            editor.active_blueprint_name = name
            editor.set_editor_message("selected", "Blueprint " .. name .. " saved and selected. Press {{keybind:pasteAtCursor}} to place it. Covers can be managed in the library.")
            editor.finish_save_progress()
        end
    end

    function editor.load_blueprint(name, supplied_content, validated_blueprint, validated_cell_count, validated_prop_count)
        local remote_paste = editor.process_progress.state == "running" and
            editor.process_progress.operation == "Place blueprint from client"
        if remote_paste then
            editor.process_step("Validating uploaded blueprint", 1, 5,
                "Checking the blueprint format and its current placement recipes.")
        else
            editor.begin_process("Load blueprint", "Reading blueprint", 0,
                "Reading and checking the selected blueprint.")
        end
        local ok, loaded = pcall(editor.load_blueprint_content, name, supplied_content,
            validated_blueprint, validated_cell_count, validated_prop_count)
        if not ok then
            editor.finish_process("error", "Blueprint loading stopped: " .. tostring(loaded), "error")
            return false
        end
        if not loaded then
            editor.finish_process("error", "Blueprint could not be loaded. See the world-editor log for the validation reason.", "warn")
            return false
        end
        if remote_paste then
            editor.process_step("Blueprint validated", 2, 5,
                "Blueprint data and prop recipes are valid on the server.")
        else
            editor.process_step("Blueprint activated", 1, 1,
                "Blueprint is active and ready for preview or placement.")
            editor.finish_process("complete", "Blueprint loaded and activated.")
        end
        return true
    end

    editor.refresh_blueprint_library = function()
        local ok, schematics = pcall(shroudforge.schematics.list)
        if not ok or type(schematics) ~= "table" then
            shroudforge.log.warn("World Editor could not read the schematic library: " .. tostring(schematics))
            return false
        end
        local entries = {}
        for _, schematic in ipairs(schematics) do
            local name = schematic.name
            if name and name:match("^[%w_-]+$") then
                entries[#entries + 1] = {
                    name = name,
                    image = schematic.image,
                }
            end
        end
        table.sort(entries, function(left, right) return left.name:lower() < right.name:lower() end)
        editor.blueprint_library = entries
        if editor.publish_editor_state then editor.publish_editor_state() end
        shroudforge.log.debug(string.format("World Editor blueprint library refreshed: %d blueprint(s)", #entries))
        for index, entry in ipairs(entries) do
            shroudforge.log.debug(string.format("World Editor blueprint %d: %s%s", index, entry.name,
                entry.name == editor.active_blueprint_name and " [selected]" or ""))
        end
        return true
    end


    function editor.refresh_blueprint_library_with_progress()
        editor.begin_process("Refresh blueprint library", "Reading blueprint files", 1,
            "Refreshing the saved blueprint list.")
        local ok = editor.refresh_blueprint_library()
        if ok then
            editor.process_step("Blueprint library refreshed", 1, 1,
                "The saved blueprint list is up to date.")
            editor.finish_process("complete", "Blueprint library refreshed.")
        else
            editor.finish_process("error", "The blueprint list could not be refreshed. Check the World Editor log.", "warn")
        end
        return ok
    end
end
