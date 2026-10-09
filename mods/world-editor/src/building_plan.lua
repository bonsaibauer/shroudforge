-- Resolve ordinary one-cell building recipes from the current KFC resources.
-- No hardcoded item IDs, material hashes, or invented terrain conversions.
local M = {}
local function scalar(value)
    if type(value)=="number" then return value end
    return value and tonumber(value.value)
end
function M.catalog(items, registries, cell_size)
    local single, result = {}, {}
    for _, resource in ipairs(registries) do
        for _, entry in ipairs(resource.data.blueprintItems or {}) do
            local size=entry.size
            if size and size.x==1 and size.y==1 and size.z==1 and #entry.data==1 and entry.data[1]==1 then
                single[scalar(entry.itemId)]=true
            end
        end
    end
    for _, resource in ipairs(items) do
        local data=resource.data
        local equipment=data and data.equipment
        local voxel=equipment and equipment.voxelData
        local id=data and scalar(data.itemId)
        local material=voxel and tonumber(voxel.placeVoxelMaterialId)
        if id and single[id] and voxel and voxel.isBuildingVoxel and material and material>=128 and material<=255 then
            local low,high=equipment.placementAABBmin,equipment.placementAABBmax
            if low and high and low.x==0 and low.y==0 and low.z==0 and
                high.x==cell_size[1] and high.y==cell_size[2] and high.z==cell_size[3] then
                if result[material] and result[material]~=id then
                    return nil,"Ambiguous one-cell material recipe "..material
                end
                result[material]=id
            end
        end
    end
    return result
end
function M.voxels(plan,target,grid,before,recipes,mode)
    local commands={}
    local function add(action,value,old,new,x,y,z)
        local material=value%256
        local p={grid.origin[1]+x*grid.cellSize[1],grid.origin[2]+y*grid.cellSize[2],grid.origin[3]+z*grid.cellSize[3]}
        commands[#commands+1]={action=action,itemId=recipes[material],position=p,
            rotation={0,0,0,1},scale={1,1,1},cell={x,y,z},before=old,after=new}
    end
    for z=0,target.sz-1 do for y=0,target.sy-1 do for x=0,target.sx-1 do
        local i=x+target.sx*(y+target.sy*z)+1
        local value,old=plan.cells[i],before[i]
        if plan.coverage and plan.coverage[i]==1 then value=0 end
        local covered=not plan.coverage or plan.coverage[i]~=0
        if covered and (mode~="add" or value~=0) and value~=old then
            -- Building cells encode their material in the low byte. The high
            -- byte is not a terrain density to approximate with a building cube.
            for _, cell in ipairs({old,value}) do
                if type(cell)~="number" or cell%1~=0 or cell<0 or cell>255 or
                    (cell~=0 and not recipes[cell]) then
                    return nil,"Selection includes terrain or a cell without an exact one-cell building recipe; no input was sent"
                end
            end
            if old~=0 then add("remove",old,old,0,target.x+x,target.y+y,target.z+z) end
            if value~=0 then add("place",value,0,value,target.x+x,target.y+y,target.z+z) end
        end
    end end end
    return commands
end
function M.inverse(command)
    local inverse_action = command.action=="place" and (command.cell and "remove" or "dismantle") or "place"
    return {action=inverse_action,itemId=command.itemId,
        handle=command.resultHandle,position=command.position,rotation=command.rotation,scale=command.scale,
        templateUuidHighHex=command.templateUuidHighHex,templateUuidLowHex=command.templateUuidLowHex,
        cell=command.cell,before=command.after,after=command.before}
end
return M
