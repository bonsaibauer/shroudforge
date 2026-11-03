// Read-only original-image contract inspection. No game function is executed.
#include <windows.h>
namespace { HMODULE mapped_image{}; HMODULE contract_image(LPCWSTR) { return mapped_image; } }
#define GetModuleHandleW contract_image
#include "code-inventory/resolve_original_call_graph.cpp"
#undef GetModuleHandleW
#include "voxels/write_registered_voxel_grid.cpp"
#include "impact/submit_building_prop_spawn.cpp"
#include <cstdio>
#include <cstdlib>
#include <numeric>
#include <filesystem>
#include <fstream>

namespace { std::optional<EngineContext::BuildingPropSpawnLayout> fixture_spawn_layout; }

// Exercise the public retry boundary with controlled clock and submit results.
// The production adapter above remains available for read-only ABI inspection.
namespace RetryFixture {
struct Attempt { bool result{}; std::uint32_t outcome{1}; ULONGLONG duration{}; };
std::vector<Attempt> attempts;
std::size_t calls{}, pauses{};
ULONGLONG now{};
std::uint64_t session{5}, supplied_session{};
bool change_session_on_pause{};
ULONGLONG clock() { return now; }
void pause(DWORD duration) {
    ++pauses; now += duration;
    if (change_session_on_pause) ++session;
}
void reset(std::initializer_list<Attempt> values) {
    attempts = values; calls = pauses = 0; now = 0; session = 5;
    supplied_session = 0; change_session_on_pause = false;
}
}
namespace WorldRuntime::RegisteredVoxelGrid {
bool RetryFixtureWrite(std::uint64_t session, const std::int32_t*,
    const std::uint32_t*, const std::uint16_t*, std::size_t, std::uint32_t* outcome) {
    RetryFixture::supplied_session = session;
    const auto index = (std::min)(RetryFixture::calls++, RetryFixture::attempts.size() - 1);
    const auto value = RetryFixture::attempts[index];
    RetryFixture::now += value.duration;
    *outcome = value.outcome;
    return value.result;
}
}
#define WriteForSession RetryFixtureWrite
#define GetTickCount64 RetryFixture::clock
#define Sleep RetryFixture::pause
#include "voxels/read_and_write_voxels.cpp"
#undef Sleep
#undef GetTickCount64
#undef WriteForSession

namespace SpawnRetryFixture {
using Result = ImpactRuntime::BuildingPropSpawnResult;
std::vector<Result> results;
std::size_t calls{}, fences{};
bool completion_unavailable{};
void reset(std::initializer_list<Result> values) {
    RetryFixture::reset({{false,1}});
    results=values;calls=fences=0;completion_unavailable=false;
}
}
namespace ImpactRuntime {
BuildingPropSpawnResult RetryFixtureSpawn(std::uint64_t session,const std::uint64_t*,
    const double*,const double*,std::uint32_t,std::uint32_t,BuildingPropSpawnReceipt& receipt) {
    RetryFixture::supplied_session=session;
    const auto result=SpawnRetryFixture::results[(std::min)(SpawnRetryFixture::calls++,SpawnRetryFixture::results.size()-1)];
    if(result==BuildingPropSpawnResult::Submitted) receipt={1001,{11,22,33},{0,0,0,1}};
    return result;
}
}
#define SubmitBuildingPropSpawn RetryFixtureSpawn
#define GetTickCount64 RetryFixture::clock
#define Sleep RetryFixture::pause
#include "building/spawn_building_prop.cpp"
#undef Sleep
#undef GetTickCount64
#undef SubmitBuildingPropSpawn

namespace EcsRuntime {
bool ResolvePropEntityId(std::uint32_t id,KfcRuntimePropRecord* prop) {
    if(SpawnRetryFixture::completion_unavailable || id!=1001 || !prop)return false;
    *prop={};prop->entity_id=id;prop->entity_handle=7;prop->item_id=42;
    prop->template_uuid[0]=1;prop->template_uuid[1]=2;
    prop->position[0]=11;prop->position[1]=22;prop->position[2]=33;
    prop->orientation[3]=1;
    for(auto& scale:prop->scale)scale=1;
    return true;
}
}
extern "C" bool __cdecl KfcRuntimeEcsRead(std::uint32_t handle,const char*,void* value,std::size_t size) {
    if(handle!=7 || !value || size!=1)return false;
    *static_cast<std::uint8_t*>(value)=0;return true;
}

// The test has no world. All ownership/publishing boundaries are unavailable.
namespace NamedServiceImplementation {
std::mutex operation_mutex;
bool copy(void* to, const void* from, std::size_t size) {
    __try { std::memcpy(to, from, size); return true; }
    __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}
bool executable_callback(std::uintptr_t at) {
    MEMORY_BASIC_INFORMATION page{};
    return at && VirtualQuery(reinterpret_cast<const void*>(at), &page, sizeof(page)) == sizeof(page) &&
        page.State == MEM_COMMIT && page.Type == MEM_IMAGE && page.AllocationBase == mapped_image &&
        (page.Protect == PAGE_EXECUTE || page.Protect == PAGE_EXECUTE_READ);
}
BindingContract binding_contract(const char* name) { BindingContract value; value.key = name; return value; }
}
namespace EngineContext {
std::optional<BuildingPropSpawnLayout> ConfiguredBuildingPropSpawnLayout() { return fixture_spawn_layout; }
struct EcsWriteGuard::State {};
EcsWriteGuard::EcsWriteGuard() = default;
EcsWriteGuard::~EcsWriteGuard() = default;
bool EcsWriteGuard::TryAcquire(const EcsView&, const char**) { return false; }
bool EcsWriteGuard::Validate() const { return false; }
void EcsWriteGuard::Release() noexcept {}
bool ResolveEcs(EcsView&, std::string*) { return false; }
}
namespace EcsRuntime {
struct SessionGuard::State {};
SessionGuard::SessionGuard() = default;
SessionGuard::~SessionGuard() = default;
bool SessionGuard::TryAcquire(std::uint64_t) { return false; }
const EngineContext::EcsView& SessionGuard::View() const { static EngineContext::EcsView view; return view; }
bool SessionGuard::WritesAllowed() const { return false; }
void SessionGuard::MarkWriteUncertain(const char*) {}
bool MarkSessionWriteUncertain(std::uint64_t, const char*) { ++SpawnRetryFixture::fences;return false; }
bool ResolveEntityCommandQueue(const NamedServiceImplementation::BindingContract&, const EngineContext::EcsView&, EntityCommandQueueSelection&) { return false; }
bool RereadEntityCommandQueue(const NamedServiceImplementation::BindingContract&, const EngineContext::EcsView&, const EntityCommandQueueSelection&, EntityCommandQueueSelection&) { return false; }
}
namespace ImpactRuntime {
ResolveResult ResolveRegisteredOperation(std::uint64_t, const char*, RegisteredOperation&) { return ResolveResult::RegistryUnavailable; }
ResolveResult ResolveRegisteredOperationPinned(const NamedServiceImplementation::BindingContract&, const EngineContext::EcsView&, const char*, RegisteredOperation&) { return ResolveResult::RegistryUnavailable; }
}
namespace WorldRuntime::Implementation {
std::atomic<std::uint64_t> context_resets{};
std::atomic<std::uintptr_t> preferred_voxel_world{};
bool resolve_voxel_world(std::uintptr_t&) { return false; }
bool resolve_client_read_world(std::uintptr_t&) { return false; }
const KfcRuntimeCompatibility::EnshroudedClient::RuntimeOperation* find_operation(const char*) { return nullptr; }
}
namespace GameThreadDispatcher {
bool Ready() { return false; }
bool Invoke(Operation, std::shared_ptr<void>, std::uint32_t) { return false; }
}
namespace WorldRuntime { bool OperationAvailable(const char*) { return false; } }
extern "C" std::uint64_t __cdecl KfcRuntimeWorldSessionId() { return RetryFixture::session; }
namespace WorldRuntime::RegisteredVoxelGrid {
struct PinnedRegion::State {};
PinnedRegion::PinnedRegion(const std::int32_t*, const std::uint32_t*) {}
PinnedRegion::~PinnedRegion() = default;
bool PinnedRegion::AvailablePinned(const EngineContext::EcsView&) const { return false; }
bool PinnedRegion::ValidatePinned(const EngineContext::EcsView&) const { return false; }
std::uintptr_t PinnedRegion::WorldPayload() const { return 0; }
Result PinnedRegion::ReadPinned(const EngineContext::EcsView&, std::uint16_t*, std::size_t) { return Result::Unavailable; }
bool Healthy() noexcept { return true; }
Result ReadForSession(std::uint64_t, const std::int32_t*, const std::uint32_t*,
    std::uint16_t*, std::size_t, std::size_t*) { return Result::Unavailable; }
}

bool check_public_write_retry() {
    namespace F = RetryFixture;
    const std::int32_t origin[3]{};
    const std::uint32_t dimensions[3]{1,1,1};
    const std::uint16_t value{65282};
    std::uint32_t outcome{};
    const auto write = [&]() { return WorldRuntime::WriteVoxels(origin, dimensions, &value, 1, &outcome); };
    F::reset({{false,1},{false,1},{true,0}});
    bool ok = write() && outcome == 0 && F::calls == 3 && F::pauses == 2 && F::supplied_session == 5;
    for (const auto stop : {0u,2u,3u}) {
        F::reset({{false,stop},{true,0}});
        ok = !write() && outcome == stop && F::calls == 1 && !F::pauses && ok;
    }
    F::reset({{true,0},{true,0}});
    ok = write() && F::calls == 1 && !F::pauses && ok;
    F::reset({{false,1},{true,0}});
    F::change_session_on_pause = true;
    ok = !write() && outcome == 1 && F::calls == 1 && F::pauses == 1 && ok;
    F::reset({{false,1}});
    ok = !write() && outcome == 1 && F::calls == 125 && F::now == 250 && ok;
    F::reset({{false,1,251},{true,0}});
    ok = !write() && outcome == 1 && F::calls == 1 && !F::pauses && ok;
    F::reset({{true,0}});
    F::session = 0;
    ok = !write() && outcome == 1 && !F::calls && ok;
    F::reset({{true,0}});
    const std::uint32_t empty[3]{1,0,1};
    const std::int32_t overflow[3]{INT32_MAX,0,0};
    ok = !WorldRuntime::WriteVoxels(nullptr,dimensions,&value,1,&outcome) &&
        !WorldRuntime::WriteVoxels(origin,nullptr,&value,1,&outcome) &&
        !WorldRuntime::WriteVoxels(origin,empty,&value,1,&outcome) &&
        !WorldRuntime::WriteVoxels(origin,dimensions,nullptr,1,&outcome) &&
        !WorldRuntime::WriteVoxels(origin,dimensions,&value,2,&outcome) &&
        !WorldRuntime::WriteVoxels(overflow,dimensions,&value,1,&outcome) &&
        !WorldRuntime::WriteVoxels(origin,dimensions,&value,1,nullptr) && !F::calls && ok;
    return ok;
}

bool check_public_spawn_retry() {
    namespace F=SpawnRetryFixture;
    using R=F::Result;
    const std::uint64_t uuid[2]{1,2};
    const double position[3]{},rotation[4]{0,0,0,1};
    std::uint32_t handle{},outcome{};
    const auto spawn=[&](){return WorldRuntime::SpawnEntity(uuid,position,rotation,42,0,&handle,&outcome);};
    F::reset({R::Unavailable,R::Unavailable,R::Submitted});
    bool ok=spawn() && handle==7 && outcome==0 && F::calls==3 && RetryFixture::pauses==2 && !F::fences;
    for(const auto stopped:{R::Rejected,R::Uncertain,R::StaleSession}) {
        F::reset({stopped,R::Submitted});
        ok=!spawn() && !handle && F::calls==1 && !RetryFixture::pauses && ok;
    }
    F::reset({R::Unavailable});
    ok=!spawn() && outcome==1 && !handle && RetryFixture::now==250 && ok;
    F::reset({R::Unavailable,R::Submitted});
    RetryFixture::change_session_on_pause=true;
    ok=!spawn() && outcome==5 && !handle && F::calls==1 && ok;
    F::reset({R::Submitted,R::Submitted});
    F::completion_unavailable=true;
    ok=!spawn() && outcome==4 && !handle && F::calls==1 && F::fences==1 && ok;
    F::reset({R::Submitted});
    ok=!WorldRuntime::SpawnEntity(nullptr,position,rotation,42,0,&handle,&outcome) && !F::calls && ok;
    return ok;
}

int wmain(int argc, wchar_t** argv) {
    if (argc < 3 || argc > 5) { std::fprintf(stderr, "usage: native-contract-test <original.exe> <terraforming-analysis-rva> [writer-analysis-rva] [profile.json]\n"); return 2; }
    mapped_image = LoadLibraryExW(argv[1], nullptr, DONT_RESOLVE_DLL_REFERENCES);
    if (!mapped_image) return 2;
    const auto base = reinterpret_cast<std::uintptr_t>(mapped_image);
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    KfcRuntimeCompatibility::EnshroudedClient::image_size = nt->OptionalHeader.SizeOfImage;
    wchar_t* end{};
    const auto root = std::wcstoull(argv[2], &end, 0);
    if (*end || root >= nt->OptionalHeader.SizeOfImage) return 2;
    std::filesystem::path fixture_path;
    fixture_spawn_layout = EngineContext::BuildingPropSpawnLayout{2088, 0};
    bool writer_argument = false;
    if (argc >= 4) {
        std::wcstoull(argv[3], &end, 0);
        writer_argument = !*end;
        if (!writer_argument) fixture_path = argv[3];
    }
    if (argc == 5) {
        if (!writer_argument) return 2;
        fixture_path = argv[4];
    }
    if (!fixture_path.empty()) try {
        std::ifstream stream(fixture_path);
        const auto fixture = nlohmann::json::parse(stream);
        if (fixture.contains("image") &&
            (fixture.at("image").at("timestamp") != nt->FileHeader.TimeDateStamp ||
             fixture.at("image").at("size") != nt->OptionalHeader.SizeOfImage)) return 2;
        const auto& layout = fixture.at("buildingPropSpawnLayout");
        fixture_spawn_layout = EngineContext::BuildingPropSpawnLayout{
            layout.at("sourceEntityIdsOffset").get<std::size_t>(),
            layout.at("allocationFailureMarkerValue").get<std::uint32_t>()};
    } catch (const std::exception& error) {
        std::fprintf(stderr, "cannot read spawn profile fixture: %s\n", error.what()); return 2;
    }
    namespace Voxel = WorldRuntime::RegisteredVoxelGrid;
    Voxel::WriterCode writer;
    ImpactRuntime::Contract spawn;
    const bool voxel_ok = Voxel::resolve_writer(base + root, writer);
    const bool spawn_ok = ImpactRuntime::resolve_contract(spawn);
    bool spawn_contract_cases = spawn_ok;
    if (spawn_ok) {
        OriginalCode::Procedure body;
        spawn_contract_cases = OriginalCode::ReadProcedure(spawn.helper, body);
        if (spawn_contract_cases) {
            auto altered = body;
            altered.instructions.back().decoded.mnemonic = ZYDIS_MNEMONIC_NOP;
            spawn_contract_cases = !ImpactRuntime::matches(altered,
                ImpactRuntime::helper_contract_recipe(*fixture_spawn_layout));
        }
        const auto expected_layout = fixture_spawn_layout;
        ImpactRuntime::Contract rejected;
        fixture_spawn_layout.reset();
        spawn_contract_cases = !ImpactRuntime::resolve_contract(rejected) && spawn_contract_cases;
        fixture_spawn_layout = expected_layout;
        ++fixture_spawn_layout->source_entity_ids;
        spawn_contract_cases = !ImpactRuntime::resolve_contract(rejected) && spawn_contract_cases;
        fixture_spawn_layout = expected_layout;
        fixture_spawn_layout->allocation_failure_marker ^= 1;
        spawn_contract_cases = !ImpactRuntime::resolve_contract(rejected) && spawn_contract_cases;
        fixture_spawn_layout = expected_layout;
        spawn_contract_cases = ImpactRuntime::resolve_contract(rejected) && spawn_contract_cases;
    }
    nlohmann::json result{{"engine_calls",0},{"world_available",false},
        {"voxel_contract",voxel_ok},{"spawn_contract",spawn_ok},
        {"spawn_profile_contract_cases",spawn_contract_cases}};
    const bool retry_ok = check_public_write_retry();
    result["public_write_retry_cases"] = retry_ok;
    const bool spawn_retry_ok=check_public_spawn_retry();
    result["public_spawn_retry_cases"] = spawn_retry_ok;
    if(!voxel_ok) {
        OriginalCode::Procedure entry;
        if(OriginalCode::ReadProcedure(base+root,entry)) {
            result["root_indirect_jumps"]=nlohmann::json::array();
            for(std::size_t i=0;i<entry.instructions.size();++i) {
                const auto& jump=entry.instructions[i];
                if(jump.decoded.meta.category!=ZYDIS_CATEGORY_UNCOND_BR || jump.operands[0].type==ZYDIS_OPERAND_TYPE_IMMEDIATE)continue;
                OriginalCode::Procedure window;
                for(std::size_t j=i>12?i-12:0;j<=i;++j)window.instructions.push_back(entry.instructions[j]);
                auto item=nlohmann::json{{"rva",jump.address-base},{"code",ImpactRuntime::canonical(window)}};
                for(const auto& ins:window.instructions)for(unsigned j=0;j<ins.decoded.operand_count_visible;++j)
                    if(ins.operands[j].type==ZYDIS_OPERAND_TYPE_MEMORY && ins.operands[j].mem.base==ZYDIS_REGISTER_RIP) {
                        std::uintptr_t target{};if(ImpactRuntime::absolute(ins,ins.operands[j],target))item["rip_targets"].push_back(target-base);
                    }
                result["root_indirect_jumps"].push_back(item);
            }
        }
    }
    if(writer_argument) {
        const auto candidate=std::wcstoull(argv[3],&end,0);
        OriginalCode::Procedure body;
        if(!*end && candidate<nt->OptionalHeader.SizeOfImage && OriginalCode::ReadProcedure(base+candidate,body)) {
            result["direct_writer_contract"]=Voxel::writer_contract(body);
            result["direct_writer_code"]=ImpactRuntime::canonical(body);
            if(!body.calls.empty()) {
                OriginalCode::Procedure commit;
                if(OriginalCode::ReadProcedure(body.calls.front(),commit))result["direct_commit_contract"]=Voxel::commit_contract(commit);
            }
        }
    }
    if (voxel_ok) result["voxel_writer_rva"] = writer.writer - base;
    if (spawn_ok) result["prop_spawn_helper_rva"] = spawn.helper - base;
    result["voxel_evidence_read_blocks"] = writer.bytes.size();
    result["spawn_evidence_read_blocks"] = spawn.code.size();
    result["voxel_evidence_bytes"] = std::accumulate(writer.bytes.begin(),writer.bytes.end(),std::size_t{},
        [](std::size_t count,const auto& block){return count+block.bytes.size();});
    result["spawn_evidence_bytes"] = std::accumulate(spawn.code.begin(),spawn.code.end(),std::size_t{},
        [](std::size_t count,const auto& block){return count+block.bytes.size();});
    // Negative argument cases exercise the real production input validator.
    ImpactRuntime::OwnedInputs input;
    const std::uint64_t uuid[2]{1,2};
    const double position[3]{1,2,3}, rotation[4]{0,0,0,1};
    bool inputs_ok = ImpactRuntime::prepare_inputs(uuid,position,rotation,42,0,input) &&
        !ImpactRuntime::prepare_inputs(uuid,position,rotation,42,1,input) &&
        !ImpactRuntime::prepare_inputs(uuid,position,rotation,0,0,input);
    const double too_large[3]{2147483648.0,0,0}, zero_rotation[4]{};
    inputs_ok = inputs_ok && !ImpactRuntime::prepare_inputs(uuid,too_large,rotation,42,0,input) &&
        !ImpactRuntime::prepare_inputs(uuid,position,zero_rotation,42,0,input);
    result["argument_cases"] = inputs_ok;
    // On failure publish narrowly bounded decoded evidence for investigation.
    if (!voxel_ok) {
        std::uintptr_t candidate{};
        const bool found = OriginalCode::FindUniqueReachable(base + root, 6, [](const OriginalCode::Procedure& code) {
            if (code.calls.size()!=1 || code.indirect_calls) return false;
            for (const auto& ins : code.instructions) for (unsigned i=0;i<ins.decoded.operand_count_visible;++i)
                if (ins.operands[i].type==ZYDIS_OPERAND_TYPE_MEMORY && ins.operands[i].mem.disp.value==0x1220) return true;
            return false;
        },candidate);
        if (found) {
            OriginalCode::Procedure body;
            OriginalCode::ReadProcedure(candidate,body);
            result["voxel_candidate_rva"] = candidate-base;
            result["voxel_candidate"] = ImpactRuntime::canonical(body);
        }
    }
    if (!spawn_ok) {
        std::uintptr_t entry{}, candidate{};
        if (OriginalCode::ResolveNamedSystemCode("player_building_place_prop",entry) &&
            OriginalCode::FindUniqueReachable(entry,2,[](const OriginalCode::Procedure& code) {
                const auto text=ImpactRuntime::canonical(code);
                return text.find("#DisableRandomLootOnDestroy")!=std::string::npos && text.find("#UsedItem")!=std::string::npos;
            },candidate)) {
            OriginalCode::Procedure body;
            OriginalCode::ReadProcedure(candidate,body);
            result["spawn_candidate_rva"] = candidate-base;
            result["spawn_candidate"] = ImpactRuntime::canonical(body);
            result["spawn_direct_calls"] = body.calls.size();
            const auto compare = [&](const char* role,const OriginalCode::Procedure& code,std::string_view recipe) {
                const auto actual=ImpactRuntime::canonical(code);
                nlohmann::json item{{"match",actual==recipe},{"rva",code.entry-base},{"calls",code.calls.size()}};
                if (actual!=recipe) {
                    std::istringstream left(actual),right{std::string(recipe)};
                    std::string a,b;unsigned line=0;
                    while (std::getline(left,a) && std::getline(right,b)) { ++line;if(a!=b) { item["line"]=line;item["actual"]=a;item["expected"]=b;break; } }
                    item["actual_size"]=actual.size();item["expected_size"]=recipe.size();
                }
                result["spawn_stages"][role]=item;
            };
            compare("body",body,ImpactRuntime::helper_contract_recipe(*fixture_spawn_layout));
            if(body.calls.size()>=4) {
                OriginalCode::Procedure q,s,t,p,a,alloc;
                const auto read=[&](std::uintptr_t at,OriginalCode::Procedure& code){return OriginalCode::ReadProcedure(at,code);};
                if(read(body.calls[0],q))compare("queue",q,ImpactRuntime::queue_recipe);
                if(read(body.calls[1],s))compare("source",s,ImpactRuntime::source_contract_recipe(*fixture_spawn_layout));
                if(read(body.calls[2],t))compare("transform",t,ImpactRuntime::transform_recipe);
                if(read(body.calls[3],p)) {
                    compare("prepare",p,ImpactRuntime::prepare_recipe);
                    if(!p.calls.empty() && read(p.calls[0],a)) {
                        compare("append",a,ImpactRuntime::append_recipe);
                        if(a.calls.size()>=2 && read(a.calls[1],alloc))compare("allocate",alloc,ImpactRuntime::allocate_recipe);
                    }
                }
            }
        }
    }
    std::puts(result.dump(2).c_str());
    return voxel_ok && spawn_ok && spawn_contract_cases && inputs_ok && retry_ok && spawn_retry_ok ? 0 : 1;
}
