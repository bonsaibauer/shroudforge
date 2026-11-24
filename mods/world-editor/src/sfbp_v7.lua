-- Executable SFBP V7 format validation and decoding.
local schema = {marker = "SHROUDFORGE_WORLD_BLUEPRINT_V7"}

function schema.decode(editor, content)
    local lines = {}
    for line in (content .. "\n"):gmatch("([^\n]*)\n") do lines[#lines + 1] = line end
    local version = lines[1]
    local rotation_axis, extent_text, kind = lines[2], lines[3], lines[4]
    local dimensions, offset_text, cell_size_text, encoded, coverage_encoded = lines[5], lines[6], lines[7], lines[8], lines[9]
    local has_voxels = kind == "voxel"
    if version ~= schema.marker or
       (rotation_axis ~= "x" and rotation_axis ~= "y" and rotation_axis ~= "z") or
       (kind ~= "voxel" and kind ~= "props") or not extent_text or not dimensions or not offset_text or
       not cell_size_text or not encoded or not coverage_encoded then
        shroudforge.log.warn("World Editor: blueprint format is invalid")
        return
    end
    local function parse_triplet(line)
        local a, b, c = line:match("^([^,]+),([^,]+),([^,]+)$")
        a, b, c = tonumber(a), tonumber(b), tonumber(c)
        if not editor.finite_number(a) or not editor.finite_number(b) or not editor.finite_number(c) then return nil end
        return {a, b, c}
    end
    local extent = parse_triplet(extent_text)
    if not extent or extent[1] < 0 or extent[2] < 0 or extent[3] < 0 or
       (kind == "props" and (extent[1] <= 0 or extent[2] <= 0 or extent[3] <= 0)) then
        shroudforge.log.warn("World Editor: blueprint extent is invalid")
        return
    end
    local cells, coverage = {}, {}
    local sx, sy, sz, voxel_offset, cell_size
    if has_voxels then
        local dimensions_values = parse_triplet(dimensions)
        voxel_offset = parse_triplet(offset_text)
        cell_size = parse_triplet(cell_size_text)
        if not dimensions_values or not voxel_offset or not cell_size or
           cell_size[1] <= 0 or cell_size[2] <= 0 or cell_size[3] <= 0 then
            shroudforge.log.warn("World Editor: voxel channel metadata is invalid")
            return
        end
        sx, sy, sz = dimensions_values[1], dimensions_values[2], dimensions_values[3]
        if sx % 1 ~= 0 or sy % 1 ~= 0 or sz % 1 ~= 0 or sx < 1 or sy < 1 or sz < 1 or sx * sy * sz > 65536 then
            shroudforge.log.warn("World Editor: blueprint dimensions exceed supported limits")
            return
        end
        for cell in encoded:gmatch("[^,]+") do
            local value = tonumber(cell)
            if not value or value % 1 ~= 0 or value < 0 or value > 65535 then
                shroudforge.log.warn("World Editor: blueprint contains an invalid voxel cell")
                return
            end
            cells[#cells + 1] = value
        end
        if #cells ~= sx * sy * sz then
            shroudforge.log.warn("World Editor: blueprint cell count does not match its dimensions")
            return
        end
        for state in coverage_encoded:gmatch("[^,]+") do
            local value = tonumber(state)
            if not value or value % 1 ~= 0 or value < 0 or value > 2 then
                shroudforge.log.warn("World Editor: blueprint contains invalid voxel coverage")
                return
            end
            coverage[#coverage + 1] = value
        end
        if #coverage ~= #cells then
            shroudforge.log.warn("World Editor: blueprint coverage count does not match its dimensions")
            return
        end
        for index, state in ipairs(coverage) do
            if state == 2 and cells[index] == 0 then
                shroudforge.log.warn("World Editor: occupied blueprint cells must have a non-zero value")
                return
            elseif state ~= 2 then
                cells[index] = 0
            end
        end
    elseif dimensions ~= "-" or offset_text ~= "-" or cell_size_text ~= "-" or encoded ~= "-" or coverage_encoded ~= "-" then
        shroudforge.log.warn("World Editor: props-only blueprint has unexpected voxel data")
        return
    end
    local props = {}
    local count_line = 10
    local count = tonumber(lines[count_line])
    local maximum = editor.maximum_copyable_props()
    if not count or count % 1 ~= 0 or count < 0 or count > maximum or count > editor.maximum_supported_props or #lines ~= count_line + count then
        shroudforge.log.warn("World Editor: blueprint prop count is invalid")
        return
    end
    local recipes = count > 0 and editor.resolve_placeable_items() or {}
    for index = 1, count do
        local fields = {}
        local field_count, invalid_field = 0, false
        for field in (lines[index + count_line] .. ","):gmatch("(.-),") do
            field_count = field_count + 1
            if field_count <= 11 then
                fields[field_count] = tonumber(field)
                if not editor.finite_number(fields[field_count]) then invalid_field = true end
            else
                fields[field_count] = field
            end
        end
        if field_count ~= 13 or invalid_field or not fields[1] or fields[1] < 1 or
           fields[1] % 1 ~= 0 or not recipes or not recipes[fields[1]] then
            shroudforge.log.warn("World Editor: blueprint references an item without a current placement recipe")
            return
        end
        for field = 2, 11 do
            if not editor.finite_number(fields[field]) then
                shroudforge.log.warn("World Editor: blueprint has invalid prop transform data")
                return
            end
        end
        local rotation_norm = fields[5]^2 + fields[6]^2 + fields[7]^2 + fields[8]^2
        if not editor.finite_number(rotation_norm) or rotation_norm < 1e-12 then
            shroudforge.log.warn("World Editor: blueprint contains a zero-length prop rotation")
            return
        end
        local template_high, template_low = fields[12], fields[13]
        if type(template_high) ~= "string" or type(template_low) ~= "string" or
           not template_high:match("^%x+$") or not template_low:match("^%x+$") or
           #template_high > 16 or #template_low > 16 or
           (template_high:match("^0+$") and template_low:match("^0+$")) then
            shroudforge.log.warn("World Editor: blueprint prop has an invalid native template UUID")
            return
        end
        template_high, template_low = template_high:lower(), template_low:lower()
        props[#props + 1] = {itemId = fields[1], templateUuidHighHex = template_high,
            templateUuidLowHex = template_low, x = fields[2], y = fields[3], z = fields[4],
            qx = fields[5], qy = fields[6], qz = fields[7], qw = fields[8],
            sx = fields[9], sy = fields[10], sz = fields[11]}
    end
    local blueprint = {region = {sx = sx, sy = sy, sz = sz, rotationAxis = rotation_axis},
        cells = has_voxels and cells or nil, props = props, hasVoxels = has_voxels,
        coverage = has_voxels and coverage or nil,
        extent = extent, voxelOffset = voxel_offset,
        cellSize = cell_size}
    return blueprint, #cells, #props
end

return schema
