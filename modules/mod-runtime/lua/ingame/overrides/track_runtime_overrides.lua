local active_keys = {}

local function new(get_session)
    local self = {owned={}, blocked={}, writes=0, waiting_count=0, details={}}

    function self:begin_update()
        self.writes, self.waiting_count, self.details = 0, 0, {}
        self.touched = {}
        local current = get_session and get_session()
        if current and current ~= 0 then
            if self.session and current ~= self.session then
                if self:has_shared_ownership() then
                    self.fault = "World changed before shared values were restored. Restart the game before writing again"
                else self:reset() end
            end
            self.session = current
        end
    end

    function self:waiting(reason)
        self.waiting_count = self.waiting_count + 1
        self.details[tostring(reason)] = true
    end

    local function read(adapter)
        local ok, snapshot, reason = pcall(adapter.read)
        if not ok then return nil, tostring(snapshot) end
        return snapshot, reason
    end

    local function release(key, entry)
        if entry.claim then entry.claim:release() end
        if active_keys[key] == self then active_keys[key] = nil end
        self.owned[key] = nil
    end

    function self:write(entry, snapshot, target)
        if self.fault then self:waiting(self.fault) return false, self.fault end
        local ok, applied, actual, reason, canonical = pcall(entry.adapter.write, snapshot, target)
        if not ok then
            self.fault = "Uncertain runtime write: " .. tostring(applied)
            entry.uncertain = true
            return false, self.fault
        end
        if not applied then
            reason = tostring(reason or "Engine write is not ready")
            if reason:lower():find("uncertain", 1, true) then
                self.fault, entry.uncertain = reason, true
            end
            self:waiting(reason)
            return false, reason
        end
        if actual == nil then
            self.fault, entry.uncertain = "Uncertain runtime write: no confirmed value was returned", true
            return false, self.fault
        end
        entry.applied = actual
        if actual ~= snapshot.value then self.writes = self.writes + 1 end
        local expected = target
        if canonical ~= nil then expected = canonical end
        if actual ~= expected then
            self.fault = "Original calculation returned an unexpected value for " .. entry.setting
            return false, self.fault
        end
        return true
    end

    function self:apply(key, setting, adapter, desired, shared)
        if self.fault then return false, self.fault end
        key = adapter.key or key
        self.touched = self.touched or {}
        self.touched[key] = true
        if self.blocked[key] then
            self:waiting(self.blocked[key].reason)
            return false, self.blocked[key].reason
        end
        if active_keys[key] and active_keys[key] ~= self then
            local reason = setting .. ": another override tracker already owns this field"
            self:waiting(reason)
            return false, reason
        end
        local entry = self.owned[key]
        local fresh = entry == nil
        local claim = entry and entry.claim or adapter.claim
        if not claim then
            local reason = setting .. ": an API ownership claim is required"
            self:waiting(reason)
            return false, reason
        end
        local ok, acquired, failure = pcall(function() return claim:acquire() end)
        if not ok or not acquired then
            local reason = setting .. ": " .. tostring(ok and failure or acquired)
            self:waiting(reason)
            return false, reason
        end
        local snapshot, reason = read(adapter)
        if not snapshot then
            if not entry then claim:release() end
            local failure = setting .. ": " .. tostring(reason)
            self:waiting(failure)
            return false, failure
        end
        if entry and snapshot.value ~= entry.applied then
            release(key, entry)
            self.blocked[key] = {setting=setting, reason=setting .. ": an external change was preserved. Toggle off and on to bind again"}
            self:waiting(self.blocked[key].reason)
            return false, self.blocked[key].reason
        end
        local baseline = snapshot.value
        if entry then baseline = entry.baseline end
        local ok, target, why = pcall(desired, snapshot, baseline)
        if not ok then
            if not entry then claim:release() end
            error(target)
        end
        if target == nil then
            if not entry then claim:release() end
            local failure = setting .. ": " .. tostring(why)
            self:waiting(failure)
            return false, failure
        end
        if adapter.normalize then
            ok, target, why = pcall(adapter.normalize, snapshot, target)
            if not ok then
                if not entry then claim:release() end
                error(target)
            end
            if target == nil then
                if not entry then claim:release() end
                local failure = setting .. ": " .. tostring(why)
                self:waiting(failure)
                return false, failure
            end
        end
        if snapshot.value == target then
            if not entry then claim:release() end
            return true, nil, fresh
        end
        entry = entry or {setting=setting, baseline=snapshot.value, applied=target,
            shared=shared or adapter.shared, claim=claim}
        entry.adapter = adapter
        self.owned[key] = entry
        active_keys[key] = self
        local applied, failure = self:write(entry, snapshot, target)
        if fresh and ((applied and entry.applied == entry.baseline) or (not applied and not self.fault)) then
            release(key, entry)
        end
        return applied, failure, fresh
    end

    function self:restore(key, entry)
        if self.fault then self:waiting(self.fault) return end
        if entry.uncertain then self:waiting("Uncertain write requires inspection: " .. entry.setting) return end
        local acquired, failure = entry.claim:acquire()
        if not acquired then self:waiting("Restore " .. entry.setting .. ": " .. tostring(failure)) return end
        local snapshot, reason = read(entry.adapter)
        if not snapshot then self:waiting("Restore " .. entry.setting .. ": " .. tostring(reason)) return end
        if snapshot.value == entry.baseline or snapshot.value ~= entry.applied then
            release(key, entry)
            return
        end
        if self:write(entry, snapshot, entry.baseline) then release(key, entry) end
    end

    function self:restore_disabled(enabled)
        for key, entry in pairs(self.owned) do
            if not enabled(entry.setting) then self:restore(key, entry) end
        end
        for key, entry in pairs(self.blocked) do
            if not enabled(entry.setting) then self.blocked[key] = nil end
        end
    end

    function self:restore_all()
        for key, entry in pairs(self.owned) do self:restore(key, entry) end
    end

    function self:restore_unused()
        for key, entry in pairs(self.owned) do
            if not self.touched or not self.touched[key] then self:restore(key, entry) end
        end
    end

    function self:has_ownership() return next(self.owned) ~= nil end
    function self:has_shared_ownership()
        for _, entry in pairs(self.owned) do if entry.shared then return true end end
        return false
    end

    function self:reset()
        if self:has_shared_ownership() then
            return false, "Shared values must be restored before ownership can be reset"
        end
        for key, entry in pairs(self.owned) do release(key, entry) end
        self.owned, self.blocked, self.fault = {}, {}, nil
        return true
    end

    function self:summary(message)
        local details = {}
        for detail in pairs(self.details) do details[#details + 1] = detail end
        table.sort(details)
        if self.fault then details[#details + 1] = self.fault end
        return message .. " | writes=" .. self.writes .. (#details > 0 and (" | " .. table.concat(details, " | ")) or "")
    end

    return self
end

return {new=new}
