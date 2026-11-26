-- The same editor module runs headless on the dedicated server. UI actions and
-- cursor input stay client-side; P2P requests enter the existing world API here.
local editor = {}

if type(runtime) ~= "table" then return {} end
runtime.require("runtime.lifecycle")
editor.editor_is_server = runtime.is_server == true
editor.editor_ui_available = not editor.editor_is_server and type(shroudforge) == "table" and type(shroudforge.ui) == "table"
if type(shroudforge) ~= "table" or (not editor.editor_is_server and not editor.editor_ui_available) then return {} end

editor.remote_setting_override = nil
function editor.setting(name)
    if editor.remote_setting_override and editor.remote_setting_override[name] ~= nil then
        return editor.remote_setting_override[name]
    end
    local value = shroudforge.settings.get(name)
    if value == nil then return "" end
    return value
end

-- Keep instructions live when settings or the keyboard layout change.
function editor.format_keybind_hint(message)
    return (tostring(message or ""):gsub("{{keybind:([%w_.-]+)}}", function(key)
        local ok, label = pcall(shroudforge.input.keybind_label, key)
        if ok then return label or "Disabled" end
        -- Unrelated error text can contain an unknown placeholder. Keep it intact.
    end))
end

editor.clipboard = nil
editor.placement_preview = nil
editor.undo_state = nil
editor.feature = nil
editor.selection_a = nil
editor.selection_b = nil
editor.selection_target = nil
editor.active_blueprint_name = nil
editor.live_prop_cache = {}
editor.placeable_items = nil
editor.prop_recipes_registered = false
editor.pending_cursor_action = nil
editor.pending_prop_capture = nil
editor.pending_world_action = nil
editor.readiness_logged = false
editor.readiness_wait_logged = false
editor.native_readiness_state = "waiting"
editor.native_readiness_reason = "Waiting for the first native world readiness sample."
editor.last_readiness_log_state = nil
editor.maximum_blueprint_bytes = 256 * 1024 * 1024
editor.maximum_supported_props = 1000000
editor.default_maximum_copyable_props = 60000
editor.save_blueprint_named = nil
editor.undo_voxels = nil
editor.rollback_partial_paste = nil
editor.refresh_blueprint_library = nil
editor.publish_editor_state = nil
editor.set_editor_message = nil
editor.previous_key_state = {}
editor.observed_rotation_setting = nil
editor.active_rotation_turns = nil
editor.voxel_grid_spec = nil
editor.editor_stage = "need_a"
editor.editor_hint = "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press {{keybind:markCursorNext}}."
editor.last_published_editor_state = nil
editor.last_execution_backend = nil
editor.last_logged_p2p_target = nil
editor.p2p_target_check_elapsed = 0
editor.native_readiness_elapsed = 0
editor.p2p_bridge = nil
editor.remote_undo = nil
editor.remote_request_pending = false
editor.remote_active_request = nil
editor.remote_recovery_required = false
editor.remote_release_pending = false
editor.remote_stalled = false
editor.abandon_remote = nil
editor.server_remote_paste = nil
editor.server_remote_transaction = nil
editor.server_remote_transactions = {}
editor.server_remote_undo = nil
editor.server_remote_capture = nil
editor.paste_remote = nil
editor.undo_remote = nil
editor.observed_session = nil
editor.retired_sessions = {}
editor.world_wait_timeout = 15

-- Overall save milestones, not a prop count or an estimated byte percentage.
editor.save_progress = {state = "idle", completed = 0, phase = "", sequence = 0}
editor.process_progress = {state = "idle", operation = "", phase = "", completed = 0,
    total = 0, sequence = 0}
editor.remote_progress_callback = nil

editor.blueprint_library = {}

editor.server_undo_export_path = "world-editor/server-undo-token.txt"
editor.server_undo_export_header = "SHROUDFORGE_WORLD_EDITOR_SERVER_UNDO_V1"

require("blueprint")(editor)
require("library")(editor)
require("preview")(editor)
require("operations")(editor)
require("history")(editor)

editor.p2p_bridge = editor.create_p2p_bridge()

editor.feature = function(feature_name)
    if runtime.has(feature_name) then return true end
    local status = runtime.status(feature_name)
    local native_mutation_unavailable = editor.native_mutation_unavailable_reason and
        editor.native_mutation_unavailable_reason(feature_name, status)
    if native_mutation_unavailable then
        editor.set_editor_message("selected", native_mutation_unavailable)
        shroudforge.log.warn("World Editor blocked " .. feature_name .. ": " .. native_mutation_unavailable)
        return false
    end
    editor.set_editor_message(nil, "Waiting for " .. feature_name .. ". " .. tostring(status and status.reason or "Runtime feature unavailable."))
    shroudforge.log.warn("World Editor: " .. feature_name .. " unavailable: " ..
        tostring(status and status.reason or "feature unavailable"))
    return false
end

function editor.on_action(name, callback)
    if not editor.editor_ui_available then return end
    shroudforge.ui.on_action(name, function(...)
        editor.update_world_session()
        shroudforge.log.debug("World Editor input: action=" .. tostring(name) .. " source=editor-panel")
        return callback(...)
    end)
end

editor.on_action("copyVoxels", editor.copy_voxels)
editor.on_action("captureAndSave", editor.capture_and_save)
editor.on_action("capturePropsOnly", function() editor.copy_props_only(false) end)
editor.on_action("saveBlueprint", editor.save_blueprint)
editor.on_action("loadBlueprint", editor.load_blueprint)
editor.on_action("selectBlueprint", function(name)
    if type(name) ~= "string" then return end
    editor.load_blueprint(name)
end)
editor.on_action("newBlueprint", editor.start_new_blueprint)
editor.on_action("refreshBlueprintLibrary", editor.refresh_blueprint_library_with_progress)
editor.on_action("pasteVoxels", editor.paste_voxels)
editor.on_action("previewPaste", editor.preview_paste)
editor.on_action("rotateBlueprint", editor.rotate_blueprint)
editor.on_action("pasteAtCursor", function() editor.paste_voxels(true) end)
editor.on_action("undoVoxels", editor.undo_voxels)
editor.on_action("markCursorNext", editor.mark_cursor_next)
editor.on_action("resetEditor", editor.reset_editor)
editor.on_action("spawnEntity", editor.spawn_entity)
editor.on_action("placeEntity", function() editor.placement_operation(false) end)
editor.on_action("destroyEntity", function() editor.placement_operation(true) end)
editor.on_action("destroySelectedProp", editor.destroy_selected_prop)
editor.on_action("markCursorA", function() editor.mark_cursor("a") end)
editor.on_action("markCursorB", function() editor.mark_cursor("b") end)
editor.on_action("markCursorTarget", function() editor.mark_cursor("target") end)
editor.on_action("clearCursorSelection", editor.clear_cursor_selection)
editor.on_action("listProps", editor.list_props)
editor.on_action("resolveRecipes", editor.report_recipe_catalog)

return {
    update_interval_ms = 100,
    on_load = function()
        if editor.editor_is_server then
            local status = runtime.network and runtime.network.status and runtime.network.status() or nil
            shroudforge.log.info("World Editor server target loaded. Client requests use Steam P2P and the server's native world runtime.")
            if status and status.local_steam_id then
                shroudforge.log.info("World Editor dedicated-server SteamID64: " .. tostring(status.local_steam_id))
            end
            if #editor.configured_client_steam_ids() == 0 then
                shroudforge.log.info("World Editor server P2P authorization follows Enshrouded's authenticated connected-player list. No client SteamID64 entry is required.")
            else
                shroudforge.log.info("World Editor server P2P authorization uses the Network module's explicit client allowlist (" .. #editor.configured_client_steam_ids() .. " IDs).")
                shroudforge.log.debug("Network allowedClientSteamIds=" .. table.concat(editor.configured_client_steam_ids(), ","))
            end
            if not status or not status.available then
                shroudforge.log.warn("World Editor server P2P is unavailable: " .. tostring(status and status.reason or "network runtime missing"))
            end
        else
            local backend = editor.execution_backend()
            editor.last_execution_backend = backend
            local target = backend == "p2p" and "Dedicated Server P2P" or
                backend == "direct" and "local / singleplayer direct" or "waiting for a validated world context"
            shroudforge.log.info("World Editor client target detection is automatic, current target: " .. target)
            editor.log_p2p_target_if_changed()
            if editor.recover_server_undo_token() then
                shroudforge.log.warn("World Editor recovered a pending server undo token from the local journal")
                editor.set_editor_message("recovery", backend == "p2p" and
                    "Recovered an unfinished server undo. Press {{keybind:undoVoxels}} to resume it. {{keybind:pasteAtCursor}} stays paused until the server confirms." or
                    "Recovered an unfinished server undo. Rejoin that Dedicated Server world and press {{keybind:undoVoxels}} to resume it.")
            end
            local status = runtime.network and runtime.network.status and runtime.network.status() or nil
            if status and status.local_steam_id then
                shroudforge.log.debug("World Editor client SteamID64 (add this to the server allowlist): " .. tostring(status.local_steam_id))
            end
            editor.refresh_blueprint_library()
            editor.publish_editor_state()
        end
    end,
    on_update = function(_delta_seconds)
        editor.update_world_session()
        local reset = false
        if not editor.editor_is_server then
            local rotate = editor.key_pressed("rotateBlueprint")
            local undo = editor.key_pressed("undoVoxels")
            local mark = editor.key_pressed("markCursorNext")
            reset = editor.key_pressed("resetEditor")
            local paste = editor.key_pressed("pasteAtCursor")
            local capture = editor.key_pressed("captureAndSave")
            if reset then
                editor.reset_editor()
            else
                if rotate then editor.rotate_blueprint() end
                if undo then editor.undo_voxels() end
                if mark then editor.mark_cursor_next() end
                if paste then editor.paste_voxels(true) end
                if capture then editor.capture_and_save() end
            end
        end
        editor.update_pending_queries(_delta_seconds)
        if editor.p2p_bridge then
            editor.p2p_bridge.tick(_delta_seconds)
        end
        if editor.editor_is_server then
            editor.update_server_remote_paste(_delta_seconds)
            editor.update_server_remote_undo(_delta_seconds)
            editor.update_server_remote_capture(_delta_seconds)
            return
        end
        editor.p2p_target_check_elapsed = editor.p2p_target_check_elapsed + math.max(0, tonumber(_delta_seconds) or 0)
        if editor.p2p_target_check_elapsed >= 1 then
            editor.p2p_target_check_elapsed = 0
            editor.log_p2p_target_if_changed()
        end
        editor.native_readiness_elapsed = editor.native_readiness_elapsed + math.max(0, tonumber(_delta_seconds) or 0)
        if editor.native_readiness_elapsed >= 2 and editor.process_progress.state ~= "running" and
           not editor.pending_world_action and not editor.pending_cursor_action and not editor.pending_prop_capture then
            editor.native_readiness_elapsed = 0
            local missing, permanently_unavailable = {}, false
            local required_features = {"runtime.world.entity.query_props_in_bounds", "runtime.world.cursor.get",
                "runtime.world.entity.spawn", "runtime.world.voxel.read", "runtime.world.voxel.write",
                "runtime.world.entity.destroy"}
            for _, prop in ipairs(editor.clipboard and editor.clipboard.props or {}) do
                if not editor.prop_scale_is_unit({prop.sx, prop.sy, prop.sz}) then
                    required_features[#required_features + 1] = "runtime.world.entity.set_scale"
                    break
                end
            end
            for _, feature in ipairs(required_features) do
                if not runtime.has(feature) then
                    local status = runtime.status(feature)
                    local reason = status and status.reason or "native world readiness is being refreshed"
                    if not (status and status.available) and not editor.world_feature_pending(reason) then
                        permanently_unavailable = true
                    end
                    missing[#missing + 1] = feature .. ": " .. tostring(reason)
                end
            end
            editor.readiness_logged = #missing == 0
            editor.readiness_wait_logged = not editor.readiness_logged
            editor.native_readiness_state = editor.readiness_logged and "ready" or
                (permanently_unavailable and "unavailable" or "waiting")
            editor.native_readiness_reason = editor.readiness_logged and "Native world APIs are ready." or table.concat(missing, ", ")
            local readiness_log_state = tostring(editor.current_session()) .. "|" .. editor.native_readiness_state .. "|" .. editor.native_readiness_reason
            if readiness_log_state ~= editor.last_readiness_log_state then
                editor.last_readiness_log_state = readiness_log_state
                if editor.readiness_logged then
                    shroudforge.log.info(editor.format_keybind_hint("World Editor native entity APIs ready: {{keybind:rotateBlueprint}} rotate, {{keybind:undoVoxels}} undo, {{keybind:markCursorNext}} mark A/B, {{keybind:resetEditor}} reset, {{keybind:pasteAtCursor}} paste, {{keybind:captureAndSave}} capture/save."))
                else
                    shroudforge.log.debug("World Editor runtime readiness " .. editor.native_readiness_state .. ": " .. editor.native_readiness_reason)
                end
            end
        end
        local backend = editor.execution_backend()
        if backend ~= editor.last_execution_backend then
            editor.last_execution_backend = backend
            editor.publish_editor_state()
            if backend == "direct" then
                shroudforge.log.info("World Editor automatic target selected: local native world")
            elseif backend == "p2p" then
                shroudforge.log.info("World Editor automatic target selected: dedicated server via Steam P2P")
            else
                shroudforge.log.debug("World Editor automatic target is waiting for a validated world session")
            end
        end
        if reset then return end
        editor.publish_editor_state()
    end,
    on_unload = function()
        if editor.save_progress.state == "saving" then
            editor.save_step(editor.save_progress.completed, "Save cancelled", "error")
        end
        editor.pending_cursor_action, editor.pending_prop_capture, editor.pending_world_action = nil, nil, nil
        editor.live_prop_cache = {}
        shroudforge.log.debug("World Editor Lua runtime unloaded")
    end,
}
