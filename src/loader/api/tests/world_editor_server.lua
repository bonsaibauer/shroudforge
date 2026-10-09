local client_id, server_id = "76561198000000001", "76561198000000002"
local to_server, to_client = {}, {}
local voxels = {0}
local writes = {}
local settings = {serverSteamId = server_id, allowedClientSteamIds = "",
    rotationQuarterTurns = 0, rotationAxis = "y", maximumCopyableProps = 60000,
    pasteVoxelMode = "replace", targetPropMode = "keep"}
local function push(queue, message) queue[#queue + 1] = message end
local function receive(queue, limit)
    local result = {}
    for _ = 1, limit do
        local message = table.remove(queue, 1)
        if not message then break end
        result[#result + 1] = message
    end
    return result
end
local function endpoint(role)
    local local_id = role == "server" and server_id or client_id
    return {
        status = function() return {available = true, role = role, local_steam_id = local_id} end,
        send_mod = function(peer, target, payload)
            assert(target == "world-editor")
            local message = {peer_steam_id = local_id, from_mod = "world-editor", payload = payload}
            push(role == "server" and to_client or to_server, message)
            return true
        end,
        accept = function() return true end,
        connected_peers = function()
            return role == "server" and {client_id} or {}
        end,
        receive_mod = function(limit) return receive(role == "server" and to_server or to_client, limit) end,
    }
end
local noop = function() end
local logger = {info = noop, debug = noop, trace = noop, warn = noop, error = noop}
local server_runtime = {
    is_server = true,
    require = noop,
    has = function() return true end,
    status = function() return {reason = "ready"} end,
    report_effect = noop,
    network = endpoint("server"),
    world = {
        session_id = function() return 9 end,
        voxel = {
            get_grid_spec = function() return {origin = {0, 0, 0}, cellSize = {.5, .5, .5}} end,
            read = function(x, y, z, sx, sy, sz)
                assert(x == 0 and y == 0 and z == 0 and sx == 1 and sy == 1 and sz == 1)
                return {voxels[1]}
            end,
            write = function(x, y, z, sx, sy, sz, values)
                assert(x == 0 and y == 0 and z == 0 and sx == 1 and sy == 1 and sz == 1)
                voxels[1] = values[1]
                writes[#writes + 1] = values[1]
                return true
            end,
        },
    },
}
local server_env = setmetatable({runtime = server_runtime,
    shroudforge = {settings = {get = function(name) return settings[name] end}, log = logger},
    game = {assets = {}}, editor_source = editor_source,
    require = function(name) if name == "p2p" then return p2p_factory end; return builtin_require(name) end,
}, {__index = _G})
local server_editor = assert(load(editor_source, "world-editor-server", "t", server_env))()
server_editor.on_load()

local client = p2p_factory({network = endpoint("client"), world = {session_id = function() return 3 end}, log = logger}, {
    on_result = function(operation, ok, transaction, reason)
        _G.last_client_result = {operation, ok, transaction, reason}
    end,
})
local blueprint = table.concat({"SHROUDFORGE_WORLD_BLUEPRINT_V7", "y", "0.5,0.5,0.5", "voxel",
    "1,1,1", "0,0,0", "0.5,0.5,0.5", "192", "2", "0"}, "\n")
assert(client.start_paste(server_id, blueprint, {anchor = {0, 0, 0}, turns = 0,
    voxelMode = "replace", targetPropMode = "keep"}))
for _ = 1, 8 do
    client.tick(.03)
    server_editor.on_update(.03)
    client.tick(.03)
end
assert(voxels[1] == 192 and writes[1] == 192, "the dedicated server's native voxel API must apply F7")
assert(last_client_result and last_client_result[1] == "paste" and last_client_result[2] == true)
local transaction = assert(last_client_result[3], "the server must return an undo token, not a client ECS handle")

assert(client.start_undo(server_id, transaction))
for _ = 1, 8 do
    client.tick(.03)
    server_editor.on_update(.03)
    client.tick(.03)
end
assert(voxels[1] == 0 and writes[2] == 0, "F4 must restore the server-owned voxel snapshot")
assert(last_client_result[1] == "undo" and last_client_result[2] == true)

-- A client may retain a stale local undo token if the success response was
-- lost. The server's completed journal is authoritative; the next F7 must work.
assert(client.start_paste(server_id, blueprint, {anchor = {0, 0, 0}, turns = 0,
    voxelMode = "replace", targetPropMode = "keep"}))
for _ = 1, 8 do
    client.tick(.03)
    server_editor.on_update(.03)
    client.tick(.03)
end
assert(voxels[1] == 192 and writes[3] == 192,
    "a completed server undo must not leave the client-side queue blocking F7")
assert(last_client_result[1] == "paste" and last_client_result[2] == true,
    "server must accept the next paste once its prior undo completed")
