-- ============================================================================
-- Item Exporter and English Translator
-- File: src/mod.lua
-- ----------------------------------------------------------------------------
-- ATTRIBUTION & ACKNOWLEDGEMENTS:
-- Parts of this code were adapted from Brabb3l's kfc-parser examples:
-- * Translations logic adapted from:
--   https://github.com/Brabb3l/kfc-parser/blob/main/examples/translations/get_translations.lua
-- * Base item list/export structure adapted from:
--   https://github.com/Brabb3l/kfc-parser/blob/main/examples/basic/export_item_list.lua
-- * Major Addition: Added full En-Us localized translation lookup and mapping
--   directly into the item info list export.
-- ============================================================================

-- Ensure that the mod loader and export feature are available before running the script
if not (loader and loader.features and loader.features.export) then return end

-------------------------------------------------------------------------------
-- UTILITY FUNCTIONS
-------------------------------------------------------------------------------

-- Sanitizes raw version strings into safe strings for file naming (removes illegal path characters)
function sanitize_filename_tag(str)
    if not str or str == "" then return "v_unknown" end
    local short_ver = tostring(str):match("^([^%|%/%\\%_]+)")
    if not short_ver or short_ver == "" then short_ver = tostring(str) end
    local clean = short_ver:gsub('[%\\%/%:%*%?%"%<%>%|%s]', '')
    return clean ~= "" and clean or "v_unknown"
end

-- Safely converts a value to a string, handling internal userdata integer representations and JSON-like wrappers correctly
function safe_to_string(val)
    if val == nil then return "" end
    if type(val) == "userdata" and integer and integer.to_string then
        local ok, res = pcall(integer.to_string, val)
        if ok and res then return res end
    end
    if type(val) == "table" then
        return safe_to_string(val.guid or val.id or val.value or val.name)
    end

    local str = tostring(val)
    local json_digits = str:match('"value"%s*:%s*(%d+)') or str:match('value%s*:%s*(%d+)')
    if json_digits then return json_digits end

    return str
end

-- Escapes special characters (quotes, commas, newlines) to ensure proper CSV compliance
function csv_escape(value)
    local escaped = tostring(value or "")
    escaped = escaped:gsub('"', '""')
    if escaped:find("\n") or escaped:find("\r") or escaped:find(",") or escaped:find('"') then
        escaped = '"' .. escaped .. '"'
    end
    return escaped
end

-- Retrieves the active game version from available runtime assets or functions with a fallback
function get_game_version()
    if game then
        if type(game.get_version) == "function" then
            local ok, ver = pcall(game.get_version)
            if ok and ver and tostring(ver) ~= "" then return tostring(ver) end
        end
        if game.version then return tostring(game.version) end
    end
    return "1076226"
end

-- Initialize version variables for dynamic file naming
local RAW_VERSION = get_game_version()
local VERSION_TAG = sanitize_filename_tag(RAW_VERSION)

-------------------------------------------------------------------------------
-- EXPORT 1: GET TRANSLATIONS (Adapted from Brabb3l's get_translations.lua)
-- Locates the En_Us localization resource file, reads its content buffer,
-- and builds a dictionary table mapping hash keys to localized text strings.
-------------------------------------------------------------------------------
local translations = {}
local loca_res = game.assets.get_resources_by_type("keen::LocaTagCollectionResource")
if loca_res and #loca_res > 0 and loca_res[1].data then
    local loca_primary = loca_res[1].data
    local content_hash = nil
    if loca_primary.languages then
        for _, loc in ipairs(loca_primary.languages) do
            if loc.language == "En_Us" then content_hash = loc.dataHash; break end
        end
    end
    if not content_hash then content_hash = loca_primary.keenglishDataHash end
    if content_hash then
        local ok_buf, buf = pcall(function() return game.assets.get_content(game.guid.from_content_hash(content_hash)):read_data() end)
        if ok_buf and buf then
            local ok_data, loc_data = pcall(function() return buf:read_resource("keen::LocaTagCollectionResourceData") end)
            if ok_data and loc_data and loc_data.tags then
                for _, tag in ipairs(loc_data.tags) do
                    local t_id = safe_to_string(tag.id)
                    if t_id ~= "" and t_id ~= "0" and tag.text then
                        translations[t_id] = tag.text
                    end
                end
            end
        end
    end
end

-------------------------------------------------------------------------------
-- EXPORT 2: ITEM INFO EXPORT FULL (Adapted from export_item_list.lua)
-- Gathers all `keen::ItemInfo` game assets, extracts their properties, computes name hashes,
-- resolves their corresponding English translation text, and exports them to a comprehensive CSV file.
-------------------------------------------------------------------------------
local all_items = game.assets.get_resources_by_type("keen::ItemInfo") or {}
local csv_rows_items = {"ItemGUID,ItemID,NameHashKey,ResolvedName,DebugName"}
for _, item in ipairs(all_items) do
    if item and item.data then
        local d = item.data
        local item_id = (d.itemId and d.itemId.value) or "UnknownID"
        local raw_name = d.name
        local hash_key = (type(raw_name) == "table" and (raw_name.id or raw_name.hash or raw_name.value))
            or ((type(raw_name) == "userdata" or type(raw_name) == "number") and raw_name)
            or (game.guid and game.guid.hash and game.guid.hash(raw_name))
        local hash_str = safe_to_string(hash_key)
        local resolved = translations[hash_str] or "N/A"
        table.insert(csv_rows_items, csv_escape(safe_to_string(item.guid)) .. "," .. csv_escape(safe_to_string(item_id)) .. "," .. csv_escape(hash_str) .. "," .. csv_escape(resolved) .. "," .. csv_escape(d.debugName or "Unknown"))
    end
end
io.export(string.format("item_info_export_FULL_%s.csv", VERSION_TAG), table.concat(csv_rows_items, "\n") .. "\n")

-------------------------------------------------------------------------------
-- EXPORT 3: TRANSLATION ENUS EXPORT FULL
-- Iterates through the compiled `translations` dictionary and dumps all key-value pairs
-- into a standalone localized text CSV file.
-------------------------------------------------------------------------------
local csv_rows_trans = {"HashKey,TranslatedText"}
for hash_val, text_val in pairs(translations) do
    table.insert(csv_rows_trans, csv_escape(safe_to_string(hash_val)) .. "," .. csv_escape(text_val))
end
io.export(string.format("translation_enus_export_FULL_%s.csv", VERSION_TAG), table.concat(csv_rows_trans, "\n") .. "\n")

-- Log completion status to the console
print("[Item Exporter and English Translator] Complete! Exported 3 base files with version tag: " .. VERSION_TAG)
