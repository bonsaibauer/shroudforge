-- The multiplayer path uses ordinary player input. It never writes client
-- voxels or spawns client-only entities and never opens another connection.
return function(api, message)
    local job, pending, elapsed = nil, nil, 0
    local function scalar(value)
        if type(value) == "number" then return value end
        if value ~= nil then return tonumber(value.value) end
    end
    local function player()
        local players, reason = api.ecs.query("keen::ecs::ClientPlayerInput", "keen::ecs::NetworkCursor", "keen::ecs::SlotSelection")
        if not players then return nil, reason end
        if #players ~= 1 then return nil, "Select one local player before using the building queue" end
        return players[1]
    end
    local function query(command)
        local p = command.position
        return api.world.entity.query_props({p[1]-.02,p[2]-.02,p[3]-.02,p[1]+.02,p[2]+.02,p[3]+.02},0)
    end
    local function equivalent(prop, command)
        if prop.itemId ~= command.itemId then return false end
        if command.entityId and prop.entityId ~= command.entityId then return false end
        if command.templateUuidHighHex and (prop.templateUuidHighHex~=command.templateUuidHighHex or
            prop.templateUuidLowHex~=command.templateUuidLowHex) then return false end
        local t = prop.transform
        if not t then return false end
        local p, q, s = t.position, t.orientation, t.scale
        local r = command.rotation
        local dot = q.x*r[1] + q.y*r[2] + q.z*r[3] + q.w*r[4]
        return math.abs(p.x-command.position[1])<.01 and math.abs(p.y-command.position[2])<.01 and
            math.abs(p.z-command.position[3])<.01 and math.abs(math.abs(dot)-1)<.001 and
            math.abs(s.x-command.scale[1])<.001 and math.abs(s.y-command.scale[2])<.001 and
            math.abs(s.z-command.scale[3])<.001
    end
    local function stop(reason)
        local old = job; job = nil
        if old then
            if old.request then api.world.building.cancel(old.request) end
            if old.phase=="effect" then pending=old end
            old.done(false, reason, old.completed, pending~=nil)
        end
    end
    local function submit(command)
        command.player = job.player
        local id, reason = api.world.building.submit(command)
        if not id then stop(reason); return false end
        job.request, job.since = id, elapsed
        return true
    end
    local result = {}
    local function scanning(reason)
        -- Only player discovery is retried. No submitted building action is
        -- resubmitted, and the existing phase deadline still bounds waiting.
        return type(reason)=="string" and (reason:find("still scanning",1,true)~=nil or
            reason == "native ECS query failed or timed out" or
            reason:find("not ready",1,true)~=nil)
    end
    local function runtime_unavailable(reason)
        return type(reason)=="string" and reason:find("not ready",1,true)~=nil
    end
    local function bind_player(active, handle)
        local slots = api.ecs.read(handle, "keen::ecs::SlotSelection")
        local slot = slots and slots.actionbarSlotSelection and scalar(slots.actionbarSlotSelection.index)
        if not slot or slot < 0 or slot > 255 then return nil, "Current building slot is unavailable" end
        active.player, active.slot, active.phase, active.since = handle, slot, "prepare", elapsed
        return true
    end
    local function observed(active, command)
        if command.cell then
            local p=command.cell
            local values=api.world.voxel.read(p[1],p[2],p[3],1,1,1)
            return values and values[1]==command.after
        end
        if command.action=="remove" or command.action=="dismantle" then
            if api.has and not api.has("runtime.world.entity.get_transform") then return end
            local prop, reason=api.world.entity.get_transform(command.handle)
            if prop then
                if command.entityId and prop.entityId~=command.entityId then
                    return nil,"Removal handle resolved to a different game entity; no input was sent"
                end
                return false
            end
            if reason and reason~="entity handle is stale or not a live prop" then return nil,reason end
            return true
        end
        local props=query(command)
        if not props then return end
        local found
        for _, prop in ipairs(props) do
            if not active.before[prop.handle] and equivalent(prop,command) then
                if found then return nil,"Placement produced ambiguous results; stopped" end
                found={handle=prop.handle,entityId=prop.entityId}
            end
        end
        return found
    end
    local function valid_vector(value, length, low, high)
        if type(value)~="table" or #value~=length then return false end
        for _, v in ipairs(value) do
            if type(v)~="number" or v~=v or v<=low or v>=high then return false end
        end
        return true
    end
    function result.start(commands, done)
        if job then return nil, "A building job is already running" end
        if pending then return nil,"The previous input has an unconfirmed outcome; observing it without resending" end
        if not api.world.building then return nil, "Update the loader to enable ordinary building input" end
        if #commands == 0 or #commands > 4096 then return nil, "Building jobs require 1–4096 actions" end
        -- Validate the entire batch before its first input, not halfway through
        -- an otherwise valid selection. Engine permissions remain authoritative.
        for _, command in ipairs(commands) do
            if (command.action~="place" and command.action~="remove" and command.action~="dismantle") or
                type(command.itemId)~="number" or command.itemId%1~=0 or command.itemId<=0 or command.itemId>0xffffffff or
                not valid_vector(command.position,3,-2147483648,2147483648) or
                not valid_vector(command.rotation,4,-math.huge,math.huge) or
                not valid_vector(command.scale,3,0,1024.000001) then
                return nil,"Invalid building command; no input was sent"
            end
            local norm=0
            for _, v in ipairs(command.rotation) do norm=norm+v*v end
            if norm<1e-12 or norm==math.huge then return nil,"Invalid building rotation; no input was sent" end
            for i,v in ipairs(command.rotation) do command.rotation[i]=v/math.sqrt(norm) end
            if (command.action=="remove" or command.action=="dismantle") and not command.cell and not command.handle then
                return nil,"Removal has no captured entity handle; no input was sent"
            end
        end
        local handle, reason = player()
        if not handle and not scanning(reason) then return nil, reason end
        local next_job = {session=api.world.session_id and api.world.session_id(), commands=commands, done=done, index=1, phase="player", completed={}, since=elapsed}
        if handle then
            local ok, bind_reason=bind_player(next_job,handle)
            if not ok then return nil,bind_reason end
        end
        job = next_job
        if not handle then message("building", "Waiting for the local player ECS scan; no input has been sent.") end
        local dismantle_only=true
        for _,command in ipairs(commands) do
            if command.action~="dismantle" or command.cell then dismantle_only=false; break end
        end
        message("building", dismantle_only and
            "Undo is using the saved blueprint prop positions. No manual aiming or item selection is required." or
            "Building through the game connection. Keep the building tool equipped.")
        return true
    end
    function result.busy() return job ~= nil end
    function result.cancel()
        if job then stop("Building queue stopped; already dispatched changes are still being observed") end
    end
    function result.close()
        if job and job.request then api.world.building.cancel(job.request) end
        job, pending=nil,nil
    end
    function result.reconcile()
        if not pending then return true end
        if pending.session and api.world.session_id() ~= pending.session then return false end
        if player()~=pending.player then return false end
        local command=pending.commands[pending.index]
        local confirmed=observed(pending,command)
        if not confirmed then return false end
        if type(confirmed)=="table" then
            command.resultHandle=confirmed.handle
            command.resultEntityId=confirmed.entityId
        else
            command.resultHandle=type(confirmed)=="number" and confirmed or nil
        end
        local old=pending; pending=nil
        old.done(false,"The delayed change was observed. The remaining queue was not sent; F4 can undo confirmed changes.",{command},false)
        return true
    end
    function result.tick(delta)
        elapsed = elapsed + math.max(0, math.min(1, tonumber(delta) or .03))
        if pending then result.reconcile() end
        if not job then return end
        if job.session then
            local current_session=api.world.session_id and api.world.session_id() or 0
            if current_session==0 then
                job.runtime_wait_since=job.runtime_wait_since or elapsed
                if not job.runtime_wait_message then
                    message("building","World/player ECS is temporarily unavailable. Undo is paused; no input was sent.")
                    job.runtime_wait_message=true
                end
                return
            end
            if current_session~=job.session then
                stop("World session changed; no further input was sent"); return
            end
        end
        local current_player, reason = player()
        if not current_player and scanning(reason) then
            if runtime_unavailable(reason) then
                job.runtime_wait_since=job.runtime_wait_since or elapsed
                if not job.runtime_wait_message then
                    message("building","World/player ECS is temporarily unavailable. Undo is paused; no input was sent.")
                    job.runtime_wait_message=true
                end
            else
                local timeout = job.phase=="target" and 60 or 15
                if elapsed-job.since > timeout then
                    stop("The requested change was not observed ("..(job.target_wait_reason or ("phase="..job.phase)).."). No automatic retry was sent.")
                end
            end
            return
        end
        if job.runtime_wait_since then
            job.since=job.since+(elapsed-job.runtime_wait_since)
            job.runtime_wait_since=nil
            job.runtime_wait_message=nil
        end
        local timeout = 15
        if elapsed-job.since > timeout then
            local detail = job.target_wait_reason or ("phase="..job.phase)
            stop("The requested change was not observed ("..detail.."). No automatic retry was sent.")
            return
        end
        if job.phase=="player" then
            if not current_player then stop(reason); return end
            local ok, bind_reason=bind_player(job,current_player)
            if not ok then stop(bind_reason); return end
        end
        if current_player ~= job.player then stop(reason or "Local player/world changed; building stopped"); return end
        local command = job.commands[job.index]
        if job.phase == "prepare" then
            if not command.cell and api.has and not api.has("runtime.world.entity.get_transform") then return end
            -- Undo is idempotent. If the exact pasted handle has already gone
            -- (for example because the player dismantled it manually), its
            -- inverse action is already satisfied. A reused or changed handle
            -- remains a conflict and is rejected below.
            if command.action == "dismantle" then
                local existing, read_reason=api.world.entity.get_transform(command.handle)
                if not existing and read_reason and read_reason~="entity handle is stale or not a live prop" then return end
                if not existing then
                    job.completed[#job.completed+1] = command
                    job.index=job.index+1
                    if job.index>#job.commands then
                        local done, completed = job.done, job.completed; job=nil; done(true,nil,completed)
                    else
                        job.phase="prepare"; job.since=elapsed
                        message("building", string.format("Observed %d/%d changes through the game connection",#job.completed,#job.commands))
                    end
                    return
                end
            end
            job.before = {}
            if command.cell then
                local p=command.cell
                local values=api.world.voxel.read(p[1],p[2],p[3],1,1,1)
                if not values then return end
                if values[1]~=command.before then stop("Target voxel changed since preparation; stopped"); return end
            elseif command.action == "place" then
                local props = query(command)
                if not props then return end
                for _, prop in ipairs(props) do job.before[prop.handle] = true end
            end
            if (command.action == "remove" or command.action == "dismantle") and not command.cell then
                if not command.handle then stop("Removal has no captured entity handle"); return end
                local prop = api.world.entity.get_transform(command.handle)
                if not prop or not equivalent(prop, command) then stop("The target changed; removal was not sent"); return end
            end
            if command.action=="dismantle" and not command.cell then
                -- The exact live handle and complete transform were checked
                -- immediately above. Send those coordinates through the normal
                -- client input path; do not make the player aim or select it.
                message("building",string.format("Undo is sending the held dismantle input for the exact pasted prop at %.2f, %.2f, %.2f. No manual aiming is required.",table.unpack(command.position)))
                if not command.entityId then stop("The pasted prop's exact game entity ID is missing; dismantle input was not sent"); return end
                if submit({action="dismantle", targetEntityId=command.entityId, position=command.position, rotation=command.rotation, scale=command.scale}) then
                    job.phase="effect"
                end
            elseif submit({action="select", itemId=command.itemId, materialItemId=command.materialItemId, slot=job.slot}) then
                job.phase="selection"
            end
        elseif job.phase == "selection" then
            local status = api.world.building.status(job.request)
            if status == "timeout" or status == "cancelled" or status == "unknown" then stop("Item selection "..status); return end
            if status ~= "dispatched" then return end
            local selection = api.ecs.read(job.player, "keen::ecs::NetworkCursor")
            if not selection or scalar(selection.currentBuildingItemId) ~= command.itemId then
                job.target_wait_reason="selected building item was not observed"
                return
            end
            -- Selection may take several network frames. Recheck the exact
            -- target immediately before any ordinary building input.
            if command.cell then
                local p=command.cell
                local values=api.world.voxel.read(p[1],p[2],p[3],1,1,1)
                if not values then return end
                if values[1]~=command.before then stop("Target voxel changed while selecting the item; no building input sent"); return end
            elseif command.action=="remove" or command.action=="dismantle" then
                local prop=api.world.entity.get_transform(command.handle)
                if not prop or not equivalent(prop,command) then stop("Removal target changed while selecting the item"); return end
            else
                local props=query(command)
                if not props then return end
                job.before={}
                for _,prop in ipairs(props) do job.before[prop.handle]=true end
            end
            if submit({action=command.action, position=command.position, rotation=command.rotation, scale=command.scale}) then
                job.phase="effect"
            end
        elseif job.phase == "effect" then
            local status = api.world.building.status(job.request)
            if status == "timeout" or status == "cancelled" or status == "unknown" then stop("Building input "..status); return end
            if status ~= "dispatched" then return end
            local confirmed, observe_reason=observed(job,command)
            if observe_reason then
                if scanning(observe_reason) then return end
                stop(observe_reason); return
            end
            if not confirmed then return end
            if type(confirmed)=="table" then
                command.resultHandle=confirmed.handle
                command.resultEntityId=confirmed.entityId
            else
                command.resultHandle=type(confirmed)=="number" and confirmed or nil
            end
            job.completed[#job.completed+1] = command
            job.index=job.index+1
            if job.index>#job.commands then
                api.world.building.cancel(job.request) -- release this mod's input sequence
                local done, completed = job.done, job.completed; job=nil; done(true,nil,completed)
            else
                job.phase="prepare"; job.since=elapsed
                job.aim_message=nil
                message("building", string.format("Observed %d/%d changes through the game connection",#job.completed,#job.commands))
            end
        end
    end
    return result
end
