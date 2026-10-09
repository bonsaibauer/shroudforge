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
    local function observed(active, command)
        if command.cell then
            local p=command.cell
            local values=api.world.voxel.read(p[1],p[2],p[3],1,1,1)
            return values and values[1]==command.after
        end
        local props=query(command)
        if not props then return end
        if command.action=="remove" or command.action=="dismantle" then
            if api.world.entity.get_transform(command.handle) then return false end
            for _, prop in ipairs(props) do if prop.handle==command.handle then return false end end
            return true
        end
        local found
        for _, prop in ipairs(props) do
            if not active.before[prop.handle] and equivalent(prop,command) then
                if found then return nil,"Placement produced ambiguous results; stopped" end
                found=prop.handle
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
        if not handle then return nil, reason end
        local slots = api.ecs.read(handle, "keen::ecs::SlotSelection")
        local slot = slots and slots.actionbarSlotSelection and scalar(slots.actionbarSlotSelection.index)
        if not slot or slot < 0 or slot > 255 then return nil, "Current building slot is unavailable" end
        job = {commands=commands, done=done, player=handle, slot=slot, index=1, phase="prepare", completed={}, since=elapsed}
        message("building", "Building through the game connection. Keep the building tool equipped.")
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
        if player()~=pending.player then return false end
        local command=pending.commands[pending.index]
        local confirmed=observed(pending,command)
        if not confirmed then return false end
        command.resultHandle=type(confirmed)=="number" and confirmed or nil
        local old=pending; pending=nil
        old.done(false,"The delayed change was observed. The remaining queue was not sent; F4 can undo confirmed changes.",{command},false)
        return true
    end
    function result.tick(delta)
        elapsed = elapsed + math.max(0, math.min(1, tonumber(delta) or .03))
        if pending then result.reconcile() end
        if not job then return end
        local current_player, reason = player()
        if current_player ~= job.player then stop(reason or "Local player/world changed; building stopped"); return end
        if elapsed-job.since > 15 then
            stop("The requested change was not observed. Check build permissions, inventory, range and server response. No automatic retry was sent.")
            return
        end
        local command = job.commands[job.index]
        if job.phase == "prepare" then
            -- Undo is idempotent. If the exact pasted handle has already gone
            -- (for example because the player dismantled it manually), its
            -- inverse action is already satisfied. A reused or changed handle
            -- remains a conflict and is rejected below.
            if command.action == "dismantle" and not api.world.entity.get_transform(command.handle) then
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
            job.before = {}
            if command.cell then
                local p=command.cell
                local values=api.world.voxel.read(p[1],p[2],p[3],1,1,1)
                if not values then return end
                if values[1]~=command.before then stop("Target voxel changed since preparation; stopped"); return end
            else
                local props = query(command)
                if not props then return end
                for _, prop in ipairs(props) do job.before[prop.handle] = true end
            end
            if (command.action == "remove" or command.action == "dismantle") and not command.cell then
                if not command.handle then stop("Removal has no captured entity handle"); return end
                local prop = api.world.entity.get_transform(command.handle)
                if not prop or not equivalent(prop, command) then stop("The target changed; removal was not sent"); return end
            end
            if submit({action="select", itemId=command.itemId, materialItemId=command.materialItemId, slot=job.slot}) then
                job.phase="selection"
            end
        elseif job.phase == "selection" then
            local status = api.world.building.status(job.request)
            if status == "timeout" or status == "cancelled" or status == "unknown" then stop("Item selection "..status); return end
            if status ~= "dispatched" then return end
            local cursor = api.ecs.read(job.player, "keen::ecs::NetworkCursor")
            if not cursor or scalar(cursor.currentBuildingItemId) ~= command.itemId then return end
            if command.action=="remove" or command.action=="dismantle" then
                -- Dismantle can use an object interaction target in addition
                -- to the secondary transform. Never assume moving the cursor
                -- transform also changes that target.
                local object=cursor.hoveredObjectId
                local object_id=object and scalar(object.value)
                local object_type=object and object.type
                local selected_object=cursor.selectedObjectId
                local selected_id=selected_object and scalar(selected_object.value)
                local correct_target
                if command.cell then
                    correct_target=object_id==0 and selected_id==0
                else
                    correct_target=object_id and object_id>0 and object_id<=0xffffffff and
                        (object_type=="Entity" or object_type==0) and api.ecs.resolve(object_id)==command.handle and
                        (selected_id==0 or selected_id==object_id)
                end
                if not correct_target then
                    if not job.aim_message then
                        message("building",string.format("Aim at removal target %.2f, %.2f, %.2f with the building tool; waiting for the matching game cursor",table.unpack(command.position)))
                        job.aim_message=true
                    end
                    return
                end
            end
            -- Selection may take several network frames. Recheck the target
            -- immediately before input instead of relying on the old snapshot.
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
            if observe_reason then stop(observe_reason); return end
            if not confirmed then return end
            command.resultHandle = type(confirmed)=="number" and confirmed or nil
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
