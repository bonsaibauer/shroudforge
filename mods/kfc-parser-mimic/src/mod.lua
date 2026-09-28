if not loader or not loader.features or not loader.features.export then return end

-- ==========================================
-- SCRIPT CONFIGURATION
-- ==========================================
local OVERWRITE_EXISTING = false  -- Set to true to overwrite existing JSON exports; false to skip
local MAX_DEREF_DEPTH    = 8      -- Max recursion depth when unwrapping resource tables


-- ==========================================
-- SECTION 1: GLOBAL UTILITIES & JSON TOOLKIT
-- ==========================================

function sanitize_filename_tag(str)
    if not str or str == "" then return "v_unknown" end
    local short_ver = tostring(str):match("^([^%|%/%\\%_]+)")
    if not short_ver or short_ver == "" then short_ver = tostring(str) end
    local clean = short_ver:gsub('[%\\%/%:%*%?%"%<%>%|%s]', '')
    return clean ~= "" and clean or "v_unknown"
end

function get_game_version()
    if game then
        if type(game.get_version) == "function" then
            local ok, ver = pcall(game.get_version)
            if ok and ver and tostring(ver) ~= "" then return tostring(ver) end
        end
        if game.version then return tostring(game.version) end
        if game.build then return tostring(game.build) end
    end
    if eml then
        if type(eml.get_version) == "function" then
            local ok, ver = pcall(eml.get_version)
            if ok and ver and tostring(ver) ~= "" then return tostring(ver) end
        end
        if eml.version then return tostring(eml.version) end
    end
    if game and game.assets and type(game.assets.get_resources_by_type) == "function" then
        local ok, build_res = pcall(game.assets.get_resources_by_type, "keen::BuildInfoResource")
        if ok and build_res and #build_res > 0 and build_res[1].data then
            local data = build_res[1].data
            local ver = safe_get(data, "version") or safe_get(data, "build_number") or safe_get(data, "version_string")
            if ver then return tostring(ver) end
        end
    end
    return "v1076226"
end

function csv_escape(value)
    local escaped = tostring(value or "")
    escaped = escaped:gsub('"', '""')
    if escaped:find("\n") or escaped:find("\r") or escaped:find(",") or escaped:find('"') then
        escaped = '"' .. escaped .. '"'
    end
    return escaped
end

function safe_to_string(val)
    if val == nil then return "0" end
    if type(val) == "userdata" and integer and integer.to_string then
        local ok, res = pcall(integer.to_string, val)
        if ok and res then return res end
    end
    return tostring(val)
end

function safe_get(obj, key)
    if type(obj) ~= "table" and type(obj) ~= "userdata" then return nil end
    local success, val = pcall(function() return obj[key] end)
    if success then return val end
    return nil
end

function file_exists(filepath)
    if io and type(io.exists) == "function" then
        local ok, exists = pcall(io.exists, filepath)
        if ok and exists ~= nil then return exists end
    end
    if io and type(io.open) == "function" then
        local ok, handle = pcall(io.open, filepath, "r")
        if ok and handle then
            pcall(function() handle:close() end)
            return true
        end
    end
    return false
end

function deep_copy(orig, visited)
    visited = visited or {}
    if type(orig) ~= "table" then return orig end
    if visited[orig] then return "<Circular Reference>" end
    visited[orig] = true

    local copy = {}
    for k, v in pairs(orig) do
        if type(v) == "table" then
            copy[k] = deep_copy(v, visited)
        else
            copy[k] = v
        end
    end
    visited[orig] = nil
    return copy
end

-- ------------------------------------------
-- JSON SERIALIZER
-- ------------------------------------------
JSON = {}

local ESCAPE_MAP = {
    ['"']  = '\\"',
    ['\\'] = '\\\\',
    ['\b'] = '\\b',
    ['\f'] = '\\f',
    ['\n'] = '\\n',
    ['\r'] = '\\r',
    ['\t'] = '\\t'
}
for i = 0, 31 do
    local c = string.char(i)
    if not ESCAPE_MAP[c] then
        ESCAPE_MAP[c] = string.format("\\u%04x", i)
    end
end

local function json_escape_string(s)
    local str = tostring(s or "")
    return (str:gsub("[%z\1-\31\"\\\127]", ESCAPE_MAP))
end

function JSON.clean_structure(val, visited)
    visited = visited or {}
    local t = type(val)

    if t == "userdata" then
        return safe_to_string(val)
    elseif t == "table" then
        if visited[val] then return "<Circular Reference>" end
        visited[val] = true

        local cleaned = {}
        for k, v in pairs(val) do
            cleaned[k] = JSON.clean_structure(v, visited)
        end
        visited[val] = nil
        return cleaned
    else
        return val
    end
end

function JSON.encode(val, depth)
    depth = depth or 0
    local t = type(val)

    if val == nil then
        return "null"
    elseif t == "boolean" then
        return val and "true" or "false"
    elseif t == "number" then
        if val ~= val or val == math.huge or val == -math.huge then return "null" end
        local str_num = tostring(val)
        if str_num:find("[a-zA-Z#]") then return "null" end
        return str_num
    elseif t == "string" then
        return '"' .. json_escape_string(val) .. '"'
    elseif t == "userdata" then
        return '"' .. json_escape_string(safe_to_string(val)) .. '"'
    elseif t == "table" then
        if depth > 32 then return '"<Max Depth Reached>"' end

        local is_array = true
        local count = 0
        local max_key = 0

        for k, _ in pairs(val) do
            count = count + 1
            if type(k) == "number" and k > 0 and math.floor(k) == k then
                if k > max_key then max_key = k end
            else
                is_array = false
            end
        end

        if is_array and count > 0 then
            if max_key ~= count then
                is_array = false
            else
                for i = 1, count do
                    if val[i] == nil then is_array = false; break end
                end
            end
        end

        if count == 0 then return is_array and "[]" or "{}" end

        local indent = string.rep("  ", depth + 1)
        local parent_indent = string.rep("  ", depth)

        if is_array then
            local items = {}
            for i = 1, count do
                table.insert(items, indent .. JSON.encode(val[i], depth + 1))
            end
            return "[\n" .. table.concat(items, ",\n") .. "\n" .. parent_indent .. "]"
        else
            local pairs_list = {}
            local entries = {}
            for orig_k, orig_v in pairs(val) do
                table.insert(entries, { key_str = tostring(orig_k), value = orig_v })
            end
            table.sort(entries, function(a, b) return a.key_str < b.key_str end)

            for _, entry in ipairs(entries) do
                local json_k = json_escape_string(entry.key_str)
                local json_v = JSON.encode(entry.value, depth + 1)
                table.insert(pairs_list, indent .. '"' .. json_k .. '": ' .. json_v)
            end
            return "{\n" .. table.concat(pairs_list, ",\n") .. "\n" .. parent_indent .. "}"
        end
    else
        return '"<' .. json_escape_string(t) .. '>"'
    end
end

function export_json_file(filename, raw_data)
    local cleaned_data = JSON.clean_structure(raw_data)
    local formatted_json = JSON.encode(cleaned_data, 0)
    io.export(filename, formatted_json)
end


-- ==========================================
-- SECTION 2: INITIALIZATION & VERSIONING
-- ==========================================

RAW_GAME_VERSION = get_game_version()
VERSION_TAG = sanitize_filename_tag(RAW_GAME_VERSION)

print(string.format("[Version Check] Detected Game Version: '%s' (Tag: '%s')", RAW_GAME_VERSION, VERSION_TAG))


-- ==========================================
-- SECTION 3: EXPORT - RESOURCE TYPES SUMMARY
-- ==========================================

local function parse_qualified_name(qname)
    if not qname or qname == "N/A" or qname == "" then
        return "N/A", "N/A", "N/A"
    end

    local parts = {}
    for part in string.gmatch(qname, "[^:]+") do
        table.insert(parts, part)
    end

    if #parts == 1 then
        return parts[1], "N/A", parts[1]
    elseif #parts == 2 then
        return parts[1], "N/A", parts[2]
    else
        local namespace = parts[1]
        local type_name = parts[#parts]
        local middle_parts = {}
        for i = 2, #parts - 1 do
            table.insert(middle_parts, parts[i])
        end
        local category = table.concat(middle_parts, "::")
        return namespace, category, type_name
    end
end

local function export_resource_types()
    print("[Resource Types] Querying game.assets.get_resource_types()...")

    local success, resource_types = pcall(game.assets.get_resource_types)
    if not success or not resource_types then
        print("[Resource Types] ERROR: game.assets.get_resource_types() failed or returned nil.")
        return
    end

    local csv_rows = {}
    local header = "Index,ValueType,ResourceTypeHash,ResolvedName,Namespace,Category,TypeName,ResourceCount,FirstSampleGUID"
    table.insert(csv_rows, header)

    local total = #resource_types
    print(string.format("[Resource Types] Found %d registered resource types.", total))

    for i = 1, total do
        local rt = resource_types[i]
        local value_type = type(rt)
        local hash_str = safe_to_string(rt)

        local resolved_name = "N/A"
        if value_type == "table" or value_type == "userdata" then
            local ok_qname, qname = pcall(function() return rt.qualified_name or rt.name end)
            if ok_qname and qname then resolved_name = tostring(qname) end
        end

        if resolved_name == "N/A" and game.assets then
            if type(game.assets.get_type_name) == "function" then
                local ok_name, name_res = pcall(game.assets.get_type_name, rt)
                if ok_name and name_res then resolved_name = tostring(name_res) end
            elseif type(game.assets.get_resource_type_name) == "function" then
                local ok_name, name_res = pcall(game.assets.get_resource_type_name, rt)
                if ok_name and name_res then resolved_name = tostring(name_res) end
            end
        end

        local namespace, category, type_name = parse_qualified_name(resolved_name)

        local res_count = 0
        local sample_guid = "N/A"
        if game.assets and type(game.assets.get_resources_by_type) == "function" then
            local res_ok, resources = pcall(game.assets.get_resources_by_type, rt)
            if res_ok and resources and type(resources) == "table" then
                res_count = #resources
                if res_count > 0 then
                    local first_res = resources[1]
                    if type(first_res) == "table" or type(first_res) == "userdata" then
                        sample_guid = safe_to_string(first_res.guid or first_res.id)
                    end
                end
            end
        end

        local row = string.format(
            "%d,%s,%s,%s,%s,%s,%s,%d,%s",
            i,
            csv_escape(value_type),
            csv_escape(hash_str),
            csv_escape(resolved_name),
            csv_escape(namespace),
            csv_escape(category),
            csv_escape(type_name),
            res_count,
            csv_escape(sample_guid)
        )
        table.insert(csv_rows, row)
    end

    local final_csv = table.concat(csv_rows, "\n") .. "\n"
    local output_filename = string.format("0_resource_types_export_%s.csv", VERSION_TAG)

    if OVERWRITE_EXISTING or not file_exists(output_filename) then
        io.export(output_filename, final_csv)
        print(string.format("[Resource Types] Export Complete: Wrote %d rows to '%s'.", total, output_filename))
    else
        print(string.format("[Resource Types] File '%s' exists and OVERWRITE_EXISTING is false. Skipping.", output_filename))
    end
end

export_resource_types()


-- ==========================================
-- SECTION 4: KFC RAW ASSET EXPORTER
-- ==========================================

local function local_fnv1a_32_hex(str)
    local hash = 2166136261
    local bxor = (bit and bit.bxor) or (bit32 and bit32.bxor)

    for i = 1, #str do
        local byte = string.byte(str, i)
        hash = (hash * 16777619) % 4294967296

        if bxor then
            hash = bxor(hash, byte)
        else
            local a, b = hash, byte
            local res, p = 0, 1
            for _ = 1, 32 do
                local a_bit, b_bit = a % 2, b % 2
                if a_bit ~= b_bit then res = res + p end
                a, b, p = math.floor(a / 2), math.floor(b / 2), p * 2
            end
            hash = res
        end
    end
    return string.format("%08x", hash)
end

local function local_resolve_type_name(rt)
    local resolved = "N/A"
    if type(rt) == "table" or type(rt) == "userdata" then
        local ok, qname = pcall(function() return rt.qualified_name or rt.name end)
        if ok and qname then resolved = tostring(qname) end
    end

    if resolved == "N/A" and game and game.assets then
        if type(game.assets.get_type_name) == "function" then
            local ok, name_res = pcall(game.assets.get_type_name, rt)
            if ok and name_res then resolved = tostring(name_res) end
        elseif type(game.assets.get_resource_type_name) == "function" then
            local ok, name_res = pcall(game.assets.get_resource_type_name, rt)
            if ok and name_res then resolved = tostring(name_res) end
        end
    end

    if resolved == "N/A" then resolved = tostring(rt) end
    return resolved
end

local function local_extract_type_name(qname)
    if not qname or qname == "N/A" or qname == "" then return "UnknownType" end
    local clean = qname:match("([^:]+)$") or qname
    clean = string.gsub(clean, "[^%w_]", "")
    return #clean > 0 and clean or "UnknownType"
end

local function local_dereference_deep(res, max_depth)
    max_depth = max_depth or MAX_DEREF_DEPTH
    if max_depth <= 0 or (type(res) ~= "userdata" and type(res) ~= "table") then
        return res
    end

    local resource_value = res

    local ok_data, data_val = pcall(function() return res.data end)
    if ok_data and data_val ~= nil then
        resource_value = data_val
    else
        local ok_method, method_val = pcall(function() return res:get_data() end)
        if ok_method and method_val ~= nil then resource_value = method_val end
    end

    if type(resource_value) == "table" then
        local expanded = {}
        for k, v in pairs(resource_value) do
            if type(v) == "table" or type(v) == "userdata" then
                expanded[k] = local_dereference_deep(v, max_depth - 1)
            else
                expanded[k] = v
            end
        end
        return expanded
    end

    return resource_value
end

local function local_get_guid(res, unwrapped, fallback_idx)
    local candidates = { res, unwrapped }
    for _, obj in ipairs(candidates) do
        if type(obj) == "table" or type(obj) == "userdata" then
            local g = safe_get(obj, "guid") or safe_get(obj, "id")
                   or safe_get(obj, "hash") or safe_get(obj, "$guid")
            if g then
                local g_str = safe_to_string(g)
                g_str = string.gsub(g_str, "[^%w%-%_]", "")
                if #g_str > 0 then return g_str end
            end
        end
    end
    return string.format("asset_%04d", fallback_idx)
end

local function local_get_type_hash(res, unwrapped, raw_qname)
    local candidates = { res, unwrapped }
    for _, obj in ipairs(candidates) do
        if type(obj) == "table" or type(obj) == "userdata" then
            local h = safe_get(obj, "type_hash") or safe_get(obj, "hash")
            if h and type(h) == "number" then
                return string.format("%08x", h)
            end
        end
    end
    return local_fnv1a_32_hex(raw_qname)
end

local function local_get_part(res, unwrapped)
    local candidates = { res, unwrapped }
    for _, obj in ipairs(candidates) do
        if type(obj) == "table" or type(obj) == "userdata" then
            local p = safe_get(obj, "$part") or safe_get(obj, "part")
            if p and type(p) == "number" then return p end
        end
    end
    return 0
end

local function export_kfc_exact_format()
    print("[KFC Exporter] Querying registered resource types...")
    local ok, res_types = pcall(game.assets.get_resource_types)
    if not ok or not res_types then
        print("[KFC Exporter] ERROR: Could not fetch resource types!")
        return
    end

    local total_types = #res_types
    local base_dir = string.format("exported_assets_%s", VERSION_TAG)
    print(string.format("[KFC Exporter] Exporting %d resource categories into '%s/'...", total_types, base_dir))

    local total_files_saved = 0
    local total_files_skipped = 0

    for idx, res_type in ipairs(res_types) do
        local raw_qname = local_resolve_type_name(res_type)
        local type_name = local_extract_type_name(raw_qname)

        local ok_get, resources = pcall(game.assets.get_resources_by_type, res_type)
        if not ok_get or not resources then
            ok_get, resources = pcall(game.assets.get_resources_by_type, raw_qname)
        end

        if ok_get and resources and type(resources) == "table" and #resources > 0 then
            local type_saved = 0
            local type_skipped = 0

            for res_idx, res in ipairs(resources) do
                local unwrapped = local_dereference_deep(res, MAX_DEREF_DEPTH)

                local guid_str = local_get_guid(res, unwrapped, res_idx)
                local type_hash_hex = local_get_type_hash(res, unwrapped, raw_qname)
                local part_num = local_get_part(res, unwrapped)

                local kfc_filename = string.format("%s_%s_%d.json", guid_str, type_hash_hex, part_num)
                local filepath = string.format("%s/%s/%s", base_dir, type_name, kfc_filename)

                if not OVERWRITE_EXISTING and file_exists(filepath) then
                    type_skipped = type_skipped + 1
                else
                    -- Deep copy raw asset data directly (no wrapper keys added)
                    local export_value = deep_copy(unwrapped)

                    pcall(export_json_file, filepath, export_value)
                    type_saved = type_saved + 1
                end
            end

            total_files_saved = total_files_saved + type_saved
            total_files_skipped = total_files_skipped + type_skipped
            print(string.format("[KFC Exporter] [%d/%d] '%s': Saved %d, Skipped %d -> %s/%s/",
                idx, total_types, type_name, type_saved, type_skipped, base_dir, type_name))
        else
            print(string.format("[KFC Exporter] [%d/%d] '%s': 0 assets found.",
                idx, total_types, type_name))
        end
    end

    print(string.format("[KFC Exporter] Export Complete! Wrote %d JSONs (%d skipped existing).",
        total_files_saved, total_files_skipped))
end

export_kfc_exact_format()
