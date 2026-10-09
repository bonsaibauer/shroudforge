// Private engine-context fixture: no game process or game files are opened.
#include "world_runtime.cpp"
#include "ecs_runtime.h"
#include <cstdio>
#include <cstdlib>
using namespace WorldRuntime;
namespace {
unsigned checks{}, destroys{}, finishes{};
std::uint32_t event_id=73;
void check(bool ok) { ++checks; if(!ok) { std::fprintf(stderr,"world context check %u failed\n",checks); std::exit(1); } }
void __fastcall destroy(void* context,const EngineTransform*,const float*,std::uint32_t feedback) {
    ++destroys; check(feedback==42);
    OnBuildingDispatch(context); // native dispatch can recursively enter the hook
}
void __fastcall finish(void*,void*,std::uint32_t id,bool complete) {
    ++finishes; check(id==event_id && complete);
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
    check(pending_removal.phase==3 && pending_removal.success);
    check(destroys==1 && finishes==0); // hook must NOT call finish recursively
    finish(&context,nullptr,event_id,true); // original engine body resumes once
    check(actor_placement_hook_hits.load()==0); // reported live regression
    OnBuildingDispatch(&context);
    check(destroys==1 && finishes==1); // completed work cannot run twice
    pending_removal.phase=1; ResetContext();
    check(pending_removal.phase==3 && !pending_removal.success);
    OnBuildingDispatch(&context); check(destroys==1); // canceled at world transition
    pending_removal.phase=2; ResetContext();
    check(pending_removal.phase==2); // never interrupt an executing native call
    pending_removal.phase=0;

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
    std::printf("%u world context checks passed; no game process opened\n",checks);
}
