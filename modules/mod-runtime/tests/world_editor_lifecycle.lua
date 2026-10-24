-- Exercise shipped Lua code with in-memory exports and controlled world outcomes.
-- These checks do not establish that a native operation works in Enshrouded.
local function equal(actual, expected, label)
    assert(actual == expected, label .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
end

local function near(actual, expected, label)
    assert(math.abs(actual - expected) < 1e-12, label .. ": expected " .. expected .. ", got " .. actual)
end

local function sequence(actual, expected, label)
    equal(#actual, #expected, label .. " length")
    for index, value in ipairs(expected) do equal(actual[index], value, label .. "[" .. index .. "]") end
end

-- Independent snapshot of the retained setting values, including manual fallback
-- coordinates. The fixture reads the actual shipped extended.mod.json values.
local expected_settings = {
    rotateBlueprint="SDL:60", undoVoxels="SDL:61", markCursorNext="SDL:62",
    resetEditor="SDL:63", pasteAtCursor="SDL:64", captureAndSave="SDL:65",
    panelWidth=1180, panelHeight=190, panelPositionX=-1, panelPositionY=92,
    maximumCopyableProps=60000, sourceX=0, sourceY=0, sourceZ=0,
    sizeX=8, sizeY=8, sizeZ=8, blueprintName="my_blueprint", blueprintNewName="my_blueprint_copy",
    pasteVoxelMode="replace", targetPropMode="keep", targetX=0, targetY=0, targetZ=0,
    targetWorldX=0, targetWorldY=0, targetWorldZ=0, rotationQuarterTurns=0, rotationAxis="y",
    entityHandle=0, templateUuidHigh="0000000000000000", templateUuidLow="0000000000000000",
    trackingId=0, feedbackId=0, entityX=0, entityY=0, entityZ=0,
    boundsMinX=0, boundsMinY=0, boundsMinZ=0, boundsMaxX=0, boundsMaxY=0, boundsMaxZ=0,
}
for name, value in pairs(expected_settings) do equal(world_editor_settings[name], value, "setting " .. name) end
for name in pairs(world_editor_settings) do assert(expected_settings[name] ~= nil, "unexpected setting " .. name) end

-- A literal V7 file is the compatibility oracle. The UUID halves exceed exact
-- double precision, and nonuniform scale must survive without numeric UUID casts.
local blueprint_file = table.concat({
    "SHROUDFORGE_WORLD_BLUEPRINT_V7", "y", "1,0.5,1.5", "voxel", "2,1,3",
    "-0.5,0,0.5", "0.5,0.5,0.5", "10,20,0,40,0,60", "2,2,1,2,0,2", "1",
    "42,0.25,0.5,0.75,0,0,0,1,0.5,1.25,2,fedcba9876543210,0123456789abcdef",
}, "\n")

local unavailable = {
    ["runtime.world.voxel.read"]=true, ["runtime.world.voxel.write"]=true,
    ["runtime.world.entity.spawn"]=true, ["runtime.world.entity.destroy"]=true,
    ["runtime.world.entity.place"]=true, ["runtime.world.entity.finish_building"]=true,
}

local function fixture(context, session, deny_unverified_features, is_server)
    local f = {context=context, session=session, settings={}, actions={}, files={}, native_calls=0, sends=0, effects={}}
    for name, value in pairs(world_editor_settings) do f.settings[name] = value end
    f.files["world-editor/blueprints/fixture.sfbp"] = blueprint_file
    local function forbidden(operation)
        return function()
            f.native_calls = f.native_calls + 1
            error("blocked native operation was invoked: " .. operation)
        end
    end
    local environment = setmetatable({}, {__index=_G})
    environment.io = {
        export=function(path, contents) f.files[path] = contents end,
        export_list=function(path)
            equal(path, "world-editor/blueprints", "library export directory")
            return {"world-editor/blueprints/fixture.sfbp"}
        end,
        read_export_to_string=function(path) return assert(f.files[path], "export does not exist: " .. path) end,
        export_exists=function(path) return f.files[path] ~= nil end,
    }
    local function ignore_log() end
    environment.shroudforge = {
        settings={get=function(name) return f.settings[name] end},
        ui={on_action=function(name, action) f.actions[name] = action end},
        input={is_keybind_down=function() return false end,
            was_keybind_pressed=function() return false end,
            keybind_label=function(key) return f.settings[key] ~= "" and "Fixture shortcut" or nil end},
        log={debug=ignore_log, info=ignore_log, warn=ignore_log, error=ignore_log, trace=ignore_log},
    }
    environment.game = {assets={get_loaded_resources_by_type=function(name)
        equal(name, "keen::ItemInfo", "placement recipe resource")
        return {{data={itemId=42, equipment={
            placedEntity="01234567-89ab-cdef-0123-456789abcdef",
            placementAABBmin={x=0, y=0, z=0}, placementAABBmax={x=1, y=1, z=1},
            placementColliders={{dataArray={{value={materialFeedbackId=7}}}}},
        }}}}
    end}}
    environment.runtime = {
        is_server=is_server == true,
        require=function(name) equal(name, "runtime.lifecycle", "required runtime feature") end,
        has=function(name) return not (deny_unverified_features and unavailable[name]) end,
        status=function() return {reason="native operation is not verified for Enshrouded 1.0"} end,
        report_effect=function(state, detail)
            assert(state ~= "write-confirmed", "a blocked action must not report a successful world change")
            f.effects[#f.effects + 1] = {state=state, detail=detail}
        end,
        network={
            status=function() return {available=true, configured_server_steam_id="76561198000000001"} end,
            send_mod=function() f.sends = f.sends + 1; error("blocked action sent a P2P request") end,
            receive_mod=function() return {} end,
        },
        world={
            session_id=function() return f.session end,
            context_kind=function() return f.context end,
            voxel={read=forbidden("voxel.read"), write=forbidden("voxel.write"),
                get_grid_spec=forbidden("voxel.get_grid_spec")},
            cursor={get=forbidden("cursor.get")},
            entity={
                spawn=forbidden("entity.spawn"), destroy=forbidden("entity.destroy"),
                place=forbidden("entity.place"), finish_building=forbidden("entity.finish_building"),
                get_transform=forbidden("entity.get_transform"), set_scale=forbidden("entity.set_scale"),
                query_props=forbidden("entity.query_props"), query_props_in_bounds=forbidden("entity.query_props_in_bounds"),
                register_prop_recipes=function(recipes)
                    equal(#recipes, 1, "registered recipe count")
                    equal(recipes[1].itemId, 42, "registered recipe item")
                    sequence(recipes[1].bounds, {0, 0, 0, 1, 1, 1}, "registered placement bounds")
                    equal(recipes[1].feedback, 7, "registered material feedback")
                    return true
                end,
            },
        },
    }
    f.runtime, f.io = environment.runtime, environment.io
    local loaded = {}
    environment.require = function(name)
        if loaded[name] then return loaded[name] end
        local source = assert(world_editor_sources[name], "unexpected module " .. name)
        local module = assert(load(source, "world-editor/" .. name, "t", environment))()
        if name == "blueprint" then
            -- Capture the real shared editor state at its existing module boundary.
            local initialize = module
            module = function(editor) f.editor = editor; initialize(editor) end
        end
        loaded[name] = module
        return module
    end
    f.mod = assert(load(world_editor_sources.mod, "world-editor/mod", "t", environment))()
    equal(f.mod.update_interval_ms, 100, "World Editor callback interval")
    f.mod.on_load()
    if is_server then f.editor.load_blueprint("fixture") else f.actions.selectBlueprint("fixture") end
    equal(f.editor.active_blueprint_name, "fixture", "blueprint loaded through UI action")
    assert(f.editor.clipboard, "the literal V7 blueprint must be accepted")
    for name, value in pairs(world_editor_settings) do equal(f.editor.setting(name), value, "editor setting " .. name) end
    return f
end

local f = fixture("direct", 81, true)
local captured = f.editor.clipboard
sequence(captured.cells, {10, 20, 0, 40, 0, 60}, "decoded cells")
sequence(captured.coverage, {2, 2, 1, 2, 0, 2}, "decoded coverage")
sequence(captured.extent, {1, 0.5, 1.5}, "decoded extent")
sequence(captured.voxelOffset, {-0.5, 0, 0.5}, "decoded voxel offset")
sequence(captured.cellSize, {0.5, 0.5, 0.5}, "decoded cell size")
local prop = captured.props[1]
equal(#captured.props, 1, "decoded prop count")
equal(prop.itemId, 42, "decoded prop item")
equal(prop.templateUuidHighHex, "fedcba9876543210", "UUID high")
equal(prop.templateUuidLowHex, "0123456789abcdef", "UUID low including leading zero")
sequence({prop.sx, prop.sy, prop.sz}, {0.5, 1.25, 2}, "decoded prop scale")
equal(f.editor.encode_blueprint(captured), blueprint_file, "V7 wire compatibility")
f.actions.saveBlueprint()
equal(f.files["world-editor/blueprints/my_blueprint.sfbp"], blueprint_file, "UI save preserves V7 content")

-- Seen from above, +90 degrees around Y maps the three original X=1 cells
-- into the first new Z row, followed by the three original X=0 cells.
f.actions.rotateBlueprint()
equal(f.editor.rotation_turns(), 1, "rotate action advances one quarter turn")
local rotated = f.editor.rotated_blueprint(captured, f.editor.rotation_turns())
sequence({rotated.sx, rotated.sy, rotated.sz}, {3, 1, 2}, "rotated dimensions")
sequence(rotated.cells, {20, 40, 60, 10, 0, 0}, "90 degree cell order")
sequence(rotated.coverage, {2, 2, 2, 2, 1, 0}, "90 degree coverage order")
sequence(rotated.extent, {1.5, 0.5, 1}, "rotated extent")
sequence(rotated.voxelOffset, {0.5, 0, -0.5}, "rotated voxel offset")
sequence(rotated.boundsMin, {0, 0, -1}, "rotated minimum bounds")
sequence(rotated.boundsMax, {1.5, 0.5, 0}, "rotated maximum bounds")
local rotated_prop = rotated.props[1]
sequence({rotated_prop.x, rotated_prop.y, rotated_prop.z}, {0.75, 0.5, -0.25}, "rotated prop position")
near(rotated_prop.qy, math.sqrt(0.5), "rotated prop quaternion Y")
near(rotated_prop.qw, math.sqrt(0.5), "rotated prop quaternion W")
sequence({rotated_prop.sx, rotated_prop.sy, rotated_prop.sz}, {0.5, 1.25, 2}, "rotation preserves prop scale")
equal(rotated_prop.templateUuidHighHex, prop.templateUuidHighHex, "rotation preserves UUID high")
equal(rotated_prop.templateUuidLowHex, prop.templateUuidLowHex, "rotation preserves UUID low")
equal(f.editor.encode_blueprint(captured), blueprint_file, "rotation does not alter the captured blueprint")

local mutation_actions = {"pasteVoxels", "pasteAtCursor", "spawnEntity", "placeEntity",
    "destroyEntity", "destroySelectedProp", "undoVoxels"}
local function expect_blocked(f, label)
    f.settings.trackingId, f.settings.feedbackId, f.settings.entityHandle = 42, 7, 91
    -- A retained journal makes F4 exercise the gate before any restoration.
    local journal = {session=81, hasVoxels=true, voxel_written=true,
        region={x=0, y=0, z=0, sx=2, sy=1, sz=3}, cells={0, 0, 0, 0, 0, 0},
        expected_cells={10, 20, 0, 40, 0, 60}, entities={}, removed_props={}}
    f.editor.undo_state = journal
    for _, action in ipairs(mutation_actions) do
        assert(f.actions[action], "missing UI action " .. action)()
        equal(f.native_calls, 0, label .. " blocks native operations for " .. action)
        equal(f.sends, 0, label .. " sends no P2P request for " .. action)
        equal(f.editor.p2p_bridge.pending(), false, label .. " queues no P2P request for " .. action)
        equal(f.editor.undo_state, journal, label .. " retains the undo journal for " .. action)
    end
end

expect_blocked(f, "local context with unavailable write features")

-- has() and status() are separate nonblocking Scene observations. A ready
-- second sample must retain a bounded continuation, without invoking a writer
-- until has() itself becomes ready. A permanently missing API still stops.
local missing_writer = fixture("direct", 81, false)
missing_writer.runtime.has = function() return false end
missing_writer.runtime.status = function() return {available=true} end
missing_writer.actions.pasteVoxels()
assert(missing_writer.editor.pending_world_action, "writer readiness race starts a bounded retry")
assert(missing_writer.editor.editor_hint:find("continue automatically", 1, true),
    "writer readiness race explains automatic continuation")
equal(missing_writer.native_calls, 0, "missing voxel writer performs no native write")
missing_writer.mod.on_update(15)
equal(missing_writer.editor.pending_world_action, nil, "unresolved writer readiness race times out")
equal(missing_writer.native_calls, 0, "missing voxel writer cannot retry into a mutation")
missing_writer.mod.on_unload()

local missing_spawn = fixture("direct", 81, false)
missing_spawn.runtime.has = function(name) return name ~= "runtime.world.entity.spawn" end
missing_spawn.runtime.status = function() return {available=true} end
equal(missing_spawn.editor.feature("runtime.world.entity.spawn"), false,
    "missing spawn command rejects stale status availability")
assert(missing_spawn.editor.editor_hint:find("Waiting for runtime.world.entity.spawn", 1, true),
    "spawn readiness race is reported as waiting")
equal(missing_spawn.native_calls, 0, "missing prop spawn invokes no native operation")
missing_spawn.mod.on_unload()

local missing_undo_writer = fixture("direct", 81, false)
missing_undo_writer.runtime.has = function(name) return name ~= "runtime.world.voxel.write" end
missing_undo_writer.runtime.status = function() return {available=true} end
local retained_journal = {session=81, hasVoxels=true, voxel_written=true,
    region={x=0, y=0, z=0, sx=2, sy=1, sz=3}, cells={0, 0, 0, 0, 0, 0},
    expected_cells={10, 20, 0, 40, 0, 60}, entities={}, removed_props={}}
missing_undo_writer.editor.undo_state = retained_journal
missing_undo_writer.actions.undoVoxels()
assert(missing_undo_writer.editor.pending_world_action, "undo writer readiness race starts a bounded retry")
equal(missing_undo_writer.editor.undo_state, retained_journal, "missing undo writer retains recovery journal")
equal(missing_undo_writer.editor.process_progress.state, "running", "undo writer readiness race remains pending")
equal(missing_undo_writer.native_calls, 0, "missing undo writer performs no mutation")
missing_undo_writer.mod.on_update(15)
equal(missing_undo_writer.editor.pending_world_action, nil, "undo writer readiness race times out")
equal(missing_undo_writer.editor.undo_state, retained_journal, "undo timeout retains recovery journal")
equal(missing_undo_writer.editor.process_progress.state, "error", "undo readiness timeout ends with an error")
missing_undo_writer.mod.on_unload()

-- Props-only paste must also respect the unavailable native spawn feature.
f.editor.clipboard = {region={rotationAxis="y"}, extent={1, 1, 1}, hasVoxels=false, props={prop}}
f.actions.pasteVoxels()
equal(f.native_calls, 0, "props-only paste does not invoke unavailable native operations")
equal(f.editor.process_progress.state, "error", "blocked prop placement is not reported complete")
f.mod.on_update(0.1)
equal(f.native_calls, 0, "update does not retry unverified operations")
equal(f.sends, 0, "update sends no local placement to P2P")
f.mod.on_unload()

-- Unknown context must stop before native feature readiness, even when every
-- feature claims availability. A zero generation must do the same for 'direct'.
expect_blocked(fixture("unknown", 81, false), "unknown world context")
expect_blocked(fixture("direct", 0, false), "unavailable world generation")

-- A transient transform failure must retain the exact journal and must never
-- invoke destruction or restoration. Only an explicit membership absence can
-- complete the removal side of undo.
local pending = fixture("direct", 81, false)
local pending_journal = {session=81, hasVoxels=false, voxel_written=false,
    entities={{entityHandle=91}}, entity_index=1, removed_props={}, removed_prop_index=0, recovery_required=false}
pending.editor.undo_state = pending_journal
pending.runtime.world.entity.get_transform = function() return nil, "entity-state-unavailable-retry" end
pending.actions.undoVoxels()
equal(pending.native_calls, 0, "unreadable prop cannot trigger a mutation")
equal(pending.editor.undo_state, pending_journal, "unreadable prop retains undo journal")
equal(pending.editor.undo_state.entity_index, 1, "unreadable prop retains undo progress")
equal(pending.editor.undo_state.recovery_required, false, "read-only prop preflight does not lock another placement")
pending.runtime.world.entity.get_transform = function() return nil, "entity-absent" end
pending.runtime.report_effect = function(state, detail)
    pending.effects[#pending.effects + 1] = {state=state, detail=detail}
end
pending.actions.undoVoxels()
equal(pending.native_calls, 0, "confirmed absent prop does not trigger another mutation")
equal(pending.editor.undo_state, nil, "confirmed absent prop completes props-only undo")

local replaced = fixture("direct", 81, false)
local original = replaced.editor.intended_spawn_transform({1, 2, 3}, {0, 0, 0, 1}, {1, 1, 1}, 42)
local replaced_journal = {session=81, hasVoxels=false, voxel_written=false,
    entities={}, entity_index=0, removed_props={{entityHandle=91, removal_pending=true,
        expected_transform=original}}, removed_prop_index=1}
replaced.editor.undo_state = replaced_journal
replaced.runtime.world.entity.get_transform = function() return nil, "entity-state-unavailable-retry" end
replaced.actions.undoVoxels()
equal(replaced.editor.undo_state, replaced_journal, "unconfirmed target removal keeps its restore record")
equal(replaced.native_calls, 0, "unconfirmed target removal cannot spawn a duplicate")
replaced.runtime.world.entity.get_transform = function() return original end
replaced.runtime.report_effect = function() end
replaced.actions.undoVoxels()
equal(replaced.native_calls, 0, "still-present original is not restored twice")
equal(replaced.editor.undo_state, nil, "verified original completes pending-removal recovery")

-- Reusing a preview must preserve the exact world bounds for target replacement,
-- including the voxel channel that extends beyond the captured world-space box.
do
    local preview = fixture("direct", 81, false)
    preview.settings.targetPropMode = "replace"
    preview.runtime.world.cursor.get = function()
        return {value={primaryTransform={position={x=10*4294967296, y=20*4294967296, z=30*4294967296}}}}
    end
    preview.runtime.world.voxel.get_grid_spec = function()
        return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
    end
    preview.runtime.world.voxel.read = function() return {0,0,0,0,0,0} end
    local queries = 0
    preview.runtime.world.entity.query_props_in_bounds = function(bounds)
        queries = queries + 1
        sequence(bounds, {9.5,20,30,11,20.5,32}, "preview replacement query bounds")
        return nil, "fixture stops after the replacement preflight query"
    end
    preview.actions.previewPaste()
    preview.actions.pasteAtCursor()
    equal(queries, 1, "F7 from preview reaches the replacement query")
    equal(preview.editor.undo_state, nil, "replacement preflight failure creates no undo journal")
    equal(preview.native_calls, 0, "replacement preflight failure performs no mutation")
end

-- After the first actual undo mutation, F7 must retain the incomplete journal.
-- A terrain-only preflight conflict has made no changes and must not set that lock.
do
    local partial = fixture("direct", 81, false)
    local original = partial.editor.intended_spawn_transform({1,2,3}, {0,0,0,1}, {1,1,1}, 42)
    local journal = {session=81, hasVoxels=true, voxel_written=true, recovery_required=false,
        region={x=0,y=0,z=0,sx=2,sy=1,sz=3}, cells={0,0,0,0,0,0},
        expected_cells={10,20,0,40,0,60}, entities={{entityHandle=91, expected_transform=original}},
        entity_index=1, removed_props={}, removed_prop_index=0}
    partial.editor.undo_state = journal
    local current_cells, writes = {999,20,0,40,0,60}, 0
    partial.runtime.world.voxel.read = function() return current_cells end
    partial.runtime.world.voxel.write = function(_, _, _, _, _, _, cells)
        equal(journal.recovery_required, true, "terrain undo locks the journal before writing")
        writes = writes + 1
        current_cells = cells
        return true
    end
    partial.runtime.world.entity.get_transform = function() return nil, "entity-state-unavailable-retry" end
    partial.actions.undoVoxels()
    equal(writes, 0, "terrain conflict stops before mutation")
    equal(journal.recovery_required, false, "terrain conflict leaves the completed placement journal unlocked")
    current_cells = journal.expected_cells
    partial.actions.undoVoxels()
    equal(writes, 1, "terrain restoration completes before the prop read pauses undo")
    equal(journal.voxel_written, false, "completed terrain restoration remains recorded")
    partial.actions.pasteAtCursor()
    equal(partial.editor.undo_state, journal, "F7 retains the partial terrain undo journal")
    equal(writes, 1, "blocked F7 performs no second terrain write")
    equal(partial.native_calls, 0, "blocked F7 performs no cursor lookup or other mutation")
    partial.runtime.world.entity.get_transform = function() return nil, "entity-absent" end
    partial.runtime.report_effect = function() end
    partial.actions.undoVoxels()
    equal(partial.editor.undo_state, nil, "retry completes partial terrain undo")
    equal(writes, 1, "retry preserves the already completed terrain restoration")
end

-- Props-only undo removes exact handles in reverse order and resumes from the
-- first unfinished handle, without relying on the game's limited undo groups.
do
    local partial = fixture("direct", 81, false)
    local original = partial.editor.intended_spawn_transform({1,2,3}, {0,0,0,1}, {1,1,1}, 42)
    local journal = {session=81, hasVoxels=false, voxel_written=false, recovery_required=false,
        entities={{entityHandle=91, expected_transform=original}, {entityHandle=92, expected_transform=original}},
        entity_index=2, removed_props={}, removed_prop_index=0}
    partial.editor.undo_state = journal
    local live, calls, retry = {[91]=true, [92]=true}, {}, false
    partial.runtime.world.entity.get_transform = function(handle)
        if live[handle] then return original end
        return nil, "entity-absent"
    end
    partial.runtime.world.entity.destroy = function(handle)
        equal(journal.recovery_required, true, "prop undo locks the journal before destruction")
        calls[#calls + 1] = handle
        if handle == 91 and not retry then return false, "fixture removal paused" end
        live[handle] = nil
        return true
    end
    partial.runtime.report_effect = function() end
    partial.actions.undoVoxels()
    sequence(calls, {92,91}, "first props-only undo calls")
    equal(journal.entity_index, 1, "partial prop undo retains the first unfinished handle")
    partial.actions.pasteAtCursor()
    equal(partial.editor.undo_state, journal, "F7 retains the partial prop undo journal")
    sequence(calls, {92,91}, "blocked F7 sends no additional deletion")
    equal(partial.native_calls, 0, "blocked F7 performs no cursor lookup or other mutation")
    retry = true
    partial.actions.undoVoxels()
    sequence(calls, {92,91,91}, "retry does not delete the completed handle again")
    equal(partial.editor.undo_state, nil, "retry completes props-only undo")
end

-- A replacement restoration can pause after spawn while scale is unavailable.
-- Retain both its journal lock and returned handle so retry cannot spawn a duplicate.
do
    local partial = fixture("direct", 81, false)
    local original = partial.editor.intended_spawn_transform({1,2,3}, {0,0,0,1}, {2,1,1}, 42)
    local journal = {session=81, hasVoxels=false, voxel_written=false, recovery_required=false,
        entities={}, entity_index=0, removed_props={{entityHandle=91, expected_transform=original,
            recipe={id=42, uuidHigh="fedcba9876543210", uuidLow="0123456789abcdef"},
            position={1,2,3}, rotation={0,0,0,1}, scale={2,1,1}}}, removed_prop_index=1}
    partial.editor.undo_state = journal
    local spawns, scales = 0, 0
    partial.runtime.world.entity.get_transform = function(handle)
        if handle == 92 then return original end
        return nil, "entity-absent"
    end
    partial.runtime.world.entity.spawn = function()
        equal(journal.recovery_required, true, "replacement undo locks the journal before spawn")
        spawns = spawns + 1
        return 92
    end
    partial.runtime.world.entity.set_scale = function(handle)
        equal(journal.recovery_required, true, "replacement undo locks the journal before scale")
        equal(handle, 92, "replacement scale uses the retained restore handle")
        scales = scales + 1
        if scales == 1 then return false, "fixture scale paused" end
        return true
    end
    partial.runtime.report_effect = function() end
    partial.actions.undoVoxels()
    equal(journal.removed_props[1].restoreHandle, 92, "partial restoration retains its new handle")
    partial.actions.pasteAtCursor()
    equal(partial.editor.undo_state, journal, "F7 retains the replacement recovery journal")
    equal(partial.native_calls, 0, "blocked F7 performs no cursor lookup or other mutation")
    partial.actions.undoVoxels()
    equal(spawns, 1, "replacement retry does not spawn a duplicate")
    equal(scales, 2, "replacement retry resumes scale restoration")
    equal(partial.editor.undo_state, nil, "retry completes replaced-prop restoration")
end

-- Unit-scale props use the original spawn transform without requesting a
-- separate tracked CurrentTransform writer. Its readback still checks scale.
-- The real headless server entrypoints must use that same placement/undo flow.
for _, is_server in ipairs({false, true}) do
    local placement = fixture("direct", 81, false, is_server)
    local prop = placement.editor.clipboard.props[1]
    prop.sx, prop.sy, prop.sz = 1, 1, 1
    placement.runtime.has = function(name) return name ~= "runtime.world.entity.set_scale" end
    placement.runtime.status = function() return {available=false, reason="world operation is not ready"} end
    local grid_reads = 0
    placement.runtime.world.voxel.get_grid_spec = function()
        grid_reads=grid_reads+1
        return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
    end
    local cells, live, spawns, destroys, reads = {0,0,0,0,0,0}, nil, 0, 0, 0
    placement.runtime.world.voxel.read = function()
        reads=reads+1
        if reads==1 then return nil,"voxel-read-pending" end
        return cells
    end
    placement.runtime.world.voxel.write = function(_,_,_,_,_,_,values) cells=values;return true end
    placement.runtime.world.entity.spawn = function(high, low, position, rotation, item)
        spawns=spawns+1
        live=placement.editor.intended_spawn_transform(position,rotation,{1,1,1},item,high,low)
        live.entityId=1001
        return 92
    end
    placement.runtime.world.entity.get_transform = function(handle)
        equal(handle,92,"unit prop uses exact native handle")
        if live then return live end
        return nil,"entity-absent"
    end
    placement.runtime.world.entity.set_scale = function() error("unit prop must not request a scale write") end
    placement.runtime.world.entity.destroy = function(handle)
        equal(handle,92,"unit undo deletes exact native handle")
        destroys=destroys+1;live=nil;return true
    end
    placement.runtime.report_effect = function() end
    local paste_result, undo_result
    if is_server then
        placement.editor.server_begin_remote_paste("76561198000000002", "fixture-server-paste",
            placement.editor.encode_blueprint(placement.editor.clipboard),
            {anchor={0,0,0}, turns=0, voxelMode="replace", targetPropMode="keep"},
            function(ok, transaction, reason) paste_result={ok=ok,transaction=transaction,reason=reason} end,
            function() end)
        equal(grid_reads,0,"server defers grid validation to canonical placement")
        placement.mod.on_update(0.1)
    else
        placement.actions.pasteVoxels()
    end
    equal(spawns,0,"pending target snapshot performs no spawn")
    equal(placement.editor.undo_state,nil,"pending target snapshot creates no journal")
    placement.runtime.has = function(name) return name=="runtime.world.voxel.read" end
    placement.mod.on_update(0.1)
    equal(spawns,1,"unit F7 spawns once without a scale writer")
    equal(placement.editor.process_progress.state,"complete","unit F7 completes")
    if is_server then
        assert(paste_result and paste_result.ok,"server confirms canonical placement")
        assert(paste_result.transaction,"server returns its undo transaction")
    end
    placement.runtime.has = function(name) return name~="runtime.world.entity.set_scale" end
    if is_server then
        local original_undo = placement.editor.undo_voxels
        local undo_entries = 0
        placement.editor.undo_voxels = function() undo_entries=undo_entries+1;original_undo() end
        placement.editor.server_begin_remote_undo("76561198000000002", "fixture-server-undo", paste_result.transaction,
            function(ok, reason) undo_result={ok=ok,reason=reason} end,function() end)
        -- Independent observations need not be ready together. The wrapper
        -- must enter canonical F4 and retain its guarded continuation.
        placement.runtime.has = function() return false end
        placement.mod.on_update(0.1)
        equal(undo_entries,1,"server enters canonical undo without duplicate availability samples")
        assert(placement.editor.pending_world_action,"server retains the canonical undo continuation")
        placement.runtime.has = function(name) return name~="runtime.world.entity.set_scale" end
        placement.mod.on_update(0.1)
        assert(undo_result and undo_result.ok,"server confirms canonical undo")
    else
        placement.actions.undoVoxels()
    end
    equal(destroys,1,"unit F4 destroys once without a scale writer")
    equal(placement.editor.undo_state,nil,"unit F4 completes the journal")
    sequence(cells,{0,0,0,0,0,0},"unit F4 restores terrain")
end

-- The full F7/F4 readiness announcement must wait for each mutation it requires.
for _, missing in ipairs({"runtime.world.voxel.write", "runtime.world.entity.destroy", "runtime.world.entity.set_scale"}) do
    local readiness = fixture("direct", 81, false)
    readiness.runtime.has = function(name) return name ~= missing end
    readiness.mod.on_update(1)
    equal(readiness.editor.last_readiness_signature, nil, "readiness is not sampled before two seconds")
    readiness.mod.on_update(1)
    equal(readiness.editor.readiness_logged, false, "full readiness waits for " .. missing)
    readiness.runtime.has = function() return true end
    readiness.mod.on_update(2)
    equal(readiness.editor.readiness_logged, true, "full readiness is announced once " .. missing .. " is ready")
    equal(readiness.editor.native_readiness_state, "ready", "readiness publishes the current ready state")
    readiness.runtime.has = function(name) return name ~= missing end
    readiness.runtime.status = function() return {available=false, reason="KFC Runtime world operation is not ready"} end
    readiness.mod.on_update(2)
    equal(readiness.editor.readiness_logged, false, "readiness is refreshed after the first success")
    equal(readiness.editor.native_readiness_state, "waiting", "a busy Scene is a waiting status")
    assert(readiness.files["world-editor/editor-state.txt"]:find("runtimeReadiness=waiting\n", 1, true),
        "changed runtime readiness is exported")
    assert(readiness.files["world-editor/editor-state.txt"]:find(missing, 1, true),
        "changed runtime readiness identifies the waiting feature")
    readiness.runtime.has = function() return true end
    readiness.mod.on_update(2)
    equal(readiness.editor.native_readiness_state, "ready", "readiness can recover in the same session")
    equal(readiness.native_calls, 0, "readiness observation performs no native mutation")
end

-- F7 waits only before dispatch. Transient writer availability and an original
-- target-cache miss both resume the same blueprint without a false undo entry.
do
    local placement = fixture("direct", 81, false)
    placement.editor.clipboard.props = {}
    local writer_ready, reads, writes = false, 0, 0
    placement.runtime.has = function(name) return name ~= "runtime.world.voxel.write" or writer_ready end
    placement.runtime.status = function() return {available=false, reason="KFC Runtime world operation is not ready"} end
    placement.runtime.world.voxel.get_grid_spec = function()
        return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
    end
    placement.runtime.world.voxel.read = function()
        reads = reads + 1
        if reads == 1 then return nil, "voxel-read-pending" end
        return {0,0,0,0,0,0}
    end
    placement.runtime.world.voxel.write = function()
        writes = writes + 1
        return true
    end
    placement.runtime.report_effect = function() end
    placement.actions.pasteVoxels()
    assert(placement.editor.pending_world_action, "F7 queues a transient writer wait")
    equal(placement.editor.undo_state, nil, "writer wait creates no false undo journal")
    equal(reads, 0, "writer wait performs no terrain read")
    equal(writes, 0, "writer wait performs no terrain write")
    placement.mod.on_update(2)
    assert(placement.editor.pending_world_action, "F7 keeps waiting during a busy Scene sample")
    writer_ready = true
    placement.mod.on_update(2)
    assert(placement.editor.pending_world_action, "target-cache pending retains the same placement")
    equal(placement.editor.pending_world_action.elapsed, 4, "target-cache retry retains the overall preflight deadline")
    equal(placement.editor.undo_state, nil, "target snapshot wait creates no false undo journal")
    equal(writes, 0, "target snapshot wait makes no mutation")
    placement.mod.on_update(1)
    equal(writes, 1, "F7 writes exactly once after preflight becomes ready")
    equal(placement.editor.pending_world_action, nil, "successful placement clears its wait")
    equal(placement.editor.process_progress.state, "complete", "successful resumed F7 finishes")
    equal(placement.editor.undo_state.recovery_required, false, "successful resumed F7 has a completed journal")
end

-- A native call with uncertain effects is never repeated by the preflight
-- readiness continuation, including after later update ticks.
do
    local uncertain = fixture("direct", 81, false)
    uncertain.editor.clipboard.props = {}
    uncertain.runtime.world.voxel.get_grid_spec = function()
        return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
    end
    uncertain.runtime.world.voxel.read = function() return {0,0,0,0,0,0} end
    local writes = 0
    uncertain.runtime.world.voxel.write = function()
        writes = writes + 1
        return false, "native call outcome is uncertain"
    end
    -- Leave automatic recovery to F4. This check isolates placement dispatch.
    uncertain.editor.rollback_partial_paste = function() end
    uncertain.actions.pasteVoxels()
    equal(writes, 1, "uncertain write is attempted exactly once")
    equal(uncertain.editor.pending_world_action, nil, "uncertain write starts no preflight retry")
    equal(uncertain.editor.undo_state.recovery_required, true, "uncertain write retains its recovery journal")
    uncertain.mod.on_update(1)
    uncertain.actions.pasteVoxels()
    equal(writes, 1, "updates and a second F7 cannot repeat an uncertain write")
end

-- F8 can wait for a temporarily unreadable terrain context. Cancelling that
-- continuation must also close the public progress state and permit a new F8.
local function waiting_capture()
    local f = fixture("direct", 81, false)
    f.editor.selection_a, f.editor.selection_b = {x=0, y=0, z=0}, {x=1, y=1, z=1}
    f.runtime.has = function(name) return name ~= "runtime.world.voxel.read" end
    f.runtime.status = function() return {reason="world operation is not ready"} end
    f.actions.captureAndSave()
    assert(f.editor.pending_world_action, "F8 must retain its pending terrain continuation")
    equal(f.editor.process_progress.state, "running", "pending F8 progress")
    equal(f.editor.save_progress.state, "saving", "pending F8 save")
    return f
end

local function capture_stopped(f, label)
    equal(f.editor.pending_world_action, nil, label .. " clears the terrain retry")
    equal(f.editor.pending_cursor_action, nil, label .. " clears the cursor continuation")
    equal(f.editor.pending_prop_capture, nil, label .. " clears the prop continuation")
    equal(f.editor.process_progress.state, "error", label .. " ends operation progress")
    assert(f.editor.save_progress.state ~= "saving", label .. " releases the save busy state")
    assert(f.editor.editor_stage ~= "capturing", label .. " leaves the capture stage")
    assert(f.files["world-editor/editor-state.txt"]:find("processState=error\n", 1, true),
        label .. " publishes terminal progress")
    equal(f.files["world-editor/blueprints/capture-1.sfbp"], nil, label .. " saves no incomplete blueprint")
    equal(f.native_calls, 0, label .. " invokes no unavailable native operation")
end

local timed_out = waiting_capture()
local previous_blueprint = timed_out.editor.clipboard
timed_out.mod.on_update(15)
capture_stopped(timed_out, "terrain retry timeout")
equal(timed_out.editor.clipboard, previous_blueprint, "timeout retains the previous complete blueprint")
local terminal_sequence = timed_out.editor.process_progress.sequence
timed_out.mod.on_update(410)
capture_stopped(timed_out, "later updates after timeout")
equal(timed_out.editor.process_progress.sequence, terminal_sequence, "cancelled F8 cannot restart itself")
timed_out.actions.captureAndSave()
assert(timed_out.editor.pending_world_action, "a new F8 is accepted after cancellation")
equal(timed_out.editor.process_progress.sequence, terminal_sequence + 1, "new F8 starts a new operation")
timed_out.actions.resetEditor()
capture_stopped(timed_out, "F6 during a terrain retry")

local missing_session = waiting_capture()
previous_blueprint = missing_session.editor.clipboard
missing_session.session = 0
missing_session.mod.on_update(0.1)
capture_stopped(missing_session, "world session unavailable")
equal(missing_session.editor.clipboard, previous_blueprint, "session loss retains the previous complete blueprint")

local denied_after_wait = waiting_capture()
denied_after_wait.runtime.status = function() return {reason="native operation is not verified"} end
denied_after_wait.mod.on_update(0.1)
capture_stopped(denied_after_wait, "terrain feature becomes unavailable")

local readiness_race = waiting_capture()
readiness_race.runtime.status = function() return {available=true} end
readiness_race.mod.on_update(0.1)
assert(readiness_race.editor.pending_world_action, "has/status readiness race retains a pending terrain capture")
readiness_race.mod.on_update(15)
capture_stopped(readiness_race, "terrain readiness race timeout")

local denied_capture = fixture("direct", 81, true)
denied_capture.editor.selection_a, denied_capture.editor.selection_b = {x=0, y=0, z=0}, {x=1, y=1, z=1}
denied_capture.actions.captureAndSave()
capture_stopped(denied_capture, "terrain feature is unavailable immediately")

local denied_export = fixture("direct", 81, false)
denied_export.editor.selection_a, denied_export.editor.selection_b = {x=0, y=0, z=0}, {x=1, y=1, z=1}
denied_export.io.export_exists = function() error("export unavailable") end
denied_export.actions.captureAndSave()
capture_stopped(denied_export, "blueprint export is unavailable")

local prop_capture = fixture("direct", 81, false)
local prop_queries = 0
prop_capture.editor.selection_a, prop_capture.editor.selection_b = {x=0, y=0, z=0}, {x=1, y=1, z=1}
prop_capture.runtime.world.entity.query_props_in_bounds = function()
    prop_queries = prop_queries + 1
    return nil, "still scanning"
end
prop_capture.actions.capturePropsOnly()
assert(prop_capture.editor.pending_prop_capture, "props-only capture retains its continuation")
local retained_journal = {session=81, recovery_required=true}
prop_capture.editor.undo_state = retained_journal
prop_capture.actions.resetEditor()
capture_stopped(prop_capture, "F6 during a prop scan")
equal(prop_capture.editor.undo_state, retained_journal, "capture reset retains previous placement recovery")
prop_capture.mod.on_update(410)
equal(prop_queries, 1, "cancelled prop capture never resumes its scan or save callback")
capture_stopped(prop_capture, "later updates after prop capture reset")

-- A bounded original cache request returns pending while the engine fills it.
-- F8 retries that same capture and preserves the overall timeout across retries.
local function cache_capture(complete_after, grid_ready_after, pending_reason)
    local f = fixture("direct", 81, false)
    f.editor.selection_a, f.editor.selection_b = {x=0, y=0, z=0}, {x=1, y=1, z=1}
    local reads, grid_reads = 0, 0
    f.runtime.world.voxel.get_grid_spec = function()
        grid_reads = grid_reads + 1
        if grid_reads < (grid_ready_after or 0) then return nil, "voxel-grid-pending" end
        return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
    end
    f.runtime.world.voxel.read = function(_, _, _, sx, sy, sz)
        reads = reads + 1
        if reads < complete_after then return nil, pending_reason or "voxel-read-pending" end
        local cells = {}
        for index=1,sx*sy*sz do cells[index]=256 end
        return cells
    end
    f.runtime.world.entity.query_props_in_bounds = function() return {} end
    f.actions.captureAndSave()
    return f, function() return reads end, function() return grid_reads end
end

local cache_ready, cache_reads = cache_capture(3)
assert(cache_ready.editor.pending_world_action, "cache-pending F8 retains its retry")
equal(cache_ready.files["world-editor/blueprints/capture-1.sfbp"], nil, "pending cache saves no partial blueprint")
cache_ready.mod.on_update(0.1)
assert(cache_ready.editor.pending_world_action, "second cache miss remains pending")
cache_ready.mod.on_update(0.1)
equal(cache_reads(), 3, "original cache is polled on later updates")
equal(cache_ready.editor.pending_world_action, nil, "completed cache clears retry")
equal(cache_ready.editor.process_progress.state, "complete", "complete cache finishes F8")
assert(cache_ready.files["world-editor/blueprints/capture-1.sfbp"], "complete cache saves the V7 blueprint")

local compatible_cache = cache_capture(2, nil, "voxel read is waiting for the Scene gate or original cache readiness")
assert(compatible_cache.editor.pending_world_action, "installed ABI-12 bridge pending remains compatible")
compatible_cache.mod.on_update(0.1)
equal(compatible_cache.editor.process_progress.state, "complete", "installed bridge pending completes F8")

local cache_timeout, timeout_reads = cache_capture(math.huge)
cache_timeout.mod.on_update(5)
equal(cache_timeout.editor.pending_world_action.elapsed, 5, "first cache retry preserves elapsed")
cache_timeout.mod.on_update(5)
equal(cache_timeout.editor.pending_world_action.elapsed, 10, "later cache retry preserves elapsed")
cache_timeout.mod.on_update(5)
capture_stopped(cache_timeout, "original cache timeout")
equal(timeout_reads(), 3, "timeout stops rather than resetting after each cache retry")

-- Grid metadata uses the same session-bound reader, so a busy Scene must wait
-- before coordinates or cells are consumed, with one overall F8 deadline.
local grid_ready, grid_cell_reads, grid_reads = cache_capture(1, 3)
assert(grid_ready.editor.pending_world_action, "pending grid metadata retains F8")
equal(grid_cell_reads(), 0, "pending metadata cannot read terrain with guessed coordinates")
equal(grid_ready.files["world-editor/blueprints/capture-1.sfbp"], nil, "pending grid saves no partial blueprint")
grid_ready.mod.on_update(0.1)
equal(grid_cell_reads(), 0, "second metadata miss still performs no terrain read")
grid_ready.mod.on_update(0.1)
equal(grid_reads(), 3, "grid is retried on later updates and then cached")
equal(grid_cell_reads(), 1, "terrain is read once after metadata becomes ready")
equal(grid_ready.editor.pending_world_action, nil, "ready grid completes its retry")
equal(grid_ready.editor.process_progress.state, "complete", "ready grid finishes F8")
assert(grid_ready.files["world-editor/blueprints/capture-1.sfbp"], "ready grid saves the V7 blueprint")

local grid_timeout, grid_timeout_cells = cache_capture(1, math.huge)
grid_timeout.mod.on_update(5)
grid_timeout.mod.on_update(5)
grid_timeout.mod.on_update(5)
capture_stopped(grid_timeout, "grid metadata timeout")
equal(grid_timeout_cells(), 0, "grid timeout never reads terrain")

local grid_invalid, grid_invalid_cells = cache_capture(1, math.huge)
grid_invalid.runtime.world.voxel.get_grid_spec = function() return nil, "native voxel grid returned invalid metadata" end
grid_invalid.mod.on_update(0.1)
capture_stopped(grid_invalid, "invalid grid metadata")
equal(grid_invalid_cells(), 0, "invalid metadata never reads terrain")

local grid_race, grid_race_cells = cache_capture(1, math.huge)
grid_race.runtime.has = function(name) return name ~= "runtime.world.voxel.grid_spec" end
grid_race.runtime.status = function() return {available=true} end
grid_race.mod.on_update(0.1)
assert(grid_race.editor.pending_world_action, "has/status readiness race retains the grid retry")
equal(grid_race_cells(), 0, "readiness race cannot consume guessed grid coordinates")
grid_race.runtime.has = function() return true end
grid_race.runtime.world.voxel.get_grid_spec = function()
    return {id="voxel", origin={0,0,0}, cellSize={0.5,0.5,0.5}, maximum={20480,20480,20480}}
end
grid_race.mod.on_update(0.1)
equal(grid_race.editor.process_progress.state, "complete", "ready metadata completes after the readiness race")

local bool_grid = cache_capture(1, math.huge)
bool_grid.runtime.world.voxel.get_grid_spec = function()
    return nil, "Grid is unavailable. This backend currently exposes 'voxel'"
end
bool_grid.mod.on_update(0.1)
assert(bool_grid.editor.pending_world_action, "installed bool-only grid bridge remains pending")
bool_grid.mod.on_update(15)
capture_stopped(bool_grid, "installed bool-only grid bridge timeout")

-- Exercise the actual P2P framing, headless capture and local save together.
-- Native mutations remain forbidden in both fixtures throughout these checks.
local function network_capture(options)
    options = options or {}
    local client = fixture("client-read-only", 81, false)
    local server = fixture("direct", 41, false, true)
    local client_id, server_id = "76561198000000002", "76561198000000001"
    local to_client, to_server, wire = {}, {}, {}
    local result = {client=client,server=server,wire=wire,reads=0,prop_reads=0}
    client.runtime.network.status = function() return {available=true,local_steam_id=client_id,configured_server_steam_id=server_id} end
    server.runtime.network.status = function() return {available=true,local_steam_id=server_id} end
    server.runtime.network.connected_peers = function() return options.unauthorized and {} or {client_id} end
    local function transport(from, destination)
        return function(peer, mod_id, payload)
            equal(mod_id,"world-editor","capture uses the existing mod channel")
            wire[#wire+1] = payload:sub(1,1)
            if from == server_id and payload:sub(1,2)=="C|" then
                if options.corrupt then payload=payload .. "!" end
                if options.drop_chunks then return true end
                if options.disconnect then client.session=82 end
            end
            if from == server_id and options.fail_send then return false,"test send failure" end
            if from == server_id and options.spoof then from="76561198000000003" end
            destination[#destination+1] = {peer_steam_id=from,from_mod=mod_id,payload=payload}
            return true
        end
    end
    client.runtime.network.send_mod = transport(client_id,to_server)
    server.runtime.network.send_mod = transport(server_id,to_client)
    local function receive(inbox)
        return function(limit)
            local messages={}
            while #inbox>0 and #messages<limit do messages[#messages+1]=table.remove(inbox,1) end
            return messages
        end
    end
    client.runtime.network.receive_mod = receive(to_client)
    server.runtime.network.receive_mod = receive(to_server)
    server.runtime.world.voxel.get_grid_spec = function()
        return {id="voxel",origin={0,0,0},cellSize={0.5,0.5,0.5},maximum={20480,20480,20480}}
    end
    server.runtime.world.voxel.read = function(_,_,_,sx,sy,sz)
        result.reads=result.reads+1
        if options.pending and result.reads==1 then return nil,"voxel-read-pending" end
        if options.timeout then return nil,"voxel-read-pending" end
        local cells={}
        for index=1,sx*sy*sz do cells[index]=65535 end
        return cells
    end
    server.runtime.world.entity.query_props_in_bounds = function()
        result.prop_reads=result.prop_reads+1
        if options.pending and result.prop_reads==1 then return nil,"still scanning" end
        return {{handle=92,itemId=42,templateUuidHighHex="fedcba9876543210",templateUuidLowHex="0123456789abcdef",
            transform={position={x=0.5,y=0.5,z=0.5},orientation={x=0,y=0,z=0,w=1},scale={x=1,y=1,z=1}}}}
    end
    result.server_clipboard = server.editor.clipboard
    result.journal={session=41,recovery_required=false}
    server.editor.undo_state=result.journal
    client.editor.selection_a={x=0,y=0,z=0}
    client.editor.selection_b=options.large and {x=31.5,y=7.5,z=31.5} or {x=1,y=1,z=1}
    if options.invalid then client.editor.selection_b.x=math.huge end
    if options.export_failure then
        client.io.export=function(path,contents)
            if path:find(".sfbp",1,true) then error("test export failure") end
            client.files[path]=contents
        end
    end
    result.tick=function(count)
        for _=1,count do client.mod.on_update(0.1);server.mod.on_update(0.1) end
    end
    client.actions.captureAndSave()
    return result
end

local online = network_capture({pending=true,large=true})
online.tick(20)
equal(online.client.editor.save_progress.state,"complete","online F8 saves after terrain and prop continuations")
equal(online.client.editor.active_blueprint_name,"capture-1","online F8 chooses a unique local capture name")
local online_file = assert(online.client.files["world-editor/blueprints/capture-1.sfbp"])
assert(#online_file>384*1024,"online F8 exercises multiple blueprint chunks")
equal(#online.client.editor.clipboard.cells,65536,"online F8 retains every server terrain cell")
equal(#online.client.editor.clipboard.props,1,"online F8 retains server props")
equal(online.client.editor.clipboard.props[1].templateUuidHighHex,"fedcba9876543210","online capture preserves exact UUID")
equal(online.server.editor.clipboard,online.server_clipboard,"server capture restores its previous clipboard")
equal(online.server.editor.undo_state,online.journal,"server capture preserves the existing placement journal")
equal(online.client.native_calls,0,"online F8 invokes no client world reader or writer")
equal(online.server.native_calls,0,"server capture invokes no native mutation")
equal(online.client.editor.remote_request_pending,false,"online F8 releases its request")
equal(online.client.editor.remote_recovery_required,false,"read-only capture creates no mutation recovery fence")
equal(online.server.files["world-editor/blueprints/capture-1.sfbp"],nil,"capture file is saved only on the client")

-- Incomplete, corrupt, stale or unauthorized responses must never become a file.
for _, options in ipairs({{corrupt=true},{drop_chunks=true},{disconnect=true},{invalid=true},
    {export_failure=true},{unauthorized=true},{spoof=true},{timeout=true},{fail_send=true}}) do
    local failed = network_capture(options)
    failed.tick(30)
    failed.client.mod.on_update(61)
    failed.server.mod.on_update(46)
    equal(failed.client.files["world-editor/blueprints/capture-1.sfbp"],nil,"failed online capture saves no incomplete file")
    assert(failed.client.editor.save_progress.state~="saving","failed online capture releases save progress")
    equal(failed.client.editor.remote_request_pending,false,"failed online capture clears the pending request")
    equal(failed.client.editor.remote_recovery_required,false,"failed capture cannot fence mutation recovery")
    equal(failed.server.editor.undo_state,failed.journal,"failed capture preserves the server undo journal")
    equal(failed.client.native_calls+failed.server.native_calls,0,"failed capture performs no mutation")
    if options.unauthorized or options.invalid then equal(failed.reads,0,"invalid or unauthorized capture performs no world read") end
end

local cancelled_online = network_capture({timeout=true})
cancelled_online.tick(2)
cancelled_online.client.actions.resetEditor()
cancelled_online.tick(20)
equal(cancelled_online.client.editor.remote_request_pending,false,"F6 cancels online F8")
equal(cancelled_online.client.editor.remote_release_pending,false,"capture cancellation sends no mutation abandon")
equal(cancelled_online.client.files["world-editor/blueprints/capture-1.sfbp"],nil,"cancelled online F8 cannot save later")
for _, command in ipairs(cancelled_online.wire) do assert(command~="A","read-only F6 must not abandon a placement transaction") end
equal(cancelled_online.server.editor.undo_state,cancelled_online.journal,"cancelled capture preserves server undo")

-- Shared framing must retain the existing F7/F4 protocol after adding F8.
do
    local bridge_factory = assert(load(world_editor_sources.p2p,"p2p-regression","t",_G))()
    local c_messages,s_messages={},{}
    local paste_calls,undo_calls,paste_content=0,0,nil
    local function api(id,incoming,outgoing,is_server)
        return {is_server=is_server,world={session_id=function() return 1 end},log={warn=function() end,debug=function() end},
            network={status=function() return {local_steam_id=id} end,
                send_mod=function(_,mod,payload) outgoing[#outgoing+1]={peer_steam_id=id,from_mod=mod,payload=payload};return true end,
                receive_mod=function() local copy={};while #incoming>0 do copy[#copy+1]=table.remove(incoming,1) end;return copy end}}
    end
    local peer,client_id="76561198000000001","76561198000000002"
    local results={}
    local client=bridge_factory(api(client_id,c_messages,s_messages,false),{on_result=function(op,ok,tx) results[op]={ok,tx} end})
    local server=bridge_factory(api(peer,s_messages,c_messages,true),{is_allowed_peer=function(p) return p==client_id end,
        on_paste=function(_,_,content,_,done) paste_calls=paste_calls+1;paste_content=content;done(true,"tx_fixture","ok") end,
        on_undo=function(_,_,tx,done) equal(tx,"tx_fixture","retained undo token");undo_calls=undo_calls+1;done(true,"ok") end})
    assert(client.start_paste(peer,online_file,{anchor={0,0,0},turns=1,voxelMode="replace",targetPropMode="keep"}))
    for _=1,10 do client.tick(0.1);server.tick(0.1) end
    equal(paste_calls,1,"F7 dispatches exactly once through shared multi-chunk framing")
    equal(paste_content,online_file,"F7 framing preserves the complete V7 file")
    assert(results.paste and results.paste[1],"F7 still receives confirmation")
    assert(client.start_undo(peer,results.paste[2]))
    for _=1,5 do client.tick(0.1);server.tick(0.1) end
    equal(undo_calls,1,"F4 dispatches exactly once after capture protocol changes")
    assert(results.undo and results.undo[1],"F4 still receives confirmation")
end

-- Simultaneous peers may use the same request number. The server binds it to
-- the authenticated peer, queues complete requests and never overlaps work.
do
    local factory=assert(load(world_editor_sources.p2p,"p2p-client-queue","t",_G))()
    local a,b="76561198000000002","76561198000000003"
    local inbox,replies,calls={},{},{}
    local session,authorized=41,{[a]=true,[b]=true}
    local function server_api()
        return {is_server=true,world={session_id=function() return session end},log={warn=function() end,debug=function() end},
            network={status=function() return {} end,
                send_mod=function(peer,_,payload) replies[#replies+1]={peer=peer,payload=payload};return true end,
                receive_mod=function(limit) local r={};while #inbox>0 and #r<limit do r[#r+1]=table.remove(inbox,1) end;return r end}}
    end
    local callbacks={is_allowed_peer=function(peer) return authorized[peer]==true end,
        on_paste=function(peer,id,content,_,done) calls[#calls+1]={peer=peer,id=id,content=content,done=done} end,
        on_capture=function(peer,id,_,done) calls[#calls+1]={peer=peer,id=id,done=done,capture=true} end,
        on_abandon=function(_,_,_,_,done) done(true,"released") end}
    local server=factory(server_api(),callbacks)
    local function upload(peer)
        local api={is_server=false,world={session_id=function() return 81 end},log={warn=function() end,debug=function() end},
            network={status=function() return {local_steam_id=peer} end,receive_mod=function() return {} end,
                send_mod=function(_,mod,payload)
                    payload=payload:gsub("^([BC])|[^|]+|","%1|same-request|")
                    inbox[#inbox+1]={peer_steam_id=peer,from_mod=mod,payload=payload};return true
                end}}
        local client=factory(api,{})
        assert(client.start_paste("76561198000000001",online_file,{anchor={10,10,10},turns=0,voxelMode="replace",targetPropMode="keep"}))
        -- Both headers arrive before either multi-chunk body is complete.
        return client
    end
    local ca,cb=upload(a),upload(b)
    ca.tick(0.1);cb.tick(0.1);ca.tick(0.1);cb.tick(0.1)
    server.tick(0.1)
    equal(#calls,1,"parallel uploads start exactly one native job")
    equal(calls[1].peer,a,"ready requests start in receive order")
    equal(calls[1].content,online_file,"interleaved peer A upload is intact")
    local duplicate=upload(a)
    duplicate.tick(0.1);duplicate.tick(0.1);server.tick(0.1)
    equal(#calls,1,"duplicate active request never dispatches again")
    calls[1].done(true,"tx-a","ok")
    server.tick(0.1)
    equal(#calls,2,"peer B starts after peer A completes")
    equal(calls[2].peer,b,"equal request IDs remain scoped to different peers")
    equal(calls[2].content,online_file,"interleaved peer B upload is intact")
    calls[1].done(true,"tx-a","duplicate completion")
    server.tick(0.1)
    equal(#calls,2,"late duplicate completion cannot release another peer's active job")
    calls[2].done(true,"tx-b","ok")
    duplicate=upload(a);duplicate.tick(0.1);duplicate.tick(0.1);server.tick(0.1)
    equal(#calls,2,"completed duplicate is answered from its peer-specific receipt")
    local seen_a,seen_b=false,false
    for _,r in ipairs(replies) do
        if r.payload:find("R|same-request|paste|1|tx-a|",1,true) then equal(r.peer,a,"A receipt is addressed only to A");seen_a=true end
        if r.payload:find("R|same-request|paste|1|tx-b|",1,true) then equal(r.peer,b,"B receipt is addressed only to B");seen_b=true end
    end
    assert(seen_a and seen_b,"both peers receive independent confirmations")

    inbox,replies,calls={},{},{}
    session,authorized=41,{[a]=true,[b]=true}
    server=factory(server_api(),callbacks)
    local crossing=upload(a)
    crossing.tick(0.1);server.tick(0.1)
    equal(#calls,0,"partial upload starts no native work")
    session=42
    crossing.tick(0.1);server.tick(0.1)
    equal(#calls,0,"upload cannot cross into a new server world session")

    -- Authorization, world session and queue age are checked again at dispatch.
    for _,mode in ipairs({"disconnect","world-change","expiry","cancel"}) do
        inbox,replies,calls={},{},{}
        session,authorized=41,{[a]=true,[b]=true}
        server=factory(server_api(),callbacks)
        for _,peer in ipairs({a,b}) do
            inbox[#inbox+1]={peer_steam_id=peer,from_mod="world-editor",payload="Q|queued-capture|0|0|0|1|1|1|y|v1"}
        end
        server.tick(0.1)
        equal(#calls,1,"capture queue serializes peers")
        if mode=="disconnect" then authorized[b]=nil
        elseif mode=="world-change" then session=42
        elseif mode=="expiry" then server.tick(46)
        else
            inbox[#inbox+1]={peer_steam_id=b,from_mod="world-editor",payload="A|cancel-b|-|queued-capture|v1"}
            server.tick(0.1)
        end
        calls[1].done(nil,"test complete")
        server.tick(0.1);server.tick(0.1)
        equal(#calls,1,"stale or cancelled queued request never reaches a native handler: " .. mode)
    end

    inbox,replies,calls={},{},{}
    session,authorized=41,{}
    server=factory(server_api(),callbacks)
    for index=1,34 do
        local peer=string.format("7656119800000%04d",index)
        authorized[peer]=true
        inbox[#inbox+1]={peer_steam_id=peer,from_mod="world-editor",payload="Q|capacity|0|0|0|1|1|1|y|v1"}
        server.tick(0.1)
    end
    equal(#calls,1,"full queue still has only one active handler")
    local full=false
    for _,r in ipairs(replies) do if r.payload:find("server-request-queue-limit-reached",1,true) then full=true end end
    assert(full,"queue rejects excess requests at its explicit bound")
end

-- Each client keeps its latest verified placement. Another F7 can replace
-- that client's confirmed journal, while recovery and other peers stay intact.
do
    local server=fixture("direct",41,false,true)
    local a,b="76561198000000002","76561198000000003"
    local prop=server.editor.clipboard.props[1]
    prop.sx,prop.sy,prop.sz=1,1,1
    local content=server.editor.encode_blueprint(server.editor.clipboard)
    local regions,live={},{}
    local handles,writes=100,0
    server.runtime.report_effect=function() end
    server.runtime.has=function(name) return name~="runtime.world.entity.set_scale" end
    server.runtime.world.voxel.get_grid_spec=function() return {id="voxel",origin={0,0,0},cellSize={0.5,0.5,0.5},maximum={20480,20480,20480}} end
    server.runtime.world.voxel.read=function(x) return regions[x] or {0,0,0,0,0,0} end
    server.runtime.world.voxel.write=function(x,_,_,_,_,_,cells) regions[x]=cells;writes=writes+1;return true end
    server.runtime.world.entity.spawn=function(high,low,position,rotation,item)
        handles=handles+1
        live[handles]=server.editor.intended_spawn_transform(position,rotation,{1,1,1},item,high,low)
        return handles
    end
    server.runtime.world.entity.get_transform=function(handle) return live[handle],live[handle] and nil or "entity-absent" end
    server.runtime.world.entity.destroy=function(handle) live[handle]=nil;return true end
    local function paste(peer,id,x)
        local result
        server.editor.server_begin_remote_paste(peer,id,content,{anchor={x,10,10},turns=0,voxelMode="replace",targetPropMode="keep"},
            function(ok,tx,reason) result={ok=ok,tx=tx,reason=reason} end,function() end)
        for _=1,5 do server.mod.on_update(0.1) end
        return assert(result,"server paste completes")
    end
    local first=paste(a,"a-first",10)
    assert(first.ok,"first A placement succeeds")
    local second=paste(a,"a-second",20)
    assert(second.ok,"same blueprint can be placed again without undo")
    equal(server.editor.server_remote_transactions[a].transaction,second.tx,"A retains the latest placement journal")
    local other=paste(b,"b-first",30)
    assert(other.ok,"B can place while A retains an undo journal")
    equal(server.editor.server_remote_transactions[a].transaction,second.tx,"B never overwrites A's journal")
    local foreign
    local before=writes
    server.editor.server_begin_remote_undo(b,"foreign-undo",second.tx,function(ok) foreign=ok end,function() end)
    equal(foreign,false,"B cannot undo A's transaction")
    equal(writes,before,"foreign undo cannot write terrain")
    server.editor.server_begin_remote_abandon(b,"foreign-release",second.tx,nil,function(ok) foreign=ok end)
    equal(foreign,false,"B cannot abandon A's transaction")
    server.editor.server_remote_transactions[a].undo.recovery_required=true
    local blocked=paste(a,"a-recovery-blocked",40)
    equal(blocked.ok,false,"incomplete A recovery still blocks a new A placement")
    equal(blocked.tx,second.tx,"recovery refusal retains A's exact token")
    for _,entry in ipairs({{a,second.tx},{b,other.tx}}) do
        local result
        server.editor.server_begin_remote_undo(entry[1],"undo-" .. entry[1],entry[2],function(ok) result=ok end,function() end)
        for _=1,5 do server.mod.on_update(0.1) end
        equal(result,true,"peer restores its own exact latest placement")
        equal(server.editor.server_remote_transactions[entry[1]],nil,"successful undo removes only its own journal")
    end
    local remaining=0
    for _ in pairs(live) do remaining=remaining+1 end
    equal(remaining,1,"undoing the latest A placement leaves the earlier successful A placement standing")
end
