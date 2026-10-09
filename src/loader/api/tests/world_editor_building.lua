local P=building_plan
local registry={{data={blueprintItems={{itemId={value=42},size={x=1,y=1,z=1},data={1}}}}}}
local items={{data={itemId={value=42},equipment={voxelData={isBuildingVoxel=true,placeVoxelMaterialId=192},
    placementAABBmin={x=0,y=0,z=0},placementAABBmax={x=.5,y=.5,z=.5}}}},
    {data={itemId={value=99}}}}
local recipes=assert(P.catalog(items,registry,{.5,.5,.5})); assert(recipes[192]==42)
local grid={origin={10,20,30},cellSize={.5,.5,.5}}
local target={x=2,y=3,z=4,sx=1,sy=1,sz=1}
local commands=assert(P.voxels({cells={192}},target,grid,{0},recipes,"add"))
local c=commands[1]; assert(#commands==1 and c.position[1]==11 and c.position[2]==21.5 and c.position[3]==32)
assert(c.before==0 and c.after==192)
local inverse=P.inverse(c); assert(inverse.action=="remove" and inverse.before==192 and inverse.after==0)
assert(not P.voxels({cells={65281}},target,grid,{0},recipes,"replace")) -- terrain is never approximated
assert(not P.voxels({cells={192}},target,grid,{135},recipes,"replace")) -- unknown old recipe
assert(#assert(P.voxels({cells={0}},target,grid,{192},recipes,"add"))==0)
assert(#assert(P.voxels({cells={192},coverage={0}},target,grid,{0},recipes,"replace"))==0)
assert(P.voxels({cells={192},coverage={1}},target,grid,{192},recipes,"replace")[1].action=="remove")
local replacement=assert(P.voxels({cells={192}},target,grid,{135},{[192]=42,[135]=43},"replace"))
assert(#replacement==2 and replacement[1].after==0 and replacement[2].before==0)

local calls,cancelled,notifications,properties={},{},{},{}
local current_player,selected,cell,status=7,99,0,"queued"
local hovered=0
local api={ecs={},world={building={},entity={},voxel={}}}
api.ecs.query=function() return {current_player} end
api.ecs.read=function(_,name)
    if name=="keen::ecs::SlotSelection" then return {actionbarSlotSelection={index={value=3}}} end
    return {currentBuildingItemId={value=selected},hoveredObjectId={value=hovered,type="Entity"},selectedObjectId={value=0}}
end
api.ecs.resolve=function(id) return id end
api.world.building.submit=function(command) calls[#calls+1]=command; status="queued"; return #calls end
api.world.building.status=function() return status end
api.world.building.cancel=function(id) cancelled[#cancelled+1]=id; return true end
api.world.voxel.read=function() return {cell} end
api.world.entity.query_props=function() return properties end
api.world.entity.get_transform=function(handle)
    for _,p in ipairs(properties) do if p.handle==handle then return p end end
end
local function done(ok,reason,completed,uncertain)
    notifications[#notifications+1]={ok=ok,reason=reason,completed=completed,uncertain=uncertain}
end
local function queue() return game_building(api,function() end) end
local q=queue()
assert(q.start(commands,done)); q.tick(.1)
assert(#calls==1 and calls[1].action=="select" and calls[1].player==7 and calls[1].slot==3)
q.tick(.1); assert(#calls==1) -- queue acknowledgement required
status="dispatched"; q.tick(.1); assert(#calls==1) -- selected item echo required
selected=42; q.tick(.1); assert(#calls==2 and calls[2].action=="place")
status="dispatched"; q.tick(.1); assert(#notifications==0 and q.busy()) -- dispatch != effect
cell=192; q.tick(.1); assert(not q.busy() and notifications[1].ok and #notifications[1].completed==1)
assert(q.start({inverse},done)); q.tick(.1); status="dispatched"; q.tick(.1)
status="dispatched"; q.tick(.1); assert(q.busy()); cell=0; q.tick(.1)
assert(not q.busy() and notifications[2].ok)

-- Whole-batch invalid input prevents even the first valid action.
local bad={action="place",itemId=42,position={0/0,0,0},rotation={0,0,0,1},scale={1,1,1}}
local count=#calls; assert(not q.start({c,bad},done)); assert(#calls==count)
assert(q.start(commands,done)); current_player=8; q.tick(.1)
assert(not q.busy() and #calls==count and not notifications[#notifications].uncertain); current_player=7

-- Timeout after dispatch never retries. A delayed world update can be reconciled.
assert(q.start(commands,done)); q.tick(.1); status="dispatched"; q.tick(.1); status="dispatched"
count=#calls
for _=1,17 do q.tick(1) end
assert(not q.busy() and #calls==count and notifications[#notifications].uncertain)
assert(not q.start(commands,done)); cell=192; q.tick(.1)
assert(#calls==count and not notifications[#notifications].uncertain and #notifications[#notifications].completed==1)

-- Cancellation before press cancels the native request, no optimistic effect.
cell=0; assert(q.start(commands,done)); q.tick(.1); count=#calls; q.cancel()
assert(not q.busy() and cancelled[#cancelled]==count and not notifications[#notifications].uncertain)

assert(q.start(commands,done)); q.tick(.1); count=#calls; cell=135; status="dispatched"; q.tick(.1)
assert(not q.busy() and #calls==count and not notifications[#notifications].uncertain) -- changed during selection
cell=0

-- Prop observation requires a new matching handle, correct transform and template.
local prop_command={action="place",itemId=42,position={1,2,3},rotation={0,0,0,1},scale={1,1,1},
    templateUuidHighHex="01",templateUuidLowHex="02"}
local function prop(handle,high)
    return {handle=handle,itemId=42,templateUuidHighHex=high,templateUuidLowHex="02",
        transform={position={x=1,y=2,z=3},orientation={x=0,y=0,z=0,w=1},scale={x=1,y=1,z=1}}}
end
properties={prop(100,"01")}; assert(q.start({prop_command},done)); q.tick(.1)
status="dispatched"; q.tick(.1); status="dispatched"; q.tick(.1); assert(q.busy())
properties[2]=prop(101,"wrong"); q.tick(.1); assert(q.busy())
properties[2]=prop(101,"01"); q.tick(.1); assert(not q.busy() and prop_command.resultHandle==101)
assert(P.inverse(prop_command).action=="dismantle")
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1); status="dispatched"; q.tick(.1)
assert(q.busy() and calls[#calls].action=="select") -- wrong interaction target must not dismantle
hovered=101; q.tick(.1)
status="dispatched"; q.tick(.1); assert(q.busy()); properties[2]=nil; q.tick(.1); assert(not q.busy())

-- A manually dismantled pasted prop already satisfies its inverse. Undo must
-- consume the journal entry without sending another input or blocking voxels.
properties={prop(102,"01")}; prop_command.resultHandle=102
local before_missing=#calls; properties={}
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
assert(not q.busy() and #calls==before_missing and notifications[#notifications].ok)

assert(q.start({prop_command},done)); q.tick(.1); count=#calls; q.close(); q.tick(1)
assert(#calls==count and cancelled[#cancelled]==count)
