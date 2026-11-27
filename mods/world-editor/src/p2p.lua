-- Small, reliable transport for World Editor requests. Blueprint bodies use
-- SFBP format 8 and are validated before sending or server dispatch.
return function(api, callbacks)
    local M = {}
    local mod_id = "world-editor"
    local chunk_bytes = 384 * 1024
    local maximum_blueprint_bytes = 32 * 1024 * 1024
    local client_response_timeout_seconds = 60
    local client_stall_notice_seconds = 10
    local uploads, completed = {}, {}
    local completed_order = {}
    local upload_count, upload_bytes = 0, 0
    local server_queue, queued_keys, queued_peers = {}, {}, {}
    local queued_bytes, active_request = 0, nil
    local maximum_queued_requests = 32
    local outgoing = {}
    local next_id = 0
    local client_pending = nil
    local last_abandon = nil
    local server_busy = false
    local capture_response = nil
    local deferred_abandons = {}
    local receive_elapsed = 0
    local last_receive_failure

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

    local function valid_selection(metadata)
        if type(metadata) ~= "table" or (metadata.axis ~= "x" and metadata.axis ~= "y" and metadata.axis ~= "z") then return false end
        for _, point in ipairs({metadata.a, metadata.b}) do
            if type(point) ~= "table" then return false end
            for index=1,3 do if not finite(point[index]) then return false end end
        end
        return type(metadata.a) == "table" and type(metadata.b) == "table"
    end

    local function valid_transfer(bytes, chunks, checksum)
        return bytes and bytes % 1 == 0 and bytes > 0 and bytes <= maximum_blueprint_bytes and
            chunks and chunks % 1 == 0 and chunks == math.ceil(bytes / chunk_bytes) and
            type(checksum) == "string" and checksum:match("^%x%x%x%x%x%x%x%x$") ~= nil
    end

    local function append_chunk(transfer, request_id, sequence, body)
        if not transfer or request_id ~= transfer.request_id or not sequence or sequence % 1 ~= 0 or
           sequence ~= #transfer.parts + 1 or sequence > transfer.chunk_count or #body == 0 or #body > chunk_bytes then
            return false, "invalid-blueprint-chunk"
        end
        if transfer.received_bytes + #body > transfer.byte_count then return false, "blueprint-size-limit-exceeded" end
        transfer.parts[sequence] = body
        transfer.received_bytes = transfer.received_bytes + #body
        return true
    end

    local function transfer_content(transfer)
        if not transfer or #transfer.parts ~= transfer.chunk_count then return nil end
        local content = table.concat(transfer.parts)
        if #content ~= transfer.byte_count or adler32(content) ~= transfer.checksum then return nil end
        return content
    end

    local function enqueue_blueprint(peer, request_id, header, content)
        outgoing[#outgoing + 1] = {peer=peer,payload=header}
        for index=1,math.ceil(#content / chunk_bytes) do
            local first = (index - 1) * chunk_bytes + 1
            outgoing[#outgoing + 1] = {peer=peer,
                payload="C|" .. request_id .. "|" .. index .. "|v1\n" .. content:sub(first, first + chunk_bytes - 1)}
        end
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

    local function queue_result(request_id, operation, ok, transaction, reason, uncertain, content)
        local pending = client_pending
        if not pending or pending.request_id ~= request_id or pending.operation ~= operation then return end
        client_pending = nil
        if operation == "abandon" and ok and last_abandon and last_abandon.request_id == request_id then
            last_abandon = nil
        end
        if callbacks.on_result then callbacks.on_result(operation, ok, transaction, reason, pending.peer,
            request_id, uncertain == true, content) end
    end

    local function allowed(peer)
        return callbacks.is_allowed_peer and callbacks.is_allowed_peer(peer) == true
    end

    local function validate_blueprint(content)
        if type(callbacks.validate_blueprint) ~= "function" then
            return nil, "SFBP format 8 validation is unavailable"
        end
        local ok, blueprint, cell_count, prop_count = pcall(callbacks.validate_blueprint, content)
        if not ok then return nil, tostring(blueprint) end
        if not blueprint then return nil, cell_count or "invalid SFBP format 8 blueprint" end
        return blueprint, cell_count, prop_count
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
            if not deferred_abandons[key] then
                local count=0
                for _ in pairs(deferred_abandons) do count=count+1 end
                if count >= maximum_queued_requests then
                    reply(peer,request_id,"abandon",false,nil,"server-release-queue-limit-reached")
                    return
                end
            end
            deferred_abandons[key] = {peer = peer, request_id = request_id,
                transaction = transaction, target_request_id = target_request_id}
            return
        end
        server_busy = true
        active_request = {peer=peer,request_id=request_id,operation="abandon",key=key}
        local finished = false
        local function done(ok, reason)
            if finished then return end
            finished, server_busy, active_request = true, false, nil
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

    local function drop_upload(peer)
        local upload = uploads[peer]
        if upload then
            upload_count, upload_bytes = upload_count-1, upload_bytes-upload.byte_count
            uploads[peer] = nil
        end
    end

    local function request_pending(peer, request_id, operation, transaction)
        local key = peer .. ":" .. request_id
        local pending = queued_keys[key] or (server_busy and active_request and active_request.key == key and active_request)
        if not pending then return false end
        if pending.operation ~= operation or (operation == "undo" and pending.transaction ~= transaction) then
            reply(peer,request_id,operation,false,nil,"request-id-operation-mismatch")
        else
            report_progress(peer,request_id,operation,0,0,"Queued on server","The server already owns this request. It will execute it once.")
        end
        return true
    end

    local function queue_server_request(peer, request_id, operation, bytes, run, transaction)
        if request_pending(peer,request_id,operation,transaction) then return end
        local reason
        local session = api.world.session_id()
        if session == 0 then reason="server-world-session-unavailable"
        elseif #server_queue >= maximum_queued_requests or queued_peers[peer] or queued_bytes+bytes > maximum_blueprint_bytes then
            reason="server-request-queue-limit-reached"
        end
        if reason then
            remember(peer .. ":" .. request_id,{operation=operation,ok=false,transaction=transaction,reason=reason})
            reply(peer,request_id,operation,false,transaction,reason)
            return
        end
        local item={peer=peer,request_id=request_id,operation=operation,bytes=bytes,run=run,
            transaction=transaction,session=session,key=peer .. ":" .. request_id,elapsed=0}
        server_queue[#server_queue+1]=item
        queued_keys[item.key],queued_peers[peer]=item,true
        queued_bytes=queued_bytes+bytes
        report_progress(peer,request_id,operation,#server_queue,0,"Queued on server","Waiting for earlier requests. The server executes one world operation at a time.")
    end

    local function remove_queued(index)
        local item=table.remove(server_queue,index)
        queued_keys[item.key],queued_peers[item.peer]=nil,nil
        queued_bytes=queued_bytes-item.bytes
        return item
    end

    local function pump_server_queue()
        if server_busy or #server_queue == 0 then return end
        local item=remove_queued(1)
        local reason
        if not allowed(item.peer) then reason="server-peer-no-longer-authorized"
        elseif item.session ~= api.world.session_id() then reason="server-world-session-changed-before-dispatch"
        elseif item.elapsed >= 45 then reason="server-request-expired-before-dispatch" end
        if reason then
            remember(item.key,{operation=item.operation,ok=false,transaction=item.transaction,reason=reason})
            reply(item.peer,item.request_id,item.operation,false,item.transaction,reason)
            return
        end
        active_request,server_busy=item,true
        item.run()
    end

    local function finish_upload(peer, upload)
        if #upload.parts ~= upload.chunk_count then return end
        local content = transfer_content(upload)
        drop_upload(peer)
        if upload.session == 0 or upload.session ~= api.world.session_id() then
            reply(peer,upload.request_id,"paste",false,nil,"server-world-session-changed-during-upload")
            return
        end
        if not content then
            reply(peer, upload.request_id, "paste", false, nil, "blueprint-integrity-check-failed")
            return
        end
        local blueprint, cell_count, prop_count = validate_blueprint(content)
        if not blueprint then
            reply(peer, upload.request_id, "paste", false, nil,
                "invalid-sfbp-v8-blueprint: " .. clean(cell_count))
            return
        end
        queue_server_request(peer,upload.request_id,"paste",#content,function()
            local finished = false
            local function progress(phase, completed, total, message)
                report_progress(peer, upload.request_id, "paste", completed, total, phase, message)
            end
            local function done(ok, transaction, reason)
                if finished then return end
                finished, server_busy, active_request = true, false, nil
                remember(peer .. ":" .. upload.request_id, {operation = "paste", ok = ok,
                    transaction = transaction, reason = clean(reason)})
                reply(peer, upload.request_id, "paste", ok, transaction, reason)
                drain_deferred_abandons()
            end
            progress("Validating blueprint", 1, 5, "The server received the complete blueprint and is validating it.")
            local ok, reason = pcall(callbacks.on_paste, peer, upload.request_id, content, upload.metadata,
                done, progress, blueprint, cell_count, prop_count)
            if not ok then done(false, nil, "server-paste-handler-failed: " .. tostring(reason)) end
        end)
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
        if command == "Q" and #fields == 10 and fields[10] == "v1" then
            local request_id = fields[2]
            local metadata = {a={tonumber(fields[3]),tonumber(fields[4]),tonumber(fields[5])},
                b={tonumber(fields[6]),tonumber(fields[7]),tonumber(fields[8])},axis=fields[9]}
            if not valid_id(request_id) or not valid_selection(metadata) then
                reply(peer, request_id or "invalid", "capture", false, nil, "invalid-capture-selection")
                return
            end
            if completed[peer .. ":" .. request_id] then
                reply(peer, request_id, "capture", false, nil, "capture-already-completed-use-a-new-request")
                return
            end
            queue_server_request(peer,request_id,"capture",0,function()
                local finished = false
                local function done(content, reason)
                    if finished then return end
                    finished = true
                    if type(content) ~= "string" or #content == 0 or #content > maximum_blueprint_bytes then
                        server_busy, active_request = false, nil
                        reason = reason or "server-capture-exceeds-transfer-limit"
                        remember(peer .. ":" .. request_id, {operation="capture",ok=false,reason=clean(reason)})
                        reply(peer, request_id, "capture", false, nil, reason)
                        drain_deferred_abandons()
                        return
                    end
                    capture_response = {peer=peer,request_id=request_id}
                    enqueue_blueprint(peer, request_id,
                        table.concat({"D",request_id,#content,math.ceil(#content / chunk_bytes),adler32(content),"v1"},"|"),content)
                    outgoing[#outgoing + 1] = {peer=peer,
                        payload=table.concat({"R",request_id,"capture","1","-","capture-confirmed"},"|"),
                        on_sent=function()
                            capture_response, server_busy, active_request = nil, false, nil
                            remember(peer .. ":" .. request_id, {operation="capture",ok=true,reason="capture-confirmed"})
                            drain_deferred_abandons()
                        end}
                end
                if not callbacks.on_capture then done(nil,"server-capture-handler-unavailable");return end
                local function progress(phase, completed, total, detail)
                    report_progress(peer, request_id, "capture", completed, total, phase, detail)
                end
                local ok, reason = pcall(callbacks.on_capture, peer, request_id, metadata, done, progress)
                if not ok then done(nil,"server-capture-handler-failed: " .. tostring(reason)) end
            end)
            return
        elseif command == "B" and #fields == 12 then
            local request_id, byte_count, chunk_count = fields[2], tonumber(fields[3]), tonumber(fields[4])
            local checksum, turns = fields[5], tonumber(fields[6])
            local anchor = {tonumber(fields[7]), tonumber(fields[8]), tonumber(fields[9])}
            local voxel_mode, prop_mode = fields[10], fields[11]
            local reserved = fields[12]
            if not valid_id(request_id) or not valid_transfer(byte_count, chunk_count, checksum) or
               not turns or turns % 1 ~= 0 or turns < 0 or turns > 3 or
               not finite(anchor[1]) or not finite(anchor[2]) or not finite(anchor[3]) or
               (voxel_mode ~= "add" and voxel_mode ~= "replace") or
               (prop_mode ~= "keep" and prop_mode ~= "replace") or reserved ~= "v1" then
                reply(peer, request_id or "invalid", "paste", false, nil, "invalid-blueprint-header")
                return
            end
            if completed[peer .. ":" .. request_id] then
                local old = completed[peer .. ":" .. request_id]
                if old.operation ~= "paste" then reply(peer,request_id,"paste",false,nil,"request-id-operation-mismatch")
                else reply(peer, request_id, "paste", old.ok, old.transaction, old.reason) end
                return
            end
            if request_pending(peer,request_id,"paste") then return end
            if upload_count >= maximum_queued_requests or upload_bytes+byte_count > maximum_blueprint_bytes then
                reply(peer,request_id,"paste",false,nil,"server-upload-limit-reached")
                return
            end
            if uploads[peer] then
                if uploads[peer].request_id == request_id then
                    report_progress(peer,request_id,"paste",0,0,"Receiving blueprint","This upload is already in progress.")
                    return
                end
                reply(peer, request_id, "paste", false, nil, "client-upload-already-in-progress")
                return
            end
            upload_count,upload_bytes=upload_count+1,upload_bytes+byte_count
            uploads[peer] = {request_id = request_id, byte_count = byte_count, chunk_count = chunk_count,
                checksum = checksum:lower(), parts = {}, received_bytes = 0,
                metadata = {turns = turns, anchor = anchor, voxelMode = voxel_mode, targetPropMode = prop_mode},
                elapsed = 0, session = api.world.session_id()}
            return
        elseif command == "C" and #fields == 4 and fields[4] == "v1" then
            local request_id, sequence = fields[2], tonumber(fields[3])
            local upload = uploads[peer]
            local appended, reason = append_chunk(upload, request_id, sequence, body)
            if not appended and reason == "blueprint-size-limit-exceeded" then
                drop_upload(peer)
                reply(peer, request_id, "paste", false, nil, reason)
            end
            if not appended then return end
            if sequence == upload.chunk_count then finish_upload(peer, upload) end
            return
        elseif command == "U" and #fields == 4 then
            local request_id, transaction = fields[2], fields[3]
            if not valid_id(request_id) or not valid_id(transaction) then return end
            local cached = completed[peer .. ":" .. request_id]
            if cached then
                if cached.operation ~= "undo" or cached.transaction ~= transaction then
                    reply(peer,request_id,"undo",false,transaction,"request-id-operation-mismatch")
                else reply(peer, request_id, "undo", cached.ok, transaction, cached.reason) end
                return
            end
            queue_server_request(peer,request_id,"undo",0,function()
                local finished = false
                local function progress(phase, completed, total, message)
                    report_progress(peer, request_id, "undo", completed, total, phase, message)
                end
                local function done(ok, reason)
                    if finished then return end
                    finished, server_busy, active_request = true, false, nil
                    remember(peer .. ":" .. request_id, {operation = "undo", ok = ok,
                        transaction = transaction, reason = clean(reason)})
                    reply(peer, request_id, "undo", ok, transaction, reason)
                    drain_deferred_abandons()
                end
                progress("Checking undo journal", 1, 4, "The server is checking the saved undo journal.")
                local ok, reason = pcall(callbacks.on_undo, peer, request_id, transaction, done, progress)
                if not ok then done(false, "server-undo-handler-failed: " .. tostring(reason)) end
            end,transaction)
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
                drop_upload(peer)
            end
            for index=#server_queue,1,-1 do
                local item=server_queue[index]
                if item.peer==peer and item.request_id==target_request_id then
                    remove_queued(index)
                    remember(item.key,{operation=item.operation,ok=false,transaction=item.transaction,reason="request-cancelled-before-dispatch"})
                end
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
        local blueprint, validation_error = validate_blueprint(content)
        if not blueprint then
            return false, "Only a valid SFBP format 8 blueprint can be uploaded: " .. clean(validation_error)
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
        enqueue_blueprint(peer, request_id, head, content)
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

    function M.start_capture(peer, metadata)
        if client_pending then return false, "A World Editor P2P request is already waiting for the server." end
        if not valid_selection(metadata) then return false, "Mark two finite world positions and choose a valid blueprint axis." end
        local request_id = make_request_id()
        local a, b = metadata.a, metadata.b
        outgoing[#outgoing + 1] = {peer=peer,payload=table.concat({"Q",request_id,a[1],a[2],a[3],b[1],b[2],b[3],metadata.axis,"v1"},"|")}
        client_pending = {request_id=request_id,operation="capture",peer=peer,elapsed=0}
        return true, request_id
    end

    function M.cancel_capture()
        if not client_pending or client_pending.operation ~= "capture" then return false end
        outgoing = {}
        queue_result(client_pending.request_id,"capture",false,nil,"capture-cancelled")
        return true
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

    local function receive_capture_part(payload)
        if not client_pending or client_pending.operation ~= "capture" then return false end
        local header, body = payload:match("^([^\n]*)\n(.*)$")
        header, body = header or payload, body or ""
        local fields = {}
        for field in (header .. "|"):gmatch("(.-)|") do fields[#fields+1]=field end
        if fields[1] ~= "D" and fields[1] ~= "C" then return false end
        if fields[2] ~= client_pending.request_id then return true end
        if fields[1] == "D" then
            local bytes, chunks = tonumber(fields[3]), tonumber(fields[4])
            if #fields ~= 6 or fields[6] ~= "v1" or client_pending.download or
               not valid_transfer(bytes,chunks,fields[5]) then
                queue_result(fields[2],"capture",false,nil,"invalid-server-blueprint-header")
                return true
            end
            client_pending.download = {request_id=fields[2],byte_count=bytes,chunk_count=chunks,
                checksum=fields[5]:lower(),parts={},received_bytes=0}
        else
            local download = client_pending.download
            local appended, reason
            if #fields == 4 and fields[4] == "v1" and download and not download.content then
                appended, reason = append_chunk(download,fields[2],tonumber(fields[3]),body)
            end
            if not appended then
                queue_result(fields[2],"capture",false,nil,reason or "invalid-server-blueprint-chunk")
                return true
            end
            if #download.parts == download.chunk_count then
                download.content = transfer_content(download)
                if not download.content then
                    queue_result(fields[2],"capture",false,nil,"server-blueprint-integrity-check-failed")
                    return true
                end
                download.parts = {}
            end
        end
        client_pending.since_progress, client_pending.stall_notified = 0, false
        return true
    end

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
                        "server-release-not-confirmed-after-60-seconds. Press {{keybind:resetEditor}} to resend the same release",
                        pending.peer, pending.request_id, true) end
                elseif pending.operation == "capture" then
                    outgoing = {}
                    queue_result(pending.request_id,"capture",false,nil,"no-response-from-server-capture-after-60-seconds")
                else
                    outgoing = {}
                    queue_result(pending.request_id, pending.operation, false, nil,
                        "no-response-from-server-peer-after-60-seconds, outcome is unknown. {{keybind:resetEditor}} can request release",
                        true)
                end
            end
        end
        for peer, upload in pairs(uploads) do
            upload.elapsed = upload.elapsed + delta
            if upload.elapsed >= 60 then
                drop_upload(peer)
                reply(peer, upload.request_id, "paste", false, nil, "blueprint-upload-expired")
            end
        end
        for index, item in ipairs(server_queue) do
            item.elapsed=item.elapsed+delta
            item.report_elapsed=(item.report_elapsed or 0)+delta
            if item.report_elapsed >= 2 then
                item.report_elapsed=0
                report_progress(item.peer,item.request_id,item.operation,index,0,"Queued on server",
                    "Waiting for earlier requests. The server executes one world operation at a time.")
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
                    if capture_response then
                        capture_response, server_busy, active_request = nil, false, nil
                        drain_deferred_abandons()
                    end
                end
                break
            end
            table.remove(outgoing, 1)
            if item.on_sent then item.on_sent() end
            sent_count = sent_count + 1
        end
        receive_elapsed = receive_elapsed + delta
        if receive_elapsed < 0.1 then return end
        receive_elapsed = 0
        if not api.is_server and not client_pending then
            last_receive_failure = nil
            return
        end
        local messages, reason = api.network.receive_mod(16)
        if not messages then
            local failure = reason and tostring(reason) or nil
            if failure ~= last_receive_failure then
                if failure and api.log and api.log.debug then api.log.debug("World Editor P2P receive: " .. failure) end
                last_receive_failure = failure
            end
            return
        end
        last_receive_failure = nil
        for _, message in ipairs(messages) do
            if api.is_server then
                handle_server_message(message)
            elseif message.from_mod == mod_id and client_pending and message.peer_steam_id == client_pending.peer then
                if not receive_capture_part(message.payload) then
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
                            local content
                            if result_operation == "capture" and ok == "1" then
                                content = client_pending.download and client_pending.download.content
                                if not content then ok, result_detail = "0", "server-capture-result-without-complete-blueprint" end
                            end
                            queue_result(result_id, result_operation, ok == "1", transaction ~= "-" and transaction or nil, result_detail, false, content)
                        end
                    end
                end
            end
        end
        if api.is_server then pump_server_queue() end
    end

    return M
end
