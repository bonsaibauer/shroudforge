-- Bounded JSON codec for blueprint files. Arrays retain their kind, including [].
local json = {}
local array_kind, object_kind = {}, {}
json.null = {}
function json.array(value) return setmetatable(value or {}, array_kind) end
function json.object(value) return setmetatable(value or {}, object_kind) end
function json.is_array(value) return type(value) == "table" and getmetatable(value) == array_kind end
function json.is_object(value) return type(value) == "table" and getmetatable(value) == object_kind end

local function finite(value)
    return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

function json.decode(text)
    local position, length, nodes = 1, #text, 0
    local function fail() error("invalid blueprint JSON at byte " .. position, 0) end
    local function whitespace()
        while position <= length and text:sub(position, position):match("[ \t\r\n]") do position = position + 1 end
    end
    local function hex4()
        local digits = text:sub(position, position + 3)
        if #digits ~= 4 or not digits:match("^%x+$") then fail() end
        position = position + 4
        return tonumber(digits, 16)
    end
    local function unicode(code)
        if code < 128 then return string.char(code) end
        if code < 2048 then return string.char(192 + math.floor(code / 64), 128 + code % 64) end
        if code < 65536 then return string.char(224 + math.floor(code / 4096), 128 + math.floor(code / 64) % 64, 128 + code % 64) end
        return string.char(240 + math.floor(code / 262144), 128 + math.floor(code / 4096) % 64,
            128 + math.floor(code / 64) % 64, 128 + code % 64)
    end
    local function string_value()
        if text:sub(position, position) ~= '"' then fail() end
        position = position + 1
        local parts, start = {}, position
        while position <= length do
            local byte = text:byte(position)
            if byte == 34 then
                parts[#parts + 1] = text:sub(start, position - 1)
                position = position + 1
                return table.concat(parts)
            elseif byte == 92 then
                parts[#parts + 1] = text:sub(start, position - 1)
                position = position + 1
                local escape = text:sub(position, position)
                position = position + 1
                if escape == "u" then
                    local code = hex4()
                    if code >= 0xd800 and code <= 0xdbff then
                        if text:sub(position, position + 1) ~= "\\u" then fail() end
                        position = position + 2
                        local low = hex4()
                        if low < 0xdc00 or low > 0xdfff then fail() end
                        code = 65536 + (code - 0xd800) * 1024 + low - 0xdc00
                    elseif code >= 0xdc00 and code <= 0xdfff then fail() end
                    parts[#parts + 1] = unicode(code)
                else
                    local escapes = {['"']='"', ['\\']='\\', ['/']='/', b='\b', f='\f', n='\n', r='\r', t='\t'}
                    if not escapes[escape] then fail() end
                    parts[#parts + 1] = escapes[escape]
                end
                start = position
            elseif byte < 32 then fail()
            else position = position + 1 end
        end
        fail()
    end
    local parse
    parse = function(depth)
        nodes = nodes + 1
        if depth > 8 or nodes > 24000000 then fail() end
        whitespace()
        local first = text:sub(position, position)
        if first == '"' then return string_value() end
        if first == "[" or first == "{" then
            local array = first == "["
            local result = array and json.array() or json.object()
            local closing = array and "]" or "}"
            position = position + 1
            whitespace()
            if text:sub(position, position) == closing then position = position + 1; return result end
            local count = 0
            while true do
                count = count + 1
                if count > 1000000 then fail() end
                local key = count
                if not array then
                    whitespace()
                    key = string_value()
                    if result[key] ~= nil then fail() end
                    whitespace()
                    if text:sub(position, position) ~= ":" then fail() end
                    position = position + 1
                end
                result[key] = parse(depth + 1)
                whitespace()
                local separator = text:sub(position, position)
                position = position + 1
                if separator == closing then return result end
                if separator ~= "," then fail() end
            end
        end
        for token, value in pairs({["true"]=true, ["false"]=false, ["null"]=json.null}) do
            if text:sub(position, position + #token - 1) == token then position = position + #token; return value end
        end
        local start = position
        if first == "-" then position = position + 1 end
        local digit = text:sub(position, position)
        if digit == "0" then position = position + 1
        elseif digit:match("[1-9]") then
            repeat position = position + 1 until not text:sub(position, position):match("%d")
        else fail() end
        if text:sub(position, position) == "." then
            position = position + 1
            if not text:sub(position, position):match("%d") then fail() end
            repeat position = position + 1 until not text:sub(position, position):match("%d")
        end
        if text:sub(position, position):match("[eE]") then
            position = position + 1
            if text:sub(position, position):match("[+-]") then position = position + 1 end
            if not text:sub(position, position):match("%d") then fail() end
            repeat position = position + 1 until not text:sub(position, position):match("%d")
        end
        local value = tonumber(text:sub(start, position - 1))
        if not finite(value) then fail() end
        return value
    end
    local result = parse(0)
    whitespace()
    if position <= length then fail() end
    return result
end

function json.encode(value)
    local parts = {}
    local function add(text) parts[#parts + 1] = text end
    local function quote(text)
        add('"' .. text:gsub('[%z\1-\31\\"]', function(character)
            return string.format("\\u%04x", character:byte())
        end) .. '"')
    end
    local function write(item, depth)
        if depth > 8 then error("blueprint JSON is too deeply nested", 0) end
        if type(item) == "string" then quote(item)
        elseif finite(item) then add(string.format("%.17g", item))
        elseif json.is_array(item) then
            add("[")
            for index, element in ipairs(item) do if index > 1 then add(",") end; write(element, depth + 1) end
            add("]")
        elseif json.is_object(item) then
            add("{")
            local keys = {}
            for key in pairs(item) do keys[#keys + 1] = key end
            table.sort(keys)
            for index, key in ipairs(keys) do
                if index > 1 then add(",") end
                quote(key); add(":"); write(item[key], depth + 1)
            end
            add("}")
        else error("unsupported blueprint JSON value", 0) end
    end
    write(value, 0)
    return table.concat(parts)
end

return json
