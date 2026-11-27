local json = require("sfbp_json")
local schema = {url = "https://shroudforge.net/schemas/sfbp/8/manifest.schema.json"}

local function require_value(valid, reason)
    if not valid then error(reason, 0) end
end

local function object(value, allowed)
    require_value(json.is_object(value), "blueprint field must be an object")
    for key in pairs(value) do require_value(allowed[key], "unknown blueprint field " .. key) end
end

local function vector(editor, value, count)
    require_value(json.is_array(value) and #value == count, "invalid blueprint vector length")
    for _, number in ipairs(value) do require_value(editor.finite_number(number), "nonfinite blueprint value") end
    return value
end

local function validate(editor, value, check_recipes)
    object(value, {['$schema']=true, formatVersion=true, rotationAxis=true, extent=true, voxels=true, props=true})
    require_value(value.formatVersion == 8 and (value['$schema'] == nil or value['$schema'] == schema.url), "unsupported blueprint format")
    require_value(value.rotationAxis == "x" or value.rotationAxis == "y" or value.rotationAxis == "z", "invalid blueprint rotation axis")
    local extent = vector(editor, value.extent, 3)
    for _, number in ipairs(extent) do require_value(number >= 0 and (value.voxels ~= nil or number > 0), "invalid blueprint extent") end
    local voxels = value.voxels
    local region = {rotationAxis=value.rotationAxis}
    if voxels ~= nil then
        object(voxels, {dimensions=true, offset=true, cellSize=true, cells=true, coverage=true})
        local dimensions = vector(editor, voxels.dimensions, 3)
        local count = 1
        for _, size in ipairs(dimensions) do
            require_value(size % 1 == 0 and size >= 1 and size <= 65536, "invalid voxel dimensions")
            count = count * size
        end
        require_value(count <= 65536, "blueprint exceeds the voxel cell limit")
        vector(editor, voxels.offset, 3)
        for _, size in ipairs(vector(editor, voxels.cellSize, 3)) do require_value(size > 0, "invalid voxel cell size") end
        require_value(json.is_array(voxels.cells) and #voxels.cells == count and json.is_array(voxels.coverage) and #voxels.coverage == count, "voxel lengths do not match dimensions")
        for index, cell in ipairs(voxels.cells) do
            local coverage = voxels.coverage[index]
            require_value(editor.finite_number(cell) and cell % 1 == 0 and cell >= 0 and cell <= 65535, "invalid voxel value")
            require_value(editor.finite_number(coverage) and coverage % 1 == 0 and coverage >= 0 and coverage <= 2, "invalid voxel coverage")
            require_value(coverage ~= 2 or cell ~= 0, "occupied voxel has a zero value")
            if coverage ~= 2 then voxels.cells[index] = 0 end
        end
        region.sx, region.sy, region.sz = dimensions[1], dimensions[2], dimensions[3]
    end
    require_value(json.is_array(value.props) and #value.props <= editor.maximum_supported_props and #value.props <= editor.maximum_copyable_props(), "invalid blueprint prop count")
    local recipes = check_recipes and #value.props > 0 and editor.resolve_placeable_items() or {}
    local props = {}
    for _, prop in ipairs(value.props) do
        object(prop, {itemId=true, position=true, rotation=true, scale=true, templateUuidHighHex=true, templateUuidLowHex=true})
        local id = prop.itemId
        require_value(editor.finite_number(id) and id % 1 == 0 and id >= 1 and id <= 4294967295, "invalid blueprint item ID")
        require_value(not check_recipes or (recipes and recipes[id]), "blueprint item has no current placement recipe")
        local position, rotation, scale = vector(editor, prop.position, 3), vector(editor, prop.rotation, 4), vector(editor, prop.scale, 3)
        local norm = rotation[1]^2 + rotation[2]^2 + rotation[3]^2 + rotation[4]^2
        require_value(editor.finite_number(norm) and norm >= 1e-12, "invalid blueprint quaternion")
        local high, low = prop.templateUuidHighHex, prop.templateUuidLowHex
        for _, half in ipairs({high or false, low or false}) do
            require_value(type(half) == "string" and #half <= 16 and half:match("^%x+$"), "invalid template UUID")
        end
        require_value(not (high:match("^0+$") and low:match("^0+$")), "template UUID is zero")
        props[#props + 1] = {itemId=id, templateUuidHighHex=high:lower(), templateUuidLowHex=low:lower(),
            x=position[1], y=position[2], z=position[3], qx=rotation[1], qy=rotation[2], qz=rotation[3], qw=rotation[4],
            sx=scale[1], sy=scale[2], sz=scale[3]}
    end
    return {region=region, extent=extent, hasVoxels=voxels ~= nil, cells=voxels and voxels.cells,
        coverage=voxels and voxels.coverage, voxelOffset=voxels and voxels.offset, cellSize=voxels and voxels.cellSize, props=props}
end

function schema.decode(editor, content)
    local ok, blueprint = pcall(function() return validate(editor, json.decode(content), true) end)
    if not ok then
        shroudforge.log.warn("World Editor: " .. tostring(blueprint))
        return nil, tostring(blueprint)
    end
    return blueprint, #(blueprint.cells or {}), #blueprint.props
end

function schema.encode(editor, source)
    local ok, result = pcall(function()
        local function array(values)
            local result = {}
            for index, value in ipairs(values) do result[index] = value end
            return json.array(result)
        end
        local value = json.object({['$schema']=schema.url, formatVersion=8,
            rotationAxis=source.region.rotationAxis or "y", extent=array(source.extent), props=json.array()})
        if source.hasVoxels then
            value.voxels = json.object({dimensions=json.array({source.region.sx, source.region.sy, source.region.sz}),
                offset=array(source.voxelOffset), cellSize=array(source.cellSize), cells=array(source.cells), coverage=array(source.coverage)})
        end
        for _, prop in ipairs(source.props or {}) do
            value.props[#value.props + 1] = json.object({itemId=prop.itemId,
                position=json.array({prop.x, prop.y, prop.z}), rotation=json.array({prop.qx, prop.qy, prop.qz, prop.qw}),
                scale=json.array({prop.sx, prop.sy, prop.sz}), templateUuidHighHex=prop.templateUuidHighHex, templateUuidLowHex=prop.templateUuidLowHex})
        end
        validate(editor, value, false)
        return json.encode(value)
    end)
    if not ok then return nil, tostring(result) end
    if #result > editor.maximum_blueprint_bytes then return nil, "blueprint exceeds the 256 MiB file limit" end
    return result
end

return schema
