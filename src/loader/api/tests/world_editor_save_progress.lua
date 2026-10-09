local actions, states, latest = {}, {}, {}
local fail_write, fail_library = false, false
local writes = 0
local function noop() end
shroudforge = {
    ui = {on_action = function(name, callback) actions[name] = callback end},
    settings = {get = function(name) if name == 'blueprintName' then return 'test' end end},
    log = {info = noop, warn = noop, error = noop, debug = noop},
}
runtime = {require = noop}
io.read_export_to_string = function()
    return table.concat({'SHROUDFORGE_WORLD_BLUEPRINT_V7', 'y', '1,1,1', 'voxel',
        '1,1,1', '0,0,0', '0.5,0.5,0.5', '128', '2', '0'}, '\n')
end
io.export = function(path, content)
    if path == 'world-editor/editor-state.txt' then
        latest = {}
        for key, value in content:gmatch('([^=\n]+)=([^\n]*)') do latest[key] = value end
        states[#states + 1] = latest
    else
        assert(latest.saveState == 'saving' and latest.saveCompleted == '2',
            'must not report file written before export succeeds')
        assert(content:find('SHROUDFORGE_WORLD_BLUEPRINT_V7', 1, true))
        if fail_write then error('disk full') end
        writes = writes + 1
    end
end
io.export_list = function()
    assert(latest.saveState == 'saving' and latest.saveCompleted == '3',
        'must not report completion before library refresh')
    if fail_library then error('directory unavailable') end
    return {'world-editor/blueprints/test.sfbp'}
end
local editor = assert(load(editor_source))()
actions.loadBlueprint('test')
actions.saveBlueprint()
assert(writes == 1 and latest.saveState == 'complete' and latest.saveCompleted == '4')
local sequence = latest.saveSequence
fail_write = true
actions.saveBlueprint()
assert(writes == 1 and latest.saveState == 'error' and latest.saveCompleted == '2')
assert(latest.saveSequence ~= sequence, 'each new save must reset elapsed time')
fail_write, fail_library = false, true
actions.saveBlueprint()
assert(writes == 2 and latest.saveState == 'error' and latest.saveCompleted == '3')
assert(latest.savePhase == 'Saved; library refresh failed')
fail_library = false
actions.saveBlueprint()
assert(writes == 3 and latest.saveState == 'complete' and latest.saveCompleted == '4')

-- Continue an unfinished capture beyond the former 30-second wall-clock limit.
local function upvalue(fn, name, replacement)
    for index = 1, 100 do
        local key, value = debug.getupvalue(fn, index)
        if not key then break end
        if key == name then
            if replacement ~= nil then debug.setupvalue(fn, index, replacement) end
            return value
        end
    end
    error('missing test seam: ' .. name)
end
local update = upvalue(editor.on_update, 'update_pending_queries')
local delivered = false
local pending = {region = {}, elapsed = 0, callback = function() delivered = true end}
upvalue(update, 'pending_prop_capture', pending)
upvalue(update, 'capture_region_props', function() return nil, 'live prop query is still scanning; retry on the next update' end)
runtime.has = function() return true end
update(31)
assert(upvalue(update, 'pending_prop_capture') == pending and not delivered,
    'a progressing native scan must not be cancelled by a Lua total-duration timeout')
upvalue(update, 'capture_region_props', function() return {} end)
update(0.03)
assert(delivered and upvalue(update, 'pending_prop_capture') == nil)

-- Starting a fresh capture cancels the old continuation instead of allowing it
-- to save over the newly selected editor state later.
upvalue(update, 'pending_prop_capture', pending)
actions.newBlueprint()
assert(upvalue(update, 'pending_prop_capture') == nil)
assert(latest.stage == 'need_a')

-- Cancelling a capture does not throw away an earlier successful paste's undo.
local journal = {recovery_required = false, entities = {}}
upvalue(actions.undoVoxels, 'undo_state', journal)
actions.resetEditor()
assert(upvalue(actions.undoVoxels, 'undo_state') == journal)

-- An immediate scan failure must leave the capture/save state retryable.
local request = upvalue(actions.listProps, 'request_prop_capture')
local message = upvalue(request, 'set_editor_message')
message('capturing', 'test')
upvalue(request, 'capture_region_props', function() return nil, 'scan failed' end)
request({}, function() error('failed scan must not deliver results') end)
assert(latest.stage == 'ready')

-- A journal from another world must not consume missing handles or write voxels.
runtime.world={session_id=function() return 22 end}
journal.session=21
local retained=upvalue(actions.undoVoxels,'undo_state')
actions.undoVoxels()
assert(latest.stage=='recovery' and upvalue(actions.undoVoxels,'undo_state')==retained)
runtime.world.session_id=function() return 0 end
actions.undoVoxels()
assert(upvalue(actions.undoVoxels,'undo_state')==retained)
