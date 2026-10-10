-- Small, reliable transport for World Editor requests. Blueprint bodies are the
-- existing SFBP V7 text; this module only frames/chunks them for Steam P2P.
return function(api, callbacks)
    local M = {}
    local mod_id = "world-editor"
    local chunk_bytes = 384 * 1024
    local maximum_blueprint_bytes = 32 * 1024 * 1024
    local client_response_timeout_seconds = 60
    local client_stall_notice_seconds = 10
    local uploads, completed = {}, {}
    local completed_order, active_upload_peer = {}, nil
    local outgoing = {}
    local next_id = 0
    local client_pending = nil
    local last_abandon = nil
    local server_busy = false
    local deferred_abandons = {}
    local receive_elapsed = 0

    local function valid_id(value)
        return type(value) == "string" and #value > 0 and #value <= 96 and value:match("^[%w_-]+$") ~= nil
    end

    local function finite(value)
        return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
    end

    local function remember(key, value)
        completed[key] = value
        completed_order[#completed_order + 1] = key
        while #completed_order > 128 do
            local oldest = table.remove(completed_order, 1)
            completed[oldest] = nil
        end
    end

    local function adler32(text)
        local a, b = 1, 0
        for index = 1, #text do
            a = (a + text:byte(index)) % 65521
            b = (b + a) % 65521
        end
        return string.format("%08x", b * 65536 + a)
    end

    local function clean(value)
        return tostring(value or ""):gsub("[|\r\n]", " "):sub(1, 180)
    end

    local function send(peer, payload)
        if type(peer) ~= "string" or peer == "" then return false, "peer Steam ID is not configured" end
        local ok, reason = api.network.send_mod(peer, mod_id, payload, {reliable = true})
        if not ok then return false, tostring(reason or "Steam P2P send failed") end
        return true
    end

    local function reply(peer, request_id, operation, ok, transaction, reason)
        local payload = table.concat({"R", request_id, operation, ok and "1" or "0",
            transaction or "-", clean(reason)}, "|")
        local sent, send_reason = send(peer, payload)
        if not sent then
            api.log.warn("World Editor P2P could not send a result to " .. peer .. ": " .. send_reason)
        end
    end

    local function report_progress(peer, request_id, operation, completed, total, phase, message)
        local payload = table.concat({"P", request_id, operation, "running",
            tostring(completed or 0), tostring(total or 0), clean(phase), clean(message)}, "|")
        local sent, send_reason = send(peer, payload)
        if not sent and api.log and api.log.debug then
            api.log.debug("World Editor P2P could not send progress to " .. peer .. ": " .. send_reason)
        end
    end

    local function queue_result(request_id, operation, ok, transaction, reason, uncertain)
        local pending = client_pending
        if not pending or pending.request_id ~= request_id or pending.operation ~= operation then return end
        client_pending = nil
        if operation == "abandon" and ok and last_abandon and last_abandon.request_id == request_id then
            last_abandon = nil
        end
        if callbacks.on_result then callbacks.on_result(operation, ok, transaction, reason, pending.peer,
            request_id, uncertain == true) end
    end

    local function allowed(peer)
        return callbacks.is_allowed_peer and callbacks.is_allowed_peer(peer) == true
    end

    local function abandon_key(peer, request_id)
        return peer .. ":" .. request_id
    end

    local function dispatch_abandon(peer, request_id, transaction, target_request_id)
        local key = abandon_key(peer, request_id)
        local cached = completed[key]
        if cached then
            reply(peer, request_id, "abandon", cached.ok, cached.transaction, cached.reason)
            return
        end
        if server_busy then
            deferred_abandons[key] = {peer = peer, request_id = request_id,
                transaction = transaction, target_request_id = target_request_id}
            return
        end
        server_busy = true
        local finished = false
        local function done(ok, reason)
            if finished then return end
            finished, server_busy = true, false
            remember(key, {operation = "abandon", ok = ok, transaction = nil, reason = clean(reason)})
            reply(peer, request_id, "abandon", ok, nil, reason)
            local next_key, next_abandon = next(deferred_abandons)
            if next_key and next_abandon then
                deferred_abandons[next_key] = nil
                dispatch_abandon(next_abandon.peer, next_abandon.request_id,
                    next_abandon.transaction, next_abandon.target_request_id)
            end
        end
        if not callbacks.on_abandon then done(false, "server-abandon-handler-unavailable"); return end
        local ok, reason = pcall(callbacks.on_abandon, peer, request_id, transaction,
            target_request_id, done)
        if not ok then done(false, "server-abandon-handler-failed: " .. tostring(reason)) end
    end

    local function drain_deferred_abandons()
        if server_busy then return end
        local key, item = next(deferred_abandons)
        if not key or not item then return end
        deferred_abandons[key] = nil
        dispatch_abandon(item.peer, item.request_id, item.transaction, item.target_request_id)
    end

    local function finish_upload(peer, upload)
        if #upload.parts ~= upload.chunk_count then return end
        local content = table.concat(upload.parts)
        uploads[peer], active_upload_peer = nil, nil
        if #content ~= upload.byte_count or adler32(content) ~= upload.checksum then
            reply(peer, upload.request_id, "paste", false, nil, "blueprint-integrity-check-failed")
            return
        end
        if server_busy then
            reply(peer, upload.request_id, "paste", false, nil, "server-world-editor-is-busy")
            return
        end
        server_busy = true
        local finished = false
        local function progress(phase, completed, total, message)
            report_progress(peer, upload.request_id, "paste", completed, total, phase, message)
        end
        local function done(ok, transaction, reason)
            if finished then return end
            finished, server_busy = true, false
            remember(peer .. ":" .. upload.request_id, {operation = "paste", ok = ok,
                transaction = transaction, reason = clean(reason)})
            reply(peer, upload.request_id, "paste", ok, transaction, reason)
            drain_deferred_abandons()
        end
        progress("Validating blueprint", 1, 5, "The server received the complete blueprint and is validating it.")
        local ok, reason = pcall(callbacks.on_paste, peer, upload.request_id, content, upload.metadata, done, progress)
        if not ok then done(false, nil, "server-paste-handler-failed: " .. tostring(reason)) end
    end

    local function handle_server_message(message)
        local peer, payload = message.peer_steam_id, message.payload
        if not allowed(peer) then return end
        if type(message.from_mod) ~= "string" or message.from_mod ~= mod_id or type(payload) ~= "string" then return end
        local header, body = payload:match("^([^\n]*)\n(.*)$")
        if not header then header, body = payload, "" end
        local fields = {}
        for field in (header .. "|"):gmatch("(.-)|") do fields[#fields + 1] = field end
        local command = fields[1]
        if command == "B" and #fields == 12 then
            local request_id, byte_count, chunk_count = fields[2], tonumber(fields[3]), tonumber(fields[4])
            local checksum, turns = fields[5], tonumber(fields[6])
            local anchor = {tonumber(fields[7]), tonumber(fields[8]), tonumber(fields[9])}
            local voxel_mode, prop_mode = fields[10], fields[11]
            local reserved = fields[12]
            if not valid_id(request_id) or not byte_count or byte_count < 1 or byte_count > maximum_blueprint_bytes or
               not chunk_count or chunk_count % 1 ~= 0 or chunk_count < 1 or chunk_count > math.ceil(maximum_blueprint_bytes / chunk_bytes) or
               byte_count % 1 ~= 0 or
               type(checksum) ~= "string" or not checksum:match("^%x%x%x%x%x%x%x%x$") or
               not turns or turns % 1 ~= 0 or turns < 0 or turns > 3 or
               not finite(anchor[1]) or not finite(anchor[2]) or not finite(anchor[3]) or
               (voxel_mode ~= "add" and voxel_mode ~= "replace") or
               (prop_mode ~= "keep" and prop_mode ~= "replace") or reserved ~= "v1" then
                reply(peer, request_id or "invalid", "paste", false, nil, "invalid-blueprint-header")
                return
            end
            if completed[peer .. ":" .. request_id] then
                local old = completed[peer .. ":" .. request_id]
                reply(peer, request_id, "paste", old.ok, old.transaction, old.reason)
                return
            end
            if server_busy then
                reply(peer, request_id, "paste", false, nil, "server-world-editor-is-busy")
                return
            end
            if active_upload_peer and active_upload_peer ~= peer then
                reply(peer, request_id, "paste", false, nil, "another-client-upload-is-in-progress")
                return
            end
            if uploads[peer] then
                reply(peer, request_id, "paste", false, nil, "client-upload-already-in-progress")
                return
            end
            active_upload_peer = peer
            uploads[peer] = {request_id = request_id, byte_count = byte_count, chunk_count = chunk_count,
                checksum = checksum:lower(), parts = {}, received_bytes = 0,
                metadata = {turns = turns, anchor = anchor, voxelMode = voxel_mode, targetPropMode = prop_mode},
                elapsed = 0}
            return
        elseif command == "C" and #fields == 4 and fields[4] == "v1" then
            local request_id, sequence = fields[2], tonumber(fields[3])
            local upload = uploads[peer]
            if not upload or request_id ~= upload.request_id or not sequence or sequence % 1 ~= 0 or
               sequence ~= #upload.parts + 1 or sequence > upload.chunk_count or #body == 0 or #body > chunk_bytes then
                return
            end
            upload.parts[sequence] = body
            upload.received_bytes = upload.received_bytes + #body
            if upload.received_bytes > upload.byte_count then
                uploads[peer], active_upload_peer = nil, nil
                reply(peer, request_id, "paste", false, nil, "blueprint-size-limit-exceeded")
                return
            end
            if sequence == upload.chunk_count then finish_upload(peer, upload) end
            return
        elseif command == "U" and #fields == 4 then
            local request_id, transaction = fields[2], fields[3]
            if not valid_id(request_id) or not valid_id(transaction) then return end
            local cached = completed[peer .. ":" .. request_id]
            if cached then reply(peer, request_id, "undo", cached.ok, transaction, cached.reason); return end
            if server_busy then reply(peer, request_id, "undo", false, transaction, "server-world-editor-is-busy"); return end
            server_busy = true
            local finished = false
            local function progress(phase, completed, total, message)
                report_progress(peer, request_id, "undo", completed, total, phase, message)
            end
            local function done(ok, reason)
                if finished then return end
                finished, server_busy = true, false
                remember(peer .. ":" .. request_id, {operation = "undo", ok = ok,
                    transaction = transaction, reason = clean(reason)})
                reply(peer, request_id, "undo", ok, transaction, reason)
                drain_deferred_abandons()
            end
            progress("Checking undo journal", 1, 4, "The server is checking the saved undo journal.")
            local ok, reason = pcall(callbacks.on_undo, peer, request_id, transaction, done, progress)
            if not ok then done(false, "server-undo-handler-failed: " .. tostring(reason)) end
            return
        elseif command == "A" and #fields == 5 and fields[5] == "v1" then
            local request_id, transaction, target_request_id = fields[2], fields[3], fields[4]
            if not valid_id(request_id) or (transaction ~= "-" and not valid_id(transaction)) or
               (target_request_id ~= "-" and not valid_id(target_request_id)) then return end
            if completed[abandon_key(peer, request_id)] then
                local cached = completed[abandon_key(peer, request_id)]
                reply(peer, request_id, "abandon", cached.ok, nil, cached.reason)
                return
            end
            local upload = uploads[peer]
            if upload and (target_request_id == "-" or upload.request_id == target_request_id) then
                uploads[peer], active_upload_peer = nil, nil
            end
            dispatch_abandon(peer, request_id,
                transaction ~= "-" and transaction or nil,
                target_request_id ~= "-" and target_request_id or nil)
            return
        elseif command == "S" and #fields == 3 then
            local request_id = fields[2]
            local cached = completed[peer .. ":" .. request_id]
            if cached then reply(peer, request_id, cached.operation, cached.ok, cached.transaction, cached.reason) end
        end
    end

    local function make_request_id()
        next_id = next_id + 1
        local status = api.network.status()
        local local_id = status and status.local_steam_id or "client"
        local session = api.world.session_id and api.world.session_id() or 0
        local nonce = tostring({}):match("0x(%x+)") or string.format("%08x", math.random(0, 2147483647))
        local clock = os and os.clock and math.floor(os.clock() * 1000) or 0
        return tostring(local_id) .. "-" .. tostring(session) .. "-" .. nonce .. "-" .. tostring(clock) .. "-" .. tostring(next_id)
    end

    function M.start_paste(peer, content, metadata)
        if client_pending then return false, "A World Editor P2P request is already waiting for the server." end
        if type(content) ~= "string" or #content == 0 or #content > maximum_blueprint_bytes then
            return false, "This blueprint exceeds the 32 MiB multiplayer transfer limit. Local blueprints are unchanged."
        end
        if type(metadata) ~= "table" or type(metadata.turns) ~= "number" or metadata.turns % 1 ~= 0 or
           metadata.turns < 0 or metadata.turns > 3 or
           (metadata.voxelMode ~= "add" and metadata.voxelMode ~= "replace") or
           (metadata.targetPropMode ~= "keep" and metadata.targetPropMode ~= "replace") then
            return false, "The server placement options are invalid."
        end
        local anchor = metadata.anchor or {}
        for index = 1, 3 do
            if not finite(anchor[index]) then
                return false, "The cursor position is invalid. Aim at a valid target and retry."
            end
        end
        local request_id = make_request_id()
        local chunk_count = math.ceil(#content / chunk_bytes)
        local head = table.concat({"B", request_id, #content, chunk_count, adler32(content),
            metadata.turns, anchor[1], anchor[2], anchor[3], metadata.voxelMode,
            metadata.targetPropMode, "v1"}, "|")
        outgoing[#outgoing + 1] = {peer = peer, payload = head}
        for index = 1, chunk_count do
            local first = (index - 1) * chunk_bytes + 1
            outgoing[#outgoing + 1] = {peer = peer,
                payload = "C|" .. request_id .. "|" .. index .. "|v1\n" .. content:sub(first, first + chunk_bytes - 1)}
        end
        client_pending = {request_id = request_id, operation = "paste", peer = peer, elapsed = 0}
        return true, request_id
    end

    function M.start_undo(peer, transaction)
        if client_pending then return false, "A World Editor P2P request is already waiting for the server." end
        if not valid_id(transaction) then return false, "The server undo token is invalid." end
        local request_id = make_request_id()
        outgoing[#outgoing + 1] = {peer = peer, payload = table.concat({"U", request_id, transaction, "v1"}, "|")}
        client_pending = {request_id = request_id, operation = "undo", peer = peer, elapsed = 0}
        return true, request_id
    end

    function M.start_abandon(peer, transaction, target_request_id)
        if client_pending and client_pending.operation == "abandon" then
            local pending = client_pending
            pending.elapsed, pending.since_progress, pending.stall_notified, pending.send_error_reported = 0, 0, false, false
            outgoing = {{peer = pending.peer, payload = pending.payload}}
            return true, pending.request_id
        end
        if type(peer) ~= "string" or peer == "" then return false, "The server Steam peer is unavailable." end
        if client_pending and client_pending.peer ~= peer then
            return false, "A different server request is still active. Wait for its result before abandoning it."
        end
        if transaction and not valid_id(transaction) then return false, "The server undo token is invalid." end
        if target_request_id and not valid_id(target_request_id) then
            return false, "The pending server request ID is invalid."
        end
        local target = target_request_id or (client_pending and client_pending.request_id)
        if last_abandon and last_abandon.peer == peer and last_abandon.transaction == (transaction or "-") and
           last_abandon.target_request_id == (target or "-") then
            local pending = last_abandon
            client_pending = {request_id = pending.request_id, operation = "abandon", peer = peer,
                elapsed = 0, since_progress = 0, stall_notified = false,
                send_error_reported = false, payload = pending.payload}
            outgoing = {{peer = peer, payload = pending.payload}}
            return true, pending.request_id
        end
        local request_id = make_request_id()
        local payload = table.concat({"A", request_id, transaction or "-", target or "-", "v1"}, "|")
        last_abandon = {peer = peer, request_id = request_id, transaction = transaction or "-",
            target_request_id = target or "-", payload = payload}
        outgoing = {{peer = peer, payload = payload}}
        client_pending = {request_id = request_id, operation = "abandon", peer = peer,
            elapsed = 0, since_progress = 0, stall_notified = false,
            send_error_reported = false, payload = payload}
        return true, request_id
    end

    function M.pending() return client_pending ~= nil end

    function M.tick(delta_seconds)
        local delta = math.max(0, tonumber(delta_seconds) or 0)
        if client_pending then
            client_pending.elapsed = client_pending.elapsed + delta
            client_pending.since_progress = (client_pending.since_progress or 0) + delta
            if client_pending.operation ~= "abandon" and not client_pending.stall_notified and
               client_pending.since_progress >= client_stall_notice_seconds then
                client_pending.stall_notified = true
                if callbacks.on_stalled then
                    callbacks.on_stalled(client_pending.operation, client_pending.peer,
                        client_pending.request_id)
                end
            end
            if client_pending.elapsed >= client_response_timeout_seconds then
                local pending = client_pending
                if pending.operation == "abandon" then
                    pending.elapsed, pending.since_progress = 0, 0
                    outgoing[#outgoing + 1] = {peer = pending.peer, payload = pending.payload}
                    if callbacks.on_result then callbacks.on_result("abandon", false, nil,
                        "server-release-not-confirmed-after-60-seconds. Press F6 to resend the same release",
                        pending.peer, pending.request_id, true) end
                else
                    outgoing = {}
                    queue_result(pending.request_id, pending.operation, false, nil,
                        "no-response-from-server-peer-after-60-seconds, outcome is unknown. F6 can request release",
                        true)
                end
            end
        end
        for peer, upload in pairs(uploads) do
            upload.elapsed = upload.elapsed + delta
            if upload.elapsed >= 60 then
                uploads[peer], active_upload_peer = nil, nil
                reply(peer, upload.request_id, "paste", false, nil, "blueprint-upload-expired")
            end
        end
        local sent_count = 0
        while #outgoing > 0 and sent_count < 2 do
            local item = outgoing[1]
            local ok, reason = send(item.peer, item.payload)
            if not ok then
                if client_pending and client_pending.operation == "abandon" then
                    if not client_pending.send_error_reported then
                        client_pending.send_error_reported = true
                        if callbacks.on_result then callbacks.on_result("abandon", false, nil,
                            "server-release-send-failed: " .. tostring(reason), client_pending.peer,
                            client_pending.request_id, true) end
                    end
                else
                    if client_pending then queue_result(client_pending.request_id, client_pending.operation, false, nil, reason, true) end
                    outgoing = {}
                end
                break
            end
            table.remove(outgoing, 1)
            sent_count = sent_count + 1
        end
        receive_elapsed = receive_elapsed + delta
        if receive_elapsed < 0.1 then return end
        receive_elapsed = 0
        if not api.is_server and not client_pending then return end
        local messages, reason = api.network.receive_mod(16)
        if not messages then
            if reason and api.log and api.log.debug then api.log.debug("World Editor P2P receive: " .. tostring(reason)) end
            return
        end
        for _, message in ipairs(messages) do
            if api.is_server then
                handle_server_message(message)
            elseif message.from_mod == mod_id and client_pending and message.peer_steam_id == client_pending.peer then
                local request_id, operation, state, completed, total, phase, detail =
                    message.payload:match("^P|([^|]+)|([^|]+)|([^|]+)|(%d+)|(%d+)|([^|]*)|([^|]*)$")
                if request_id == client_pending.request_id and operation == client_pending.operation then
                    client_pending.since_progress, client_pending.stall_notified = 0, false
                    if callbacks.on_progress then
                        callbacks.on_progress(operation, state, tonumber(completed), tonumber(total), phase, detail, client_pending.peer)
                    end
                else
                    local result_id, result_operation, ok, transaction, result_detail =
                        message.payload:match("^R|([^|]+)|([^|]+)|([01])|([^|]+)|([^|]*)$")
                    if result_id == client_pending.request_id and result_operation == client_pending.operation then
                        queue_result(result_id, result_operation, ok == "1", transaction ~= "-" and transaction or nil, result_detail)
                    end
                end
            end
        end
    end

    return M
end
