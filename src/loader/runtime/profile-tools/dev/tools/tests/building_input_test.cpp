#include "building_input.h"
#include "ecs_runtime.h"
#include "profile.h"
#include <windows.h>
#include <array>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>

extern "C" std::uint32_t __cdecl KfcRuntimeWorldBuildingInput(std::uint32_t, std::uint32_t,
    std::uint32_t, std::uint32_t, std::uint32_t, std::uint32_t,
    const double*, const double*, const double*);
extern "C" std::uint32_t __cdecl KfcRuntimeWorldBuildingInputStatus(std::uint32_t);
extern "C" bool __cdecl KfcRuntimeWorldBuildingInputCancel(std::uint32_t);
namespace { const void* expected; unsigned checks; bool enabled = true;
std::uint32_t tick=12345;
void check(bool value) { ++checks; if (!value) { std::fprintf(stderr, "check %u failed\n", checks); std::exit(1); } }
std::uint32_t __fastcall version(void*) { return tick; }
template<class T> T at(const std::array<unsigned char,1392>& bytes, std::size_t offset) {
    T out{}; std::memcpy(&out, bytes.data()+offset, sizeof(out)); return out;
}}
namespace WorldRuntime { bool OperationAvailable(const char*) { return enabled; } }
namespace EcsRuntime { bool MatchesComponentAddress(std::uint32_t handle, const char*, const void* pointer) { return handle == 7 && pointer == expected; } }
int main() {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    RuntimeOperation op; op.name = "runtime.world.building.version"; op.available = true;
    op.function_rva = reinterpret_cast<std::uintptr_t>(&version) - reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    runtime_operations.push_back(op);
    op.name="runtime.world.building.input"; op.abi="client-player-input-v1";
    runtime_operations.push_back(op);
    image_target="enshrouded.exe"; exact_build_match=true;
    std::array<unsigned char,1392> input{}; expected = input.data();
    double position[]{1,2,3}, rotation[]{0,0,0,1}, scale[]{1,2,3};
    auto request = [&](unsigned player, unsigned kind, unsigned item=42) {
        return KfcRuntimeWorldBuildingInput(player,kind,item,88,4,kind==4?99u:0u,position,rotation,scale);
    };
    auto id = request(7,0); check(id != 0);
    BuildingInput::Observe(input.data()+624, input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==3);
    check(at<unsigned>(input,432)==12345 && at<unsigned>(input,440)==42 && input[436]==4);
    check(at<unsigned>(input,164)==12345 && at<unsigned>(input,168)==88 && at<unsigned>(input,172)==88);
    auto before = input;
    id=request(7,1); check(id!=0 && request(7,1)==0);
    std::array<unsigned char,1392> other{};
    BuildingInput::Observe(other.data()+624,other.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==1 && other==std::array<unsigned char,1392>{});
    BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==2);
    check(at<std::int64_t>(input,624)==(1ll<<32) && at<std::int64_t>(input,640)==(3ll<<32));
    check(at<float>(input,672)==3 && (at<std::uint64_t>(input,792)&3)==3);
    check(std::memcmp(input.data(),before.data(),624)==0 && input[736]==before[736]);
    BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==2); // repeated system visit, same tick
    ++tick;
    std::memset(input.data()+624,0,112); // game cursor moved between press/release
    BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==3 && at<std::uint64_t>(input,792)==8);
    check(at<std::int64_t>(input,624)==(1ll<<32)); // release keeps requested pose
    std::uint64_t jump=1ull<<30; std::memcpy(input.data()+792,&jump,8);
    id=request(7,2); BuildingInput::Observe(input.data()+624,input.data());
    check(at<std::uint64_t>(input,792)==(jump|16));
    ++tick; BuildingInput::Observe(input.data()+624,input.data()); check(at<std::uint64_t>(input,792)==jump);
    id=request(7,4); BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==2 && at<std::uint64_t>(input,792)==(jump|(1ull<<5)|(1ull<<7)));
    ++tick; BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==2 && (at<std::uint64_t>(input,792)&((1ull<<5)|(1ull<<7)))!=0);
    Sleep(1250); ++tick; BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==3 && at<std::uint64_t>(input,792)==jump);
    id=request(7,1); check(KfcRuntimeWorldBuildingInputCancel(id));
    before=input; BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==5 && input==before);
    id=request(7,1); BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputCancel(id) && request(7,1)==0);
    ++tick; BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==5 && at<std::uint64_t>(input,792)==(jump|8));
    std::memcpy(input.data()+792,&jump,8);
    before=input; id=request(7,1); BuildingInput::Reset(); BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==5 && input==before);
    check(request(0,1)==0 && request(7,9)==0 && request(7,0,0)==0);
    scale[1]=std::numeric_limits<double>::infinity(); check(request(7,1)==0); scale[1]=1;
    position[1]=std::numeric_limits<double>::quiet_NaN(); check(request(7,1)==0); position[1]=1;
    exact_build_match=false; check(request(7,1)==0); exact_build_match=true;
    image_target="enshrouded_server.exe"; check(request(7,1)==0); image_target="enshrouded.exe";
    std::uint64_t hold=4; std::memcpy(input.data()+792,&hold,8);
    id=request(7,1); before=input; BuildingInput::Observe(input.data()+624,input.data());
    check(KfcRuntimeWorldBuildingInputStatus(id)==1 && input==before); BuildingInput::Reset();
    enabled=false; check(request(7,1)==0);
    std::printf("%u building input checks passed; no game process opened\n",checks);
}
