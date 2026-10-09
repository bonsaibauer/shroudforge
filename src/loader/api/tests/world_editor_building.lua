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
local transform_failure=nil
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
    if transform_failure then return nil,transform_failure end
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
    return {handle=handle,entityId=handle+1000,itemId=42,templateUuidHighHex=high,templateUuidLowHex="02",
        transform={position={x=1,y=2,z=3},orientation={x=0,y=0,z=0,w=1},scale={x=1,y=1,z=1}}}
end
properties={prop(100,"01")}; assert(q.start({prop_command},done)); q.tick(.1)
status="dispatched"; q.tick(.1); status="dispatched"; q.tick(.1); assert(q.busy())
properties[2]=prop(101,"wrong"); q.tick(.1); assert(q.busy())
properties[2]=prop(101,"01"); q.tick(.1); assert(not q.busy() and prop_command.resultHandle==101 and prop_command.resultEntityId==1101)
assert(P.inverse(prop_command).action=="dismantle")
local direct_entity={recipe={id=42},entityHandle=303,entityId=0x12345,
    position={1,2,3},rotation={0,0,0,1},scale={1,1,1},
    expected_transform={templateUuidHighHex="01",templateUuidLowHex="02"}}
local direct_undo=P.dismantle_command(direct_entity)
assert(direct_undo.action=="dismantle" and direct_undo.handle==303 and direct_undo.entityId==0x12345,
    "direct ECS paste undo must carry the spawned prop's exact game entity ID into held dismantle input")
local direct_live=prop(303,"01"); direct_live.entityId=0x12345; properties={direct_live}
assert(q.start({direct_undo},done)); q.tick(.1)
assert(calls[#calls].action=="dismantle" and calls[#calls].targetEntityId==0x12345,
    "direct ECS prop undo must submit the exact target entity ID")
status="dispatched"; q.tick(.1); properties={}; q.tick(.1); assert(not q.busy())
properties={prop(100,"01"),prop(101,"01")}
local mixed_journal={
    {action="place",itemId=42,position={1,2,3},rotation={0,0,0,1},scale={1,1,1},resultHandle=201},
    {action="place",itemId=42,position={0,0,0},rotation={0,0,0,1},scale={1,1,1},cell={1,0,0},before=0,after=42},
    {action="place",itemId=42,position={4,5,6},rotation={0,0,0,1},scale={1,1,1},resultHandle=202},
    {action="place",itemId=42,position={0,0,0},rotation={0,0,0,1},scale={1,1,1},cell={2,0,0},before=0,after=42},
}
local mixed_undo=P.undo_batch(mixed_journal)
assert(#mixed_undo==4 and mixed_undo[1].journal_index==4 and mixed_undo[2].journal_index==2 and
    mixed_undo[3].journal_index==3 and mixed_undo[4].journal_index==1,
    "voxel blocks must undo before any prop dismantle, retaining reverse order within each kind")
assert(mixed_undo[1].command.cell and mixed_undo[2].command.cell and not mixed_undo[3].command.cell)
local partial_journal={table.unpack(mixed_journal)}
P.consume_undo_batch(partial_journal,mixed_undo,2)
assert(#partial_journal==2 and partial_journal[1]==mixed_journal[1] and partial_journal[2]==mixed_journal[3],
    "partial voxel undo must remove only its matching journal entries and preserve props")
local before_dismantle=#calls
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
assert(q.busy() and #calls==before_dismantle+1 and calls[#calls].action=="dismantle" and calls[#calls].targetEntityId==prop_command.resultEntityId) -- exact entity, no inventory selection or manual aim
status="dispatched"; q.tick(.1); assert(q.busy()); properties[2]=nil; q.tick(.1)
assert(not q.busy() and notifications[#notifications].ok)

-- Temporary transform-read failures are never mistaken for a missing prop,
-- either before sending the input or while confirming its effect.
properties={prop(105,"01")}; prop_command.resultHandle=105; prop_command.resultEntityId=1105
q=queue(); transform_failure="KFC Runtime is not ready"; before_dismantle=#calls
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
assert(q.busy() and #calls==before_dismantle,"unavailable transform read must not send input")
transform_failure=nil; q.tick(.1)
assert(q.busy() and #calls==before_dismantle+1 and calls[#calls].action=="dismantle")
status="dispatched"; transform_failure="KFC Runtime is not ready"; q.tick(.1)
assert(q.busy(),"unavailable transform read must not confirm removal")
transform_failure=nil; properties={}; q.tick(.1)
assert(not q.busy() and notifications[#notifications].ok)

-- A manually dismantled pasted prop already satisfies its inverse. Undo must
-- consume the journal entry without sending another input or blocking voxels.
properties={prop(102,"01")}; prop_command.resultHandle=102; prop_command.resultEntityId=1102
local before_missing=#calls; properties={}
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
assert(not q.busy() and #calls==before_missing and notifications[#notifications].ok)

assert(q.start({prop_command},done)); q.tick(.1); count=#calls; q.close(); q.tick(1)
assert(#calls==count and cancelled[#cancelled]==count)

-- Incremental ECS discovery is pending work, not a failed undo. No input is
-- submitted until discovery finishes, including temporary scans mid-command.
q=queue(); cell=0; selected=42
local querying=api.ecs.query
api.ecs.query=function() return nil,"live ECS query is still scanning; retry on the next update" end
count=#calls
assert(q.start(commands,done)); q.tick(.1); q.tick(.1)
assert(q.busy() and #calls==count)
api.ecs.query=querying; q.tick(.1)
assert(#calls==count+1 and calls[#calls].action=="select")
api.ecs.query=function() return nil,"live ECS query is still scanning; retry on the next update" end
q.tick(.1); assert(q.busy() and #calls==count+1)
api.ecs.query=querying; status="dispatched"; q.tick(.1)
assert(#calls==count+2 and calls[#calls].action=="place")
status="dispatched"; cell=192; q.tick(.1)
assert(not q.busy() and notifications[#notifications].ok)

api.ecs.query=function() return nil,"live ECS query is still scanning; retry on the next update" end
assert(q.start(commands,done)); count=#calls
for _=1,17 do q.tick(1) end
assert(not q.busy() and #calls==count and not notifications[#notifications].uncertain)
api.ecs.query=querying

-- Removing a known handle must not rescan every prop in the world on every
-- preparation and observation tick. Identity and transform checks still apply.
q=queue(); properties={prop(103,"01")}; prop_command.resultHandle=103; prop_command.resultEntityId=1103; hovered=103
api.world.entity.query_props=function() error('undo performed an unnecessary world scan') end
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
status="dispatched"; q.tick(.1); status="dispatched"; q.tick(.1)
assert(q.busy()); properties={}
api.has=function() return false end
q.tick(.1); assert(q.busy(), 'unavailable prop API is not proof of removal')
api.has=function() return true end
q.tick(.1)
assert(not q.busy() and notifications[#notifications].ok)

-- A one-frame read-only discovery timeout must not abort Undo or duplicate input.
q=queue(); cell=0; selected=42
api.ecs.query=function() return nil,"native ECS query failed or timed out" end
count=#calls
assert(q.start(commands,done)); q.tick(.1)
assert(q.busy() and #calls==count)
api.ecs.query=querying; q.tick(.1)
assert(#calls==count+1)
api.ecs.query=function() return nil,"native ECS query failed or timed out" end
q.tick(.1); q.tick(.1)
assert(q.busy() and #calls==count+1)
api.ecs.query=querying; status="dispatched"; q.tick(.1)
assert(#calls==count+2)
status="dispatched"; cell=192; q.tick(.1)
assert(not q.busy() and notifications[#notifications].ok)
api.ecs.query=function() return nil,"native ECS query failed or timed out" end
count=#calls; assert(q.start(commands,done))
for _=1,17 do q.tick(1) end
assert(not q.busy() and #calls==count)
api.ecs.query=function() return nil,"unknown component" end
assert(not q.start(commands,done))
api.ecs.query=querying

-- A changed world stops a queued action before submission, even if a player
-- mock returns the same handle. Session identity is independent of handles.
local session=21
api.world.session_id=function() return session end
q=queue(); cell=0; count=#calls
assert(q.start(commands,done)); session=22; q.tick(.1)
assert(not q.busy() and #calls==count)
assert(notifications[#notifications].reason:find("World session changed",1,true))

-- Prop Undo uses the saved handle/transform and sends dismantle without waiting
-- for the physical cursor to hover the prop.
properties={prop(104,"01")}; hovered=0; prop_command.resultHandle=104; prop_command.resultEntityId=1104
before_dismantle=#calls
assert(q.start({P.inverse(prop_command)},done)); q.tick(.1)
assert(q.busy() and #calls==before_dismantle+1 and calls[#calls].action=="dismantle")
q.cancel(); properties={}

-- A stalled game thread temporarily hides the player and session. Read-only
-- discovery pauses the Undo deadline and sends no selection/dismantle input.
local stable_query=api.ecs.query
local stable_session=session
q=queue(); cell=0; hovered=0; count=#calls; local notice_count=#notifications
api.ecs.query=function() return nil,"KFC Runtime is not ready" end
assert(q.start({commands[1]},done),"T1 transient start should queue")
q.tick(.1); assert(q.busy() and #calls==count,"T2 transient player query should pause")
api.world.session_id=function() return 0 end
for _=1,20 do q.tick(1) end
assert(q.busy() and #calls==count and #notifications==notice_count,"T3 unavailable world session should pause without input")
api.world.session_id=function() return stable_session end
api.ecs.query=stable_query; q.tick(.1); q.tick(.1)
assert(#calls==count+1 and calls[#calls].action=="select","T4 restored ECS should resume selection")
q.close(); api.ecs.query=querying
