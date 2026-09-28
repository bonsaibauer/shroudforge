-- ============================================================================
-- Fishing Data Exporter
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

-- Guard clause: Ensure the script only executes if EML export features are available
if not (loader and loader.features and loader.features.export) then return end

-------------------------------------------------------------------------------
-- HELPER UTILITIES
-------------------------------------------------------------------------------

-- Sanitizes game version strings to ensure clean CSV output filenames without illegal characters
function sanitize_filename_tag(str)
    if not str or str == "" then return "v_unknown" end
    local short_ver = tostring(str):match("^([^%|%/%\\%_]+)")
    if not short_ver or short_ver == "" then short_ver = tostring(str) end
    local clean = short_ver:gsub('[%\\%/%:%*%?%"%<%>%|%s]', '')
    return clean ~= "" and clean or "v_unknown"
end

-- Safely retrieves a field from a table or C++ userdata object without throwing a script error
function safe_get(obj, key)
    if obj == nil or (type(obj) ~= "table" and type(obj) ~= "userdata") then return nil end
    local ok, val = pcall(function() return obj[key] end)
    return ok and val or nil
end

-- Safely converts native 64-bit integers, GUID structs, and tables to printable string IDs
function safe_to_string(val)
    if val == nil then return "0" end
    if type(val) == "userdata" and integer and integer.to_string then
        local ok, res = pcall(integer.to_string, val)
        if ok and res then return res end
    end
    if type(val) == "table" then
        return safe_to_string(val.guid or val.id or val.value or val.name)
    end
    return tostring(val)
end

-- Escapes double quotes and commas so strings safely format into standard CSV fields
function csv_escape(value)
    local escaped = tostring(value or "")
    escaped = escaped:gsub('"', '""')
    if escaped:find("\n") or escaped:find("\r") or escaped:find(",") or escaped:find('"') then
        escaped = '"' .. escaped .. '"'
    end
    return escaped
end

-- Temporary "cheat" / string-parsing method: Infers map biome tiers by analyzing words inside TemplateResource names
function infer_test_biome_from_name(template_name)
    if not template_name or template_name == "" or template_name == "N/A" then return "Global / Unknown" end
    local lower = template_name:lower()
    if lower:find("grassland") or lower:find("_t1_") then return "Springlands (T1)" end
    if lower:find("deepforest") or lower:find("forest") or lower:find("_t2_") then return "Revelwood (T2)" end
    if lower:find("steppes") or lower:find("_t3_") then return "Nomad Highlands (T3)" end
    if lower:find("desert") or lower:find("_t4_") then return "Kindlewastes (T4)" end
    if lower:find("mountains") or lower:find("_t5_") then return "Albaneve Summits (T5)" end
    if lower:find("wetlands") or lower:find("_t6_") then return "Veilwater Basin (T6)" end
    if lower:find("default") or lower:find("player") then return "Default" end
    if lower:find("z_test") then return "For Testing / Unlinked" end
    return "Global / Unlinked"
end

-------------------------------------------------------------------------------
-- UNIVERSAL ITEM RESOLVER MODULE
-------------------------------------------------------------------------------
-- Reads game memory to match raw 64-bit hashes, GUIDs, and ItemIDs to human-readable En_Us names
local ItemResolver = {
    translations = {},
    registry_names = {},
    item_info_map = {},
    id_to_item_id = {},
    cached_names = {}
}

function ItemResolver:init()
    -- 1. Stream binary localization archives to extract human-readable English text for hashes
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
                        if tag.id and tag.id.value then self.translations[tag.id.value] = tag.text end
                    end
                end
            end
        end
    end

    -- 2. Build ItemRegistryResource lookup table mapping numerical item IDs to internal debug names
    local reg_res = game.assets.get_resources_by_type("keen::ItemRegistryResource") or {}
    if reg_res and reg_res[1] and reg_res[1].data then
        local reg_items = safe_get(reg_res[1].data, "items") or safe_get(reg_res[1].data, "entries") or {}
        for _, reg_entry in ipairs(reg_items) do
            local r_id = safe_to_string(safe_get(reg_entry, "itemId") or safe_get(reg_entry, "id"))
            local r_name = safe_get(reg_entry, "name") or safe_get(reg_entry, "debugName")
            if r_id ~= "0" and r_name then
                self.registry_names[r_id] = tostring(r_name)
            end
        end
    end

    -- 3. Cache loaded keen::ItemInfo objects and create cross-indexes for GUIDs and ItemIDs
    local item_resources = game.assets.get_resources_by_type("keen::ItemInfo") or {}
    for _, item in ipairs(item_resources) do
        if item and item.data then
            local d = item.data
            local numeric_id = safe_to_string(d.itemId and d.itemId.value or "N/A")
            local guid_str = safe_to_string(item.guid)

            self.item_info_map[numeric_id] = d
            self.item_info_map[guid_str] = d

            self.id_to_item_id[numeric_id] = numeric_id
            self.id_to_item_id[guid_str] = numeric_id
        end
    end
end

-- Resolves any given handle/GUID to its corresponding integer Item ID
function ItemResolver:get_item_id(raw_handle)
    local key_str = safe_to_string(raw_handle)
    return self.id_to_item_id[key_str] or key_str
end

-- Resolves any raw handle, GUID, or ID to its English translation or fallback internal name
function ItemResolver:get_name(raw_handle)
    local key_str = safe_to_string(raw_handle)

    if self.cached_names[key_str] then
        return self.cached_names[key_str]
    end

    local item_data = self.item_info_map[key_str]

    if item_data and item_data.name then
        local raw_name = item_data.name
        local hash_key = (type(raw_name) == "table") and (raw_name.id or raw_name.hash or raw_name.value) or ((type(raw_name) == "number") and raw_name or game.guid.hash(raw_name))
        if self.translations[hash_key] and self.translations[hash_key] ~= "" then
            self.cached_names[key_str] = self.translations[hash_key]
            return self.translations[hash_key]
        end
    end

    if self.registry_names[key_str] and self.registry_names[key_str] ~= "" then
        self.cached_names[key_str] = self.registry_names[key_str]
        return self.registry_names[key_str]
    end

    if item_data then
        local default_name = safe_get(item_data, "debugName") or safe_get(item_data, "devName") or safe_get(item_data, "displayName")
        if default_name and tostring(default_name) ~= "" then
            self.cached_names[key_str] = tostring(default_name)
            return tostring(default_name)
        end
    end

    return key_str
end

-- Initialize the ItemResolver lookup tables on startup
ItemResolver:init()

-------------------------------------------------------------------------------
-- SAFE TIMESTAMP & EXPORT DRIVER
-------------------------------------------------------------------------------
-- Generates a formatted date string for unique CSV output filenames
local function get_safe_timestamp()
    local ok, ts = pcall(function()
        return os and os.date and os.date("%Y%m%d_%H%M%S")
    end)
    if ok and ts then return ts end

    -- Fallback timestamp using system uptime if 'os' library is sandboxed
    local uptime = safe_to_string(game and game.get_time and game.get_time())
    return "run_" .. uptime:gsub("%.", "_")
end

local VERSION_TAG = sanitize_filename_tag(game.version or "1076226")
local TIMESTAMP = get_safe_timestamp()

-- ============================================================================
-- TABLE 1: BIOME -> FISH SPAWN TABLES (DEEP LINK RESOLUTION)
-- Relational Link: ScatterSetObject -> TemplateResource -> FishSpawnTableResource
-- ============================================================================

-- Enum mapping for internal engine biome integers
local BIOME_ENUM_MAP = {
    [0] = "Default",
    [1] = "Grassland",
    [2] = "Desert",
    [3] = "Wetland",
    [4] = "Steppes",
    [5] = "Deepforest",
    [6] = "ColdHeights",
    [7] = "TallTrees",
    [8] = "AncientLand"
}

-- Deep extraction helper to pull raw string/GUID from direct values, tables, or wrappers
local function extract_guid(val)
    if val == nil then return nil end
    if type(val) == "string" then return val end
    if type(val) == "table" then
        return val.guid or val.id or val["$value"] or val.value or safe_to_string(val)
    end
    return safe_to_string(val)
end

-- Converts numeric biome enum values into readable biome names
local function resolve_biome_name(raw_biome)
    if raw_biome == nil then return nil end
    if type(raw_biome) == "number" then return BIOME_ENUM_MAP[raw_biome] end
    local b_str = safe_to_string(raw_biome)
    local num = tonumber(b_str)
    if num then return BIOME_ENUM_MAP[num] end
    return b_str ~= "" and b_str or nil
end

local spawn_to_template = {}
local template_by_guid = {}
local templates = game.assets.get_resources_by_type("keen::ecs::TemplateResource") or {}

-- Step 1: Scan all TemplateResources in memory for keen::ecs::FishingSpot components
-- and map their Fishing Table GUIDs back to their parent entity templates
local mapped_templates_count = 0
for _, t in ipairs(templates) do
    local t_guid = safe_to_string(t.guid)
    template_by_guid[t_guid] = t

    if t.data and t.data.components then
        for _, comp in ipairs(t.data.components) do
            local c_type = safe_get(comp, "$type") or safe_get(comp, "type")
            if c_type == "keen::ecs::FishingSpot" then
                local val = safe_get(comp, "$value") or safe_get(comp, "value") or comp
                local spawn_table_ref = safe_get(val, "spawnTable") or safe_get(val, "fishSpawnTable") or safe_get(val, "spawnTableResource")

                local spawn_guid = extract_guid(spawn_table_ref)
                if spawn_guid and spawn_guid ~= "" then
                    spawn_to_template[spawn_guid] = t
                    mapped_templates_count = mapped_templates_count + 1
                end
            end
        end
    end
end

-- ============================================================================
-- STEP 2 (WORK IN PROGRESS): ROOT SCATTER CONTAINER & NODE CRAWLER
-- Goal: Link entity templates to world scatter generation rules to find exact biomes
-- ============================================================================
local template_to_connected_biome = {}
local template_to_connected_rule = {}
local mapped_scatters_count = 0

local BIOME_ENUM_MAP = {
    [0] = "Default", [1] = "Grassland", [2] = "Desert", [3] = "Wetland",
    [4] = "Steppes", [5] = "Deepforest", [6] = "ColdHeights", [7] = "TallTrees", [8] = "AncientLand"
}

local RULE_ENUM_MAP = {
    [0] = "Nowhere", [1] = "Everywhere", [2] = "Flat", [3] = "AboveFog",
    [4] = "BelowFog", [5] = "InCave", [6] = "ForestCore", [7] = "ForestBorder",
    [8] = "Road", [9] = "RoadBorder", [10] = "Water"
}

-- Helper: Extracts entity references, biomes, and rules from procedural scatter nodes
local function process_scatter_node(node)
    if type(node) ~= "table" then return end

    local e_ref = safe_get(node, "entity") or safe_get(node, "model")
    local raw_biome = safe_get(node, "biome")
    local raw_rule = safe_get(node, "rule")

    if e_ref then
        local e_guid = extract_guid(e_ref)
        if e_guid then
            local b_name = (type(raw_biome) == "number" and BIOME_ENUM_MAP[raw_biome]) or (type(raw_biome) == "string" and raw_biome) or nil
            local r_name = (type(raw_rule) == "number" and RULE_ENUM_MAP[raw_rule]) or nil

            if b_name or r_name then
                if b_name then template_to_connected_biome[e_guid] = b_name end
                if r_name then template_to_connected_rule[e_guid] = r_name end
                mapped_scatters_count = mapped_scatters_count + 1
            end
        end
    end
end

-- Recursively crawls nested scatter container structures up to 6 levels deep
local function scan_table_recursive(tbl, depth)
    if not tbl or type(tbl) ~= "table" or (depth and depth > 6) then return end

    if safe_get(tbl, "entity") and (safe_get(tbl, "biome") or safe_get(tbl, "rule")) then
        process_scatter_node(tbl)
    end

    for _, v in pairs(tbl) do
        if type(v) == "table" then
            scan_table_recursive(v, (depth or 0) + 1)
        end
    end
end

-- Container types that define procedural world spawner rules
local root_container_types = {
    "keen::ScatterSet3Resource",
    "keen::ScatterSetObject",
    "keen::SceneProceduralLayer",
    "keen::SceneScatterData"
}

-- Iterate through all spawner containers loaded in RAM
for _, container_type in ipairs(root_container_types) do
    local ok, containers = pcall(game.assets.get_resources_by_type, container_type)
    if ok and containers then
        print(string.format("[DEBUG] Container '%s': Found %d assets", container_type, #containers))
        for _, res in ipairs(containers) do
            if res and res.data then
                scan_table_recursive(res.data, 0)
            end
        end
    end
end

print(string.format("[DEBUG] Mapped %d FishingSpot templates | Mapped %d ScatterNode rules", mapped_templates_count, mapped_scatters_count))

-- Step 3: Iterate all FishSpawnTableResource objects and compile output rows
local fish_tables = game.assets.get_resources_by_type("keen::fishing::FishSpawnTableResource") or {}
local rows_t1 = {}

for _, res in ipairs(fish_tables) do
    local r_guid = safe_to_string(res.guid)
    local template = spawn_to_template[r_guid]

    -- Fallback: check if spawn table has direct reference to owner template
    if not template and res.data then
        local owner_ref = extract_guid(safe_get(res.data, "ownerTemplate") or safe_get(res.data, "template"))
        if owner_ref then template = template_by_guid[owner_ref] end
    end

    local t_guid = template and safe_to_string(template.guid) or "N/A"
    local t_name = template and template.data and template.data.name or "Default/Player"

    -- Populate name-inferred biome (cheat method)
    local name_biome = infer_test_biome_from_name(t_name)

    -- Attempt graph-connected biome resolution (Part 2)
    local graph_biome = template_to_connected_biome[t_guid]
                       or template_to_connected_biome[t_name]
                       or template_to_connected_biome[r_guid]

    if not graph_biome or graph_biome == "" then
        local raw_b = safe_get(res.data, "biome") or safe_get(res.data, "biomeType")
        graph_biome = resolve_biome_name(raw_b) or "Unlinked / Dynamic"
    end

    -- Mathematical probability distribution calculations for Fish, Shoes, and Trash
    local d = res.data
    local trash_pct = (d.canSpawnTrash and d.trashEntry) and d.trashEntry.percentage or 0
    local shoe_pct = 0
    if d.shoeEntries then for _, s in ipairs(d.shoeEntries) do shoe_pct = shoe_pct + (s.percentage or 0) end end
    local fish_weight = 0
    if d.fishEntries then for _, f in ipairs(d.fishEntries) do fish_weight = fish_weight + (f.weight or 0) end end
    local remaining_pool = math.max(0.0, 1.0 - trash_pct - shoe_pct)

    -- Export Trash entries
    if d.canSpawnTrash and d.trashEntry then
        table.insert(rows_t1, {r_guid, t_guid, t_name, "Trash", "N/A", "Trash Table Sub-Loot", "N/A", string.format("%.2f%%", trash_pct * 100), safe_to_string(d.trashEntry.timeFrame or "AllDay"), name_biome, graph_biome})
    end
    -- Export Shoe entries
    if d.shoeEntries then
        for _, s in ipairs(d.shoeEntries) do
            local item_handle = safe_to_string(s.item and (s.item.value or s.item))
            table.insert(rows_t1, {r_guid, t_guid, t_name, "Shoe", ItemResolver:get_item_id(item_handle), ItemResolver:get_name(item_handle), "N/A", string.format("%.2f%%", (s.percentage or 0) * 100), safe_to_string(s.timeFrame or "AllDay"), name_biome, graph_biome})
        end
    end
    -- Export Fish entries and calculate global drop percentage
    if d.fishEntries then
        for _, f in ipairs(d.fishEntries) do
            local item_handle = safe_to_string(f.item and (f.item.value or f.item))
            local chance = (fish_weight > 0) and (((f.weight or 0) / fish_weight) * remaining_pool) or 0
            table.insert(rows_t1, {r_guid, t_guid, t_name, "Fish", ItemResolver:get_item_id(item_handle), ItemResolver:get_name(item_handle), safe_to_string(f.weight or 0), string.format("%.2f%%", chance * 100), safe_to_string(f.timeFrame or "AllDay"), name_biome, graph_biome})
        end
    end
end

-- Export Table 1 CSV
local csv1 = "TableGUID,TemplateGUID,TemplateName,Type,ItemID,ItemName,Weight,GlobalChance,TimeFrame,NameBasedBiome,ConnectedBiome\n"
for _, r in ipairs(rows_t1) do
    csv1 = csv1 .. csv_escape(r[1]) .. "," .. csv_escape(r[2]) .. "," .. csv_escape(r[3]) .. "," .. csv_escape(r[4]) .. "," .. csv_escape(r[5]) .. "," .. csv_escape(r[6]) .. "," .. csv_escape(r[7]) .. "," .. csv_escape(r[8]) .. "," .. csv_escape(r[9]) .. "," .. csv_escape(r[10]) .. "," .. csv_escape(r[11]) .. "\n"
end
io.export(string.format("1_fish_spawn_tables_final_%s_%s.csv", VERSION_TAG, TIMESTAMP), csv1)

-- ============================================================================
-- TABLE 2: FISHING ROD INFORMATION
-- ============================================================================
local STRENGTH_GUID = "af2e21f2-09e1-4e79-9654-3199b550d1ab"
local ENDURANCE_GUID = "f0dd579e-91f2-465e-805a-094f06d1633a"

-- Helper to parse complex variant impact trees on item setups
local function extract_impact_value_by_guid(impacts_root, target_guid)
    if not impacts_root then return "0" end
    local target_lower = target_guid:lower()

    local function resolve_node_val(node)
        if not node then return nil end

        local cfg = safe_get(node, "configGuid") or safe_get(node, "config_guid") or safe_get(node, "config") or safe_get(node, "id")
        local cfg_str = safe_to_string(cfg and (cfg.guid or cfg.value or cfg) or cfg):lower()

        if cfg_str:find(target_lower, 1, true) then
            local val = safe_get(node, "value") or safe_get(node, "$value") or safe_get(node, "amount") or safe_get(node, "modifier")
            if type(val) == "table" or type(val) == "userdata" then
                val = safe_get(val, "value") or safe_get(val, "$value")
            end
            if val ~= nil then return safe_to_string(val) end
        end
        return nil
    end

    local function traverse_array(arr)
        if not arr then return nil end
        local items = safe_get(arr, "$value") or safe_get(arr, "entries") or arr
        if type(items) ~= "table" and type(items) ~= "userdata" then return nil end

        for _, entry in pairs(items) do
            local res = resolve_node_val(entry)
            if res then return res end

            local variant_inner = safe_get(entry, "$value") or safe_get(entry, "data") or safe_get(entry, "value")
            res = resolve_node_val(variant_inner)
            if res then return res end
        end
        return nil
    end

    local simple_arr = safe_get(impacts_root, "simple")
    local res = traverse_array(simple_arr)
    if res then return res end

    local scaled_arr = safe_get(impacts_root, "scaled")
    res = traverse_array(scaled_arr)
    if res then return res end

    return traverse_array(impacts_root) or "0"
end

local rows_t2 = {}
local processed_guids = {}
local item_resources = game.assets.get_resources_by_type("keen::ItemInfo") or {}

-- Extract Fishing Rod stats from keen::ItemInfo resources
for _, item in ipairs(item_resources) do
    if item and item.data then
        local d = item.data
        local i_guid = safe_to_string(item.guid)
        local is_rod = false
        local shoe_factor = "1.0"
        local trash_factor = "1.0"

        local rod_setup = safe_get(d, "fishingRodItemSetup")
        if rod_setup then
            local is_set = safe_get(rod_setup, "isSet")
            if is_set == true or safe_get(rod_setup, "$type") then
                is_rod = true
                shoe_factor = safe_to_string(safe_get(rod_setup, "shoeWeightFactor") or 1.0)
                trash_factor = safe_to_string(safe_get(rod_setup, "trashWeightFactor") or 1.0)
            end
        end

        if is_rod and not processed_guids[i_guid] then
            processed_guids[i_guid] = true

            local r_item_id = ItemResolver:get_item_id(i_guid)
            local r_name = ItemResolver:get_name(i_guid)

            local impacts_data = safe_get(d, "impactValues")
            local f_strength = extract_impact_value_by_guid(impacts_data, STRENGTH_GUID)
            local f_endurance = extract_impact_value_by_guid(impacts_data, ENDURANCE_GUID)

            table.insert(rows_t2, {i_guid, r_item_id, r_name, shoe_factor, trash_factor, f_strength, f_endurance})
        end
    end
end

-- Export Table 2 CSV
local csv2 = "RodItemGUID,RodItemID,RodName,ShoeWeightFactor,TrashWeightFactor,FishingStrength,FishingEndurance\n"
for _, r in ipairs(rows_t2) do
    csv2 = csv2 .. csv_escape(r[1]) .. "," .. csv_escape(r[2]) .. "," .. csv_escape(r[3]) .. "," .. csv_escape(r[4]) .. "," .. csv_escape(r[5]) .. "," .. csv_escape(r[6]) .. "," .. csv_escape(r[7]) .. "\n"
end
io.export(string.format("2_fishing_rods_final_%s_%s.csv", VERSION_TAG, TIMESTAMP), csv2)

-- ============================================================================
-- TABLE 3: BAIT, TRASH FACTORS & FISH BOOSTER
-- ============================================================================
-- Cache all TrashLootTableResource objects
local trash_table_cache = {}
local trash_resources = game.assets.get_resources_by_type("keen::fishing::TrashLootTableResource") or {}
for _, tt in ipairs(trash_resources) do
    if tt and tt.data then
        trash_table_cache[safe_to_string(tt.guid)] = tt.data
    end
end

local rows_t3 = {}

-- Extract Bait stats and sub-trash table probability from keen::ItemInfo
for _, item in ipairs(item_resources) do
    if item and item.data then
        local d = item.data
        local bait_setup = safe_get(d, "baitItemSetup")

        local is_set = safe_get(bait_setup, "isSet")
        if is_set == true or safe_get(bait_setup, "$type") then
            local b_guid = safe_to_string(item.guid)
            local b_item_id = ItemResolver:get_item_id(b_guid)
            local b_name = ItemResolver:get_name(b_guid)

            local raw_booster = safe_get(bait_setup, "fishBooster")
            local fish_booster = "N/A"
            if raw_booster ~= nil then
                local b_val = safe_to_string(safe_get(raw_booster, "value") or safe_get(raw_booster, "$value") or raw_booster)
                if b_val ~= "0" and b_val ~= "" then fish_booster = b_val end
            end

            local trash_table_ref = safe_get(bait_setup, "trashLootTable")
            local trash_table_guid = safe_to_string(trash_table_ref and (trash_table_ref.guid or trash_table_ref.value or trash_table_ref))

            local trash_data = trash_table_cache[trash_table_guid]
            if trash_data and trash_data.entries then
                local total_w = 0
                for _, entry in ipairs(trash_data.entries) do total_w = total_w + (entry.weight or 0) end

                for _, entry in ipairs(trash_data.entries) do
                    local t_item_handle = safe_to_string(entry.item and entry.item.value)
                    local t_item_id = ItemResolver:get_item_id(t_item_handle)
                    local t_item_name = ItemResolver:get_name(t_item_handle)
                    local weight = entry.weight or 0
                    local rate = (total_w > 0) and string.format("%.2f%%", (weight / total_w) * 100) or "0.00%"
                    local time_frame = safe_to_string(entry.timeFrame or "AllDay")

                    table.insert(rows_t3, {b_guid, b_item_id, b_name, fish_booster, trash_table_guid, t_item_id, t_item_name, safe_to_string(weight), rate, time_frame})
                end
            else
                table.insert(rows_t3, {b_guid, b_item_id, b_name, fish_booster, trash_table_guid ~= "0" and trash_table_guid or "None", "N/A", "None", "0", "0.00%", "AllDay"})
            end
        end
    end
end

-- Export Table 3 CSV
local csv3 = "BaitItemGUID,BaitItemID,BaitName,FishBooster,TrashLootTable,TrashItemID,TrashItemName,ItemWeight,EffectiveDropRate,TimeFrame\n"
for _, r in ipairs(rows_t3) do
    csv3 = csv3 .. csv_escape(r[1]) .. "," .. csv_escape(r[2]) .. "," .. csv_escape(r[3]) .. "," .. csv_escape(r[4]) .. "," .. csv_escape(r[5]) .. "," .. csv_escape(r[6]) .. "," .. csv_escape(r[7]) .. "," .. csv_escape(r[8]) .. "," .. csv_escape(r[9]) .. "," .. csv_escape(r[10]) .. "\n"
end
io.export(string.format("3_fishing_bait_and_trash_final_%s_%s.csv", VERSION_TAG, TIMESTAMP), csv3)

print("[Fishing Exporter Mod] Executed! Created Tables 1, 2, and 3.")
