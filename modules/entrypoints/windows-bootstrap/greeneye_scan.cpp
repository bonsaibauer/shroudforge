// Exact dynamic-attach contract. Validates original scan identity; NEVER calls it.
#include "greeneye_scan.h"
#include "bootstrap_state.h"
#include <bcrypt.h>
#include <array>
#include <vector>
#pragma comment(lib,"bcrypt.lib")
namespace GreeneyeDynamic {
namespace {
constexpr std::size_t ImageSize=0x4e000,FileSize=294912;
constexpr char FileHash[]="973a2284ba4faf24e55c9e4b165c136c81dec4d3b1706b3f21f9cbd314c26eed";
struct Body {unsigned begin,end,unwind;const char* hash;};
constexpr Body Bodies[]{
 {0x4580,0x4f1b,0x39bac,"772517ef7693e20209c57ad44cc1782c58a62b43977bb8d4ae72d55760d6f878"},
 {0xe6a8,0xe7d0,0x3ad14,"7492eda24de99586dcc7aa287b28904ffdefcaeb1cfd9b7afd0d2d5fa87b42d1"},
 {0xe7d0,0xe80d,0x39fc0,"0f5a69b3d244d9e4e62e2eb1ee9644586a92b3ad83a50a35025a5bb3fa1b3b30"},
 {0x2b50,0x3fb0,0x39a38,"ced2ce8672019fe9f6c53bce5f06017f0e0a5f321ae5c08d3f052ec95b493a01"}
};
struct State {
 std::atomic<unsigned> phase{}; // 0 absent,1 loading,2 witnessed,3 finalizing,4 complete,5 failed
 std::atomic<const char*> failure{"none"};
 std::uintptr_t module{};DWORD thread{},process{};HANDLE file{};
 unsigned char* entry{};std::array<unsigned char,14> original{};
};
State state; // one operation; accepted references stay resident until process exit
bool Refuse(const char* reason) noexcept {
 state.failure.store(reason,std::memory_order_release);state.phase.store(5,std::memory_order_release);return false;
}
bool Readable(std::uintptr_t address,std::size_t length,std::uintptr_t image=0,bool execute=false) noexcept {
 if(!address||!length||address>UINTPTR_MAX-length)return false;
 while(length) {
  MEMORY_BASIC_INFORMATION m{};
  if(!VirtualQuery(reinterpret_cast<void*>(address),&m,sizeof(m))||m.State!=MEM_COMMIT||
   (m.Protect&(PAGE_NOACCESS|PAGE_GUARD))||!m.Protect||
   (image&&(m.Type!=MEM_IMAGE||reinterpret_cast<std::uintptr_t>(m.AllocationBase)!=image)))return false;
  const auto p=m.Protect&0xff;
  if(p!=PAGE_READONLY&&p!=PAGE_READWRITE&&p!=PAGE_WRITECOPY&&p!=PAGE_EXECUTE_READ&&
   p!=PAGE_EXECUTE_READWRITE&&p!=PAGE_EXECUTE_WRITECOPY)return false;
  if(execute&&p!=PAGE_EXECUTE_READ&&p!=PAGE_EXECUTE_WRITECOPY)return false;
  const auto end=reinterpret_cast<std::uintptr_t>(m.BaseAddress)+m.RegionSize;
  if(end<=address)return false;
  const auto part=(std::min)(length,static_cast<std::size_t>(end-address));address+=part;length-=part;
 }
 return true;
}
bool Hash(const void* bytes,std::size_t length,const char* expected) noexcept {
 BCRYPT_ALG_HANDLE algorithm{};BCRYPT_HASH_HANDLE hash{};std::array<unsigned char,32> digest{};
 bool ok=length<=ULONG_MAX&&BCryptOpenAlgorithmProvider(&algorithm,BCRYPT_SHA256_ALGORITHM,nullptr,0)>=0;
 if(ok)ok=BCryptCreateHash(algorithm,&hash,nullptr,0,nullptr,0,0)>=0;
 if(ok)ok=BCryptHashData(hash,static_cast<PUCHAR>(const_cast<void*>(bytes)),static_cast<ULONG>(length),0)>=0&&
  BCryptFinishHash(hash,digest.data(),static_cast<ULONG>(digest.size()),0)>=0;
 if(hash)BCryptDestroyHash(hash);if(algorithm)BCryptCloseAlgorithmProvider(algorithm,0);
 constexpr char hex[]="0123456789abcdef";
 if(ok)for(std::size_t i=0;i<digest.size();++i)
  if(expected[i*2]!=hex[digest[i]>>4]||expected[i*2+1]!=hex[digest[i]&15])return false;
 return ok;
}
bool FileIdentity(std::uintptr_t module) {
 wchar_t path[32768]{};
#ifdef SHROUDFORGE_DYNAMIC_TEST
 const auto count=DynamicTestModulePath(module,path,32768);
#else
 const auto count=GetModuleFileNameW(reinterpret_cast<HMODULE>(module),path,32768);
#endif
 if(!count||count>=32768)return false;
 state.file=CreateFileW(path,GENERIC_READ,FILE_SHARE_READ,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,nullptr);
 if(state.file==INVALID_HANDLE_VALUE)return false;
 BY_HANDLE_FILE_INFORMATION before{},after{};
 if(!GetFileInformationByHandle(state.file,&before)||before.nFileSizeHigh||before.nFileSizeLow!=FileSize)return false;
 std::vector<unsigned char> bytes(FileSize);DWORD read{};
 return ReadFile(state.file,bytes.data(),static_cast<DWORD>(bytes.size()),&read,nullptr)&&read==FileSize&&
  Hash(bytes.data(),bytes.size(),FileHash)&&GetFileInformationByHandle(state.file,&after)&&
  before.dwVolumeSerialNumber==after.dwVolumeSerialNumber&&before.nFileIndexHigh==after.nFileIndexHigh&&
  before.nFileIndexLow==after.nFileIndexLow&&before.nFileSizeHigh==after.nFileSizeHigh&&before.nFileSizeLow==after.nFileSizeLow&&
  CompareFileTime(&before.ftLastWriteTime,&after.ftLastWriteTime)==0;
}
bool ImageIdentity(std::uintptr_t module) noexcept {
 if(module>UINTPTR_MAX-ImageSize||!Readable(module,sizeof(IMAGE_DOS_HEADER),module))return false;
 const auto* dos=reinterpret_cast<const IMAGE_DOS_HEADER*>(module);
 if(dos->e_magic!=IMAGE_DOS_SIGNATURE||dos->e_lfanew<=0||
  static_cast<std::size_t>(dos->e_lfanew)>4096-sizeof(IMAGE_NT_HEADERS64))return false;
 if(!Readable(module+dos->e_lfanew,sizeof(IMAGE_NT_HEADERS64),module))return false;
 const auto* nt=reinterpret_cast<const IMAGE_NT_HEADERS64*>(module+dos->e_lfanew);
 if(nt->Signature!=IMAGE_NT_SIGNATURE||nt->FileHeader.Machine!=IMAGE_FILE_MACHINE_AMD64||
  nt->FileHeader.TimeDateStamp!=0x6aeb9745||nt->FileHeader.SizeOfOptionalHeader!=sizeof(IMAGE_OPTIONAL_HEADER64)||
  nt->OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR64_MAGIC||nt->OptionalHeader.SizeOfImage!=ImageSize||
  nt->OptionalHeader.AddressOfEntryPoint!=0xe7d0||nt->OptionalHeader.NumberOfRvaAndSizes<=IMAGE_DIRECTORY_ENTRY_EXCEPTION)return false;
 const auto& dir=nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
 if(!dir.Size||dir.Size%sizeof(RUNTIME_FUNCTION)||dir.Size>ImageSize||dir.VirtualAddress>ImageSize-dir.Size||
  !Readable(module+dir.VirtualAddress,dir.Size,module))return false;
 const auto* functions=reinterpret_cast<const RUNTIME_FUNCTION*>(module+dir.VirtualAddress);
 for(const auto& body:Bodies) {
  unsigned found=0;
  for(std::size_t i=0;i<dir.Size/sizeof(RUNTIME_FUNCTION);++i) {
   const auto& f=functions[i];
   if(f.BeginAddress<=body.begin&&body.begin<f.EndAddress) {
    if(f.BeginAddress!=body.begin||f.EndAddress!=body.end||f.UnwindData!=body.unwind)return false;
    ++found;
   }
  }
  if(found!=1||!Readable(module+body.begin,body.end-body.begin,module,true)||
   !Hash(reinterpret_cast<void*>(module+body.begin),body.end-body.begin,body.hash))return false;
 }
 return true;
}
bool CurrentEntry() noexcept {
 return GetCurrentThreadId()==state.thread&&GetCurrentProcessId()==state.process&&
  StartupGate::parked_thread_id.load(std::memory_order_acquire)==state.thread&&
  StartupGate::post_restore_state.load(std::memory_order_acquire)==2&&state.entry==StartupGate::entrypoint&&
  Readable(reinterpret_cast<std::uintptr_t>(state.entry),14,0,true)&&
  !std::memcmp(state.original.data(),StartupGate::original,14)&&!std::memcmp(state.entry,state.original.data(),14);
}
bool DynamicState() noexcept {
 // scanStarted is NOT a completion signal alone. A fresh successful Rust load
 // returned after exact CRT/DllMain's NULL-reserved synchronous scan at4E66.
 // Static attach sets the saved EP slot and is a different contract.
 if(!Readable(state.module+0x46ce8,0x1c,state.module))return false;
 unsigned role{};std::uintptr_t originalEntry{};std::array<unsigned char,12> saved{};
 std::memcpy(&role,reinterpret_cast<void*>(state.module+0x46ce8),4);
 std::memcpy(&originalEntry,reinterpret_cast<void*>(state.module+0x46cf0),8);
 std::memcpy(saved.data(),reinterpret_cast<void*>(state.module+0x46cf8),12);
 return role==1&&*reinterpret_cast<const unsigned char*>(state.module+0x46cec)==1&&
  !originalEntry&&saved==std::array<unsigned char,12>{};
}
}
const char* Failure() noexcept {return state.failure.load(std::memory_order_acquire);}
bool Begin(unsigned char* entry,const unsigned char* original) noexcept {
 unsigned absent=0;
 if(!state.phase.compare_exchange_strong(absent,1,std::memory_order_acq_rel))return false;
 state.thread=GetCurrentThreadId();state.process=GetCurrentProcessId();state.entry=entry;
 if(!original)return Refuse("dynamic-original-bytes-unavailable");
 std::memcpy(state.original.data(),original,14);
 if(!CurrentEntry())return Refuse("dynamic-entry-not-restored-or-continuation-expired");
 if(!EcsRuntime::RequireOriginalOwners())return Refuse("dynamic-native-pending-gate-refused");
 return CurrentEntry()||Refuse("dynamic-entry-expired-before-load");
}
bool Witness(std::uintptr_t module) noexcept {
 try {
  if(state.phase.load(std::memory_order_acquire)!=1||!CurrentEntry()||!module)return Refuse("dynamic-load-return-or-entry-refused");
  state.module=module;HMODULE host{},self{};
#ifdef SHROUDFORGE_DYNAMIC_TEST
  if(!DynamicTestPin(module,&host,&self))return Refuse("dynamic-module-residency-refused");
#else
  if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_PIN,reinterpret_cast<LPCWSTR>(module),&host)||
   reinterpret_cast<std::uintptr_t>(host)!=module||!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_PIN,
    reinterpret_cast<LPCWSTR>(&Witness),&self))return Refuse("dynamic-module-residency-refused");
#endif
  if(!FileIdentity(module))return Refuse("dynamic-original-host-file-differs");
  if(!ImageIdentity(module))return Refuse("dynamic-original-host-control-flow-differs");
  if(!DynamicState())return Refuse("dynamic-attach-scan-state-differs");
  if(!CurrentEntry())return Refuse("dynamic-entry-changed-after-load");
  unsigned expected=1;
  return state.phase.compare_exchange_strong(expected,2,std::memory_order_acq_rel)||Refuse("dynamic-witness-repeated");
 }catch(...) {return Refuse("dynamic-witness-exception");}
}
bool Finalize() noexcept {
 unsigned expected=2;
 if(!state.phase.compare_exchange_strong(expected,3,std::memory_order_acq_rel))return Refuse("dynamic-finalization-repeated-or-unwitnessed");
 if(!CurrentEntry()||!ImageIdentity(state.module)||!DynamicState())return Refuse("dynamic-owner-changed-before-finalize");
 try {
  if(!EcsRuntime::FinalizeOriginalOwners())return Refuse("dynamic-native-finalization-refused");
  if(!CurrentEntry()||!ImageIdentity(state.module)||!DynamicState())return Refuse("dynamic-owner-changed-or-expired-after-finalize");
  state.phase.store(4,std::memory_order_release);return true;
 }catch(...) {return Refuse("dynamic-native-finalization-exception");}
}
}
