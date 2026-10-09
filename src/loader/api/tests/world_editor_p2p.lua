local server_queue, client_queue, sent_client = {}, {}, {}
local client_id, server_id = "76561198000000001", "76561198000000002"
local function push(queue, value) queue[#queue + 1] = value end
local function receive(queue, limit)
    local result = {}
    for _ = 1, limit do
        local message = table.remove(queue, 1)
        if not message then break end
        result[#result + 1] = message
    end
    return result
end
local function network(role)
    return {
        status = function() return {available = true, local_steam_id = role == "client" and client_id or server_id} end,
        send_mod = function(peer, target, payload)
            assert(target == "world-editor")
            local message = {peer_steam_id = role == "client" and client_id or server_id,
                from_mod = "world-editor", payload = payload}
            if role == "client" then
                push(server_queue, message)
                sent_client[#sent_client + 1] = message
            else
                assert(peer == client_id)
                push(client_queue, message)
            end
            return true
        end,
        accept = function() return true end,
        receive_mod = function(limit) return receive(role == "client" and client_queue or server_queue, limit) end,
    }
end
local log = {warn = function() end, debug = function() end}
local client_results, placed, undo_calls = {}, {}, 0
local client = p2p_factory({network = network("client"), world = {session_id = function() return 4 end}, log = log}, {
    on_result = function(...) client_results[#client_results + 1] = {...} end,
})
local server = p2p_factory({network = network("server"), world = {session_id = function() return 9 end},
    is_server = true, log = log}, {
    is_allowed_peer = function(peer) return peer == client_id end,
    on_paste = function(peer, request, content, metadata, done)
        placed[#placed + 1] = {peer = peer, request = request, content = content, metadata = metadata}
        done(true, "tx_server_9_1", "native readback confirmed")
    end,
    on_undo = function(peer, request, transaction, done)
        undo_calls = undo_calls + 1
        assert(peer == client_id and transaction == "tx_server_9_1")
        done(true, "server journal restored")
    end,
})

local content = "SHROUDFORGE_WORLD_BLUEPRINT_V7\n" .. string.rep("192,0,128,1,\n", 41000)
local metadata = {anchor = {1.5, 2, -3.5}, turns = 2, voxelMode = "replace", targetPropMode = "keep"}
assert(#content > 384 * 1024, "fixture must cross a P2P chunk boundary")
local ok, request = client.start_paste(server_id, content, metadata)
assert(ok and client.pending() and request)
for _ = 1, 8 do
    client.tick(.03)
    server.tick(.03, function() return {client_id} end)
    client.tick(.03)
end
assert(#placed == 1 and placed[1].content == content, "server must reassemble the exact existing V7 text")
assert(placed[1].metadata.turns == 2 and placed[1].metadata.anchor[3] == -3.5)
assert(#client_results == 1 and client_results[1][1] == "paste" and client_results[1][2] == true)
assert(client_results[1][3] == "tx_server_9_1")

-- Replaying the exact reliable request returns its cached result instead of
-- invoking the world operation a second time.
for _, message in ipairs(sent_client) do push(server_queue, message) end
server.tick(.03, function() return {client_id} end)
client.tick(.03)
assert(#placed == 1 and #client_results == 1, "duplicate request must not repeat the paste")

assert(client.start_undo(server_id, "tx_server_9_1"))
for _ = 1, 5 do
    client.tick(.03)
    server.tick(.03, function() return {client_id} end)
    client.tick(.03)
end
assert(undo_calls == 1 and #client_results == 2)
assert(client_results[2][1] == "undo" and client_results[2][2] == true)

-- A missing peer response must not leave the editor permanently stuck. The
-- timeout is deliberately marked as an unknown outcome so the UI warns users
-- to verify the server world before retrying.
assert(client.start_undo(server_id, "tx_server_9_1"))
client.tick(61)
assert(not client.pending(), "a server that never responds must not block the editor forever")
assert(#client_results == 3 and client_results[3][1] == "undo" and client_results[3][2] == false)
assert(client_results[3][4]:find("outcome is unknown", 1, true))

-- Steam peer authorization and sender-mod identity are both checked before
-- any server callback is reached.
push(server_queue, {peer_steam_id = "76561198000000099", from_mod = "world-editor", payload = "U|bad_1|tx_server_9_1|v1"})
push(server_queue, {peer_steam_id = client_id, from_mod = "other-mod", payload = "U|bad_2|tx_server_9_1|v1"})
server.tick(.03, function() return {client_id} end)
assert(undo_calls == 1, "unauthorized peers and other mods must not invoke world operations")

local bad_id = "checksum_case_1"
push(server_queue, {peer_steam_id = client_id, from_mod = "world-editor", payload =
    table.concat({"B", bad_id, 3, 1, "00000000", 0, 0, 0, 0, "replace", "keep", "v1"}, "|")})
push(server_queue, {peer_steam_id = client_id, from_mod = "world-editor", payload = "C|" .. bad_id .. "|1|v1\nBAD"})
server.tick(.03, function() return {client_id} end)
assert(#placed == 1, "corrupted or truncated blueprint data must be rejected before invoking paste")

local too_large = string.rep("x", 32 * 1024 * 1024 + 1)
local accepted, reason = client.start_paste(server_id, too_large, metadata)
assert(not accepted and reason:find("32 MiB", 1, true), "P2P size limit must fail before queuing network data")
