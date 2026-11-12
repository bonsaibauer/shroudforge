#pragma once
#include "map/public_creative_compat.h"
#include "engine-hooks/held_entry_preparation.h"
// Creative 3.2 MapDiscovery uses its A8-byte GenericHook, not the HUD emitter.
// This private admission recognizes only the proved near6/far15 native prefix.
// It never calls a foreign callback, changes an original hook or skips its tail.
namespace MapRuntime::CreativeFogOwner {
using namespace PublicCreativeCompat;
inline constexpr std::uintptr_t Target=0x3a2780,Singleton=0x58b498,Callback=0x13d27;
inline constexpr unsigned char OriginalPrefix[]{0x40,0x53,0x56,0x57,0x41,0x56,0x41,0x57,0x48,0x81,0xec,0x80,0,0,0};
struct Snapshot {
    std::uintptr_t module{},object{},block{};
    unsigned stolen{},mode{};
    std::array<unsigned char,15> entry{},saved{};
    std::array<unsigned char,0xa8> model{};
    std::array<unsigned char,29> continuation{};
    bool operator==(const Snapshot&)const=default;
};
inline bool Executable(std::uintptr_t at,std::size_t size) {
    MEMORY_BASIC_INFORMATION info{};
    if(!Range(at,size) || VirtualQuery(reinterpret_cast<void*>(at),&info,sizeof(info))!=sizeof(info) || info.State!=MEM_COMMIT ||
        (info.Protect&(PAGE_GUARD|PAGE_NOACCESS)))return false;
    const auto start=reinterpret_cast<std::uintptr_t>(info.BaseAddress);
    return at>=start && at-start<=info.RegionSize && size<=info.RegionSize-(at-start) && ReadableProtection(info.Protect,true);
}
inline bool CallbackBytes(std::uintptr_t module) {
    std::array<unsigned char,5> thunk{};std::array<unsigned char,0x243> body{};std::array<unsigned char,0x8c> wrapper{};
    std::uintptr_t destination{};
    return Copy(module+Callback,thunk.data(),thunk.size(),true) && thunk[0]==0xe9 &&
        Relative(module+Callback,Word<std::int32_t>(thunk.data(),1),destination) && destination==module+0x1f38e0 &&
        Copy(module+0x1f38e0,body.data(),body.size(),true) && HasHash(body.data(),body.size(),"88eccb32108c542ed1dac0bdfe664e0898c11c22acbe90a9db99a40682cb8192") &&
        Copy(module+0x1f3d00,wrapper.data(),wrapper.size(),true) && HasHash(wrapper.data(),wrapper.size(),"7ef24a6f2c6721dbf5bc654edde285b2b5c10f650094e26d414adeb1102fd0bb");
}
inline bool Capture(std::uintptr_t image,Snapshot& out) {
    Snapshot value;const auto entry=image+Target;
    if(!Copy(entry,value.entry.data(),value.entry.size(),true))return false;
    constexpr unsigned char jump[]{0xff,0x25,0,0,0,0};
    std::uintptr_t callback{},relay{};
    if(value.entry[0]==0xe9){
        value.mode=1;value.stolen=6;
        std::array<unsigned char,14> bytes{};
        if(value.entry[5]!=0x90 || !Relative(entry,Word<std::int32_t>(value.entry.data(),1),relay) ||
            !Copy(relay,bytes.data(),bytes.size(),true) || std::memcmp(bytes.data(),jump,6))return false;
        callback=Word<std::uintptr_t>(bytes.data(),6);
        // Untouched bytes after the six-byte near detour are mandatory too.
        if(std::memcmp(value.entry.data()+6,OriginalPrefix+6,9))return false;
    }else{
        value.mode=2;value.stolen=15;
        if(std::memcmp(value.entry.data(),jump,6) || value.entry[14]!=0x90)return false;
        callback=Word<std::uintptr_t>(value.entry.data(),6);
    }
    HMODULE reference{};
    if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,reinterpret_cast<LPCWSTR>(callback),&reference))return false;
    struct Release {HMODULE value;~Release(){FreeLibrary(value);}} release{reference};
    value.module=reinterpret_cast<std::uintptr_t>(reference);
    if(callback!=value.module+Callback || !CallbackBytes(value.module) ||
        !Copy(value.module+Singleton,&value.object,8) || (value.object&7) || !Range(value.object,0x1a0) ||
        !Copy(value.object,value.model.data(),value.model.size()))return false;
    const auto field=[&](std::size_t offset){return Word<std::uintptr_t>(value.model.data(),offset);};
    value.block=field(0x20);const auto backup=field(0x30);const auto length=value.stolen+(value.mode==1?19:14);
    if(field(0)!=entry || field(8)!=callback || !Range(field(0x10),1) || !Range(field(0x18),1) ||
        field(0x28)!=length || field(0x48)!=length || value.model[0xa0]!=1 || Word<unsigned>(value.model.data(),0xa4)!=value.mode ||
        !Range(backup,value.stolen) || field(0x38)!=backup+value.stolen || field(0x40)<field(0x38) || field(0x40)-backup>64 ||
        !Executable(value.block,length) || !Copy(value.block,value.continuation.data(),length,true) ||
        !Copy(backup,value.saved.data(),value.stolen) || std::memcmp(value.saved.data(),OriginalPrefix,value.stolen) ||
        std::memcmp(value.continuation.data(),OriginalPrefix,value.stolen))return false;
    if(value.mode==1){
        std::uintptr_t resume{};
        if(relay!=value.block+11 || value.continuation[6]!=0xe9 ||
            !Relative(value.block+6,Word<std::int32_t>(value.continuation.data(),7),resume) || resume!=entry+6 ||
            std::memcmp(value.continuation.data()+11,jump,6) || Word<std::uintptr_t>(value.continuation.data(),17)!=callback)return false;
    }else if(std::memcmp(value.continuation.data()+15,jump,6) || Word<std::uintptr_t>(value.continuation.data(),21)!=entry+15)return false;
    // The original callback owns the mutex at +50. Its lock state is not an
    // identity field; retain only the hook tuple and active/mode fields.
    std::fill(value.model.begin()+0x50,value.model.begin()+0xa0,0);
    out=value;return true;
}
class Admission final {
    HMODULE reference_{};
    std::uintptr_t image_{};
    bool attempted_{},native_{};
    mutable std::atomic<bool> revoked_{};
    const char* reason_="not-started";
public:
    Admission()=default;
    Admission(const Admission&)=delete;
    Admission& operator=(const Admission&)=delete;
    const char* Reason()const noexcept{return reason_;}
    bool Prepare(std::uintptr_t image,const EngineHooks::HeldEntryToken& token) {
        if(attempted_ || !token.ValidForCurrentThread()){reason_="preconditions";return false;}
        attempted_=true;image_=image;
        std::array<unsigned char,15> original{};
        if(Copy(image+Target,original.data(),original.size(),true) && !std::memcmp(original.data(),OriginalPrefix,original.size())){
            native_=true;reason_="native";return true;
        }
        reason_="owner-before";Snapshot before,after;
        if(!Capture(image,before))return false;
        HMODULE reference{};
        reason_="module-reference";
        if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,reinterpret_cast<LPCWSTR>(before.module+Callback),&reference))return false;
        struct Release {HMODULE value;~Release(){if(value)FreeLibrary(value);}} release{reference};
        std::wstring path;BY_HANDLE_FILE_INFORMATION info{};Digest identity{};TrustedImageBytes expected{};
        reason_="publisher-identity";
        if(reinterpret_cast<std::uintptr_t>(reference)!=before.module || !ColdFileIdentity(reference,Creative320,path,info,identity,expected))return false;
        reason_="owner-after";
        if(!Capture(image,after) || before!=after || !token.ValidForCurrentThread())return false;
        // Process-resident DLL reference, not cached foreign object/code pointers.
        reference_=reference;release.value=nullptr;reason_="creative-3.2";return true;
    }
    bool Prefix(std::array<unsigned char,15>& original,std::array<unsigned char,15>* observed=nullptr)const {
        const auto refuse=[&]{revoked_=true;return false;};
        if(!image_ || revoked_)return false;
        if(native_){
            if(!Copy(image_+Target,original.data(),original.size(),true) || std::memcmp(original.data(),OriginalPrefix,original.size()))return refuse();
            if(observed)*observed=original;return true;
        }
        Snapshot before,after;
        if(!reference_ || !Capture(image_,before) || before.module!=reinterpret_cast<std::uintptr_t>(reference_) ||
            !Capture(image_,after) || before!=after)return refuse();
        if(observed)*observed=before.entry;
        std::copy(std::begin(OriginalPrefix),std::end(OriginalPrefix),original.begin());return true;
    }
};
}
