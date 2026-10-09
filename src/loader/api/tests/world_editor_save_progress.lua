local actions, states, latest = {}, {}, {}
local fail_write, fail_library = false, false
local writes = 0
local execution_mode = 'direct'
local function noop() end
shroudforge = {
    ui = {on_action = function(name, callback) actions[name] = callback end},
    settings = {get = function(name)
        if name == 'blueprintName' then return 'test' end
        if name == 'executionMode' then return execution_mode end
    end},
    log = {info = noop, warn = noop, error = noop, debug = noop},
}
runtime = {require = noop, world = {session_id = function() return 21 end}}
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
    -- UI callbacks synchronize the session before invoking the editor action.
    for index = 1, 100 do
        local key, value = debug.getupvalue(fn, index)
        if not key then break end
        if key == 'callback' and type(value) == 'function' then
            if name == 'callback' then return value end
            return upvalue(value, name, replacement)
        end
    end
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
upvalue(actions.undoVoxels,'callback')()
assert(latest.stage=='recovery' and upvalue(actions.undoVoxels,'undo_state')==retained)
runtime.world.session_id=function() return 0 end
upvalue(actions.undoVoxels,'callback')()
assert(upvalue(actions.undoVoxels,'undo_state')==retained)

-- Context waits are finite and cannot deliver a closure into another session.
local session, ready = 30, false
runtime.world.session_id=function() return session end
runtime.has=function() return ready end
runtime.status=function() return {reason='active voxel world context is not available'} end
local requested=0
local wait={operation='runtime.world.voxel.read', session=30, elapsed=0,
    action=function() requested=requested+1 end}
upvalue(update,'pending_world_action',wait)
update(14)
assert(upvalue(update,'pending_world_action')==wait and requested==0)
update(1)
assert(upvalue(update,'pending_world_action')==nil and requested==0 and latest.stage=='ready')
wait.elapsed=0; upvalue(update,'pending_world_action',wait)
session,ready=31,true
update(.03)
assert(requested==0 and upvalue(update,'pending_world_action')==nil)
wait.elapsed=0; wait.session=31; upvalue(update,'pending_world_action',wait)
update(.03)
assert(requested==1 and upvalue(update,'pending_world_action')==nil)

-- Transition cleanup retains the blueprint but quarantines journals and
-- stale continuations. Temporary unavailability preserves the same journal.
local transition=upvalue(editor.on_update,'update_world_session')
session=21
transition()
session=0; transition()
assert(upvalue(actions.undoVoxels,'undo_state')==retained)
upvalue(update,'pending_world_action',wait)
session=32; transition()
assert(upvalue(actions.undoVoxels,'undo_state')==nil)
local retired=upvalue(transition,'retired_sessions')
assert(#retired==1 and retired[1].undo==retained)
assert(upvalue(update,'pending_world_action')==nil)
transition()
assert(#retired==1, 'stable session must not repeatedly archive state')

-- Target selection must not infer a remote server from transient read-only ECS
-- access: the client executable also hosts local worlds. Direct remains the
-- safe default; only explicit dedicated-server targeting selects P2P.
local backend=upvalue(actions.pasteVoxels,'execution_backend')
runtime.has=function(name) return name~='runtime.world.voxel.write' end
assert(backend()=='direct', 'read-only runtime state must not redirect a local world to a server')
execution_mode='p2p'
assert(backend()=='p2p', 'explicit server target must route through P2P when client world APIs are unavailable')
execution_mode='direct'
assert(backend()=='direct')
