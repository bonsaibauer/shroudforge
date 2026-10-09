// Private engine-context fixture: no game process or game files are opened.
#include "world_runtime.cpp"
#include "ecs_runtime.h"
#include <cstdio>
#include <cstdlib>
using namespace WorldRuntime;
namespace {
unsigned checks{}, destroys{}, finishes{};
unsigned voxel_reads{};
std::uintptr_t expected_read_world{};
std::uint32_t event_id=73;
void check(bool ok) { ++checks; if(!ok) { std::fprintf(stderr,"world context check %u failed\n",checks); std::exit(1); } }
void __fastcall destroy(void* context,const EngineTransform*,const float*,std::uint32_t feedback) {
    ++destroys; check(feedback==42);
    OnBuildingDispatch(context); // native dispatch can recursively enter the hook
}
void __fastcall finish(void*,void*,std::uint32_t id,bool complete) {
    ++finishes; check(id==event_id && complete);
}
bool __fastcall read_cells(CellSpan* cells, const std::uint32_t*, const void* world, std::uint32_t, const std::int32_t*) {
    check(reinterpret_cast<std::uintptr_t>(world)==expected_read_world);
    ++voxel_reads;
    for (std::size_t i=0;i<cells->size;++i) cells->data[i]={7,8};
    return true;
}
}
int main() {
    namespace Profile=KfcRuntimeCompatibility::EnshroudedClient;
    struct Context { std::uintptr_t root,remove,state,commands; std::uint32_t owner; } context{123,1,2,3,4};
    Profile::world_context_layout.remove_queue=offsetof(Context,remove);
    Profile::world_context_layout.publish_state=offsetof(Context,state);
    Profile::world_context_layout.publish_commands=offsetof(Context,commands);
    Profile::world_context_layout.owner=offsetof(Context,owner);
    Profile::RuntimeOperation remove,done;
    remove.name="runtime.world.entity.destroy"; remove.available=true;
    remove.function_rva=reinterpret_cast<std::uintptr_t>(&destroy)-image_base();
    done.name="runtime.world.entity.finish_building"; done.available=true;
    done.function_rva=reinterpret_cast<std::uintptr_t>(&finish)-image_base();
    Profile::runtime_operations={remove,done};
    Profile::world_finish_event_id_rva=reinterpret_cast<std::uintptr_t>(&event_id)-image_base();
    pending_removal.phase=1; pending_removal.feedback=42;
    observed_execution_view.store(999);
    OnBuildingDispatch(&context);
    check(pending_removal.phase==1 && destroys==0); // unrelated execution view
    observed_execution_view.store(context.root);
    context.remove=0; OnBuildingDispatch(&context);
    check(pending_removal.phase==1 && destroys==0); // incomplete engine context
    context.remove=1; OnBuildingDispatch(&context);
    // R9 deliberately does NOT consume the POD removal request from the
    // building dispatcher. The reverted R10 consumer caused live hangs.
    check(pending_removal.phase==1 && destroys==0);
    check(finishes==0 && actor_placement_hook_hits.load()==0);
    OnBuildingDispatch(&context);
    check(destroys==0 && finishes==0);
    pending_removal.phase=1; ResetContext();
    check(pending_removal.phase==1);
    OnBuildingDispatch(&context); check(destroys==0);
    pending_removal.phase=2; ResetContext();
    check(pending_removal.phase==2); // never interrupt an executing native call
    pending_removal.phase=0;

    // Existing local singleton route, actor route and missing-context evidence.
    struct World { std::uintptr_t padding{}, store{1}; } local_world, actor_world;
    struct Singleton { std::uintptr_t padding{}, context{}; } singleton;
    auto singleton_pointer=reinterpret_cast<std::uintptr_t>(&singleton);
    singleton.context=reinterpret_cast<std::uintptr_t>(&local_world)+8;
    Profile::RuntimeOperation active;
    active.name="runtime.world.context.active"; active.available=true;
    active.global_rva=reinterpret_cast<std::uintptr_t>(&singleton_pointer)-image_base();
    active.context_pointer_offset=offsetof(Singleton,context);
    active.world_offset=-8; active.validation_offset=offsetof(World,store);
    Profile::runtime_operations={active};
    std::uintptr_t resolved{};
    check(resolve_voxel_world(resolved) && resolved==reinterpret_cast<std::uintptr_t>(&local_world));
    auto diagnostics=nlohmann::json::parse(ContextDiagnostics());
    check(diagnostics["source"]=="client-singleton" && diagnostics["global"]["reason"]=="valid");
    singleton_pointer=0;
    check(!resolve_voxel_world(resolved));
    diagnostics=nlohmann::json::parse(ContextDiagnostics());
    check(diagnostics["global"]["reason"]=="singleton-null" && diagnostics["clientSessionMode"]=="unknown");
    struct Service { std::uintptr_t padding{}, world{}; } service{0,reinterpret_cast<std::uintptr_t>(&actor_world)};
    struct Frame { std::uintptr_t padding{}, service{}; } frame{0,reinterpret_cast<std::uintptr_t>(&service)};
    Profile::world_context_layout.actor_frame_service_view=offsetof(Frame,service);
    Profile::world_context_layout.service_view_world=offsetof(Service,world);
    observe_actor_world(&frame);
    check(resolve_voxel_world(resolved) && resolved==service.world);
    check(nlohmann::json::parse(ContextDiagnostics())["source"]=="actor-frame");
    preferred_voxel_world.store(service.world);
    ResetContext();
    check(!resolve_voxel_world(resolved));
    check(preferred_voxel_world.load()==0 && observed_actor_world.load()==0);
    observe_actor_world(reinterpret_cast<void*>(1));
    check(nlohmann::json::parse(ContextDiagnostics())["actor"]["reason"]=="service-view-unavailable");
    SetCursorHookReady(true);
    std::array<std::uint8_t, NativeCursorSize> cursor{}, snapshot{};
    std::uint64_t sequence{};
    OnCursorUpdate(cursor.data(),nullptr);
    check(ReadCursorSnapshot(snapshot.data(),snapshot.size(),&sequence));
    ResetContext();
    check(!ReadCursorSnapshot(snapshot.data(),snapshot.size(),&sequence));
    OnCursorUpdate(cursor.data(),nullptr);
    check(ReadCursorSnapshot(snapshot.data(),snapshot.size(),&sequence));

    // The same current execution-view chain is observed after switching worlds.
    struct Manager { std::uint64_t count; std::uintptr_t table; };
    std::uintptr_t entries[2]{};
    Manager first{2,reinterpret_cast<std::uintptr_t>(entries)}, second=first;
    struct Root { std::uintptr_t manager; } root{reinterpret_cast<std::uintptr_t>(&first)};
    std::uintptr_t view=reinterpret_cast<std::uintptr_t>(&root);
    Profile::lookup_manager=offsetof(Root,manager);
    Profile::entity_manager_count=offsetof(Manager,count);
    Profile::entity_manager_table=offsetof(Manager,table);
    GameThreadDispatcher::ObserveExecutionView(&view);
    check(GameThreadDispatcher::EntityManager()==root.manager);
    EcsRuntime::Tick();
    const auto first_session=KfcRuntimeWorldSessionId();
    check(first_session!=0);
    first.count=0; first.table=0;
    check(KfcRuntimeWorldSessionId()==0);
    root.manager=reinterpret_cast<std::uintptr_t>(&second);
    GameThreadDispatcher::ObserveExecutionView(&view);
    check(GameThreadDispatcher::EntityManager()==root.manager);
    check(KfcRuntimeWorldSessionId()==0); // new manager not yet adopted by ECS
    EcsRuntime::Tick();
    check(KfcRuntimeWorldSessionId()!=0 && KfcRuntimeWorldSessionId()!=first_session);
    root.manager=reinterpret_cast<std::uintptr_t>(&first);
    GameThreadDispatcher::ObserveExecutionView(&view);
    check(GameThreadDispatcher::EntityManager()==reinterpret_cast<std::uintptr_t>(&second));
    GameThreadDispatcher::ObserveExecutionView(reinterpret_cast<void*>(1));
    check(GameThreadDispatcher::EntityManager()==reinterpret_cast<std::uintptr_t>(&second));
    // A valid preview-world lookup must not fight the live local-player source.
    first.count=2; first.table=reinterpret_cast<std::uintptr_t>(entries);
    root.manager=reinterpret_cast<std::uintptr_t>(&second);
    GameThreadDispatcher::ObserveExecutionView(&view,true);
    root.manager=reinterpret_cast<std::uintptr_t>(&first);
    for (unsigned i=0;i<10;++i) GameThreadDispatcher::ObserveExecutionView(&view);
    check(GameThreadDispatcher::EntityManager()==reinterpret_cast<std::uintptr_t>(&second));
    check(KfcRuntimeWorldSessionId()!=0 && KfcRuntimeWorldSessionId()!=first_session);
    // A new local-player world still replaces the previous authoritative source.
    GameThreadDispatcher::ObserveExecutionView(&view,true);
    check(GameThreadDispatcher::EntityManager()==reinterpret_cast<std::uintptr_t>(&first));
    check(KfcRuntimeWorldSessionId()==0);
    // The cursor-derived replica supports reads only. A successful read cannot
    // promote its pointer into the direct write/spawn context.
    Profile::world_context_layout.client_cursor_service_view=offsetof(Frame,service);
    Profile::world_context_layout.client_cursor_service_world=offsetof(Service,world);
    Profile::RuntimeOperation read,write;
    read.name="runtime.world.voxel.read"; read.available=true;
    read.function_rva=reinterpret_cast<std::uintptr_t>(&read_cells)-image_base();
    write.name="runtime.world.voxel.write"; write.available=true;
    Profile::runtime_operations={active,read,write};
    OnCursorUpdate(cursor.data(),&view,&frame);
    check(resolve_client_read_world(resolved) && resolved==service.world);
    check(!resolve_voxel_world(resolved));
    check(!OperationAvailable("runtime.world.voxel.write"));
    check(nlohmann::json::parse(ContextDiagnostics())["source"]=="client-cursor-read");
    Operation read_op;
    read_op.kind=Operation::Kind::Read; read_op.dimensions={1,1,1}; read_op.cells.resize(1);
    read_op.context_generation=context_resets.load();
    expected_read_world=service.world;
    execute(&read_op);
    check(read_op.result && voxel_reads==1 && preferred_voxel_world.load()==0);
    Operation write_op;
    write_op.kind=Operation::Kind::Write;
    execute(&write_op);
    check(!write_op.result && !write_op.write_attempted);
    client_read_ms.store(GetTickCount64()-501);
    check(!resolve_client_read_world(resolved));
    OnCursorUpdate(cursor.data(),&view,&frame);
    ResetContext();
    check(!resolve_client_read_world(resolved));
    read_op.result=false;
    execute(&read_op);
    check(!read_op.result && voxel_reads==1);
    // The existing local singleton retains precedence and direct capability.
    singleton_pointer=reinterpret_cast<std::uintptr_t>(&singleton);
    OnCursorUpdate(cursor.data(),&view,&frame);
    check(OperationAvailable("runtime.world.voxel.write"));
    check(resolve_voxel_world(resolved) && resolved==reinterpret_cast<std::uintptr_t>(&local_world));
    read_op.context_generation=context_resets.load();
    expected_read_world=reinterpret_cast<std::uintptr_t>(&local_world);
    execute(&read_op);
    check(read_op.result && voxel_reads==2 && preferred_voxel_world.load()==expected_read_world);
    std::printf("%u world context checks passed; no game process opened\n",checks);
}
