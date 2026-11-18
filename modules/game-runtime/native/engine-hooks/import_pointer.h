#pragma once
#include <windows.h>
#include <span>
#include <cstdint>
#include <cstring>
#include <cctype>
namespace EngineHooks {
// Import data cells are replaced atomically; no executable instruction is copied.
// Owners must pin callbacks before publication and keep forwarding state forever.
inline bool ImportRead(const void* p, void* out, std::size_t bytes) {
    __try { std::memcpy(out,p,bytes);return true; } __except(EXCEPTION_EXECUTE_HANDLER) {return false;}
}
inline std::uintptr_t* FindNamedImport(std::span<std::uint8_t> image,const char* dll,const char* name) {
    const auto fits=[&](std::uint64_t r,std::uint64_t n){return r<=image.size() && n<=image.size()-r;};
    const auto read=[&](std::uint64_t r,auto& value){return fits(r,sizeof(value)) && ImportRead(image.data()+r,&value,sizeof(value));};
    const auto equal=[&](std::uint64_t r,const char* expected,bool insensitive){
        for(std::size_t i=0;i<512;++i){char c{};if(!read(r+i,c))return false;
            const auto a=insensitive?std::tolower(static_cast<unsigned char>(c)):c;
            const auto b=insensitive?std::tolower(static_cast<unsigned char>(expected[i])):expected[i];
            if(a!=b)return false;if(!c)return true;}return false;};
    IMAGE_DOS_HEADER dos{};IMAGE_NT_HEADERS64 nt{};
    if(!read(0,dos)||dos.e_magic!=IMAGE_DOS_SIGNATURE||dos.e_lfanew<=0||!read(dos.e_lfanew,nt)||
        nt.Signature!=IMAGE_NT_SIGNATURE||nt.OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR64_MAGIC||
        nt.FileHeader.Machine!=IMAGE_FILE_MACHINE_AMD64||(!nt.OptionalHeader.SizeOfImage || nt.OptionalHeader.SizeOfImage>image.size())||
        nt.OptionalHeader.NumberOfRvaAndSizes<=IMAGE_DIRECTORY_ENTRY_IMPORT)return nullptr;
    image=image.first(nt.OptionalHeader.SizeOfImage);
    const auto dir=nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if(!dir.VirtualAddress||!fits(dir.VirtualAddress,dir.Size))return nullptr;
    std::uintptr_t* result{};bool terminated{};
    for(std::size_t off=0;off+sizeof(IMAGE_IMPORT_DESCRIPTOR)<=dir.Size;off+=sizeof(IMAGE_IMPORT_DESCRIPTOR)){
        IMAGE_IMPORT_DESCRIPTOR d{};if(!read(dir.VirtualAddress+off,d))return nullptr;
        if(!d.Name && !d.FirstThunk && !d.OriginalFirstThunk){terminated=true;break;}
        if(!equal(d.Name,dll,true))continue;
        if(!d.OriginalFirstThunk||!d.FirstThunk)return nullptr;
        bool thunk_end{};
        for(std::size_t i=0;i<4096;++i){IMAGE_THUNK_DATA64 t{};std::uintptr_t value{};
            const auto cell=std::uint64_t(d.FirstThunk)+i*8;
            if(!read(std::uint64_t(d.OriginalFirstThunk)+i*8,t)||!read(cell,value))return nullptr;
            if(!t.u1.AddressOfData){thunk_end=true;break;}if(IMAGE_SNAP_BY_ORDINAL64(t.u1.Ordinal))continue;
            if(t.u1.AddressOfData>image.size() || !fits(t.u1.AddressOfData,3))return nullptr;
            if(!equal(t.u1.AddressOfData+2,name,false))continue;
            if(result||cell%alignof(void*)||!value)return nullptr;
            result=reinterpret_cast<std::uintptr_t*>(image.data()+cell);
        }if(!thunk_end)return nullptr;
    }return terminated?result:nullptr;
}
inline bool ReplaceImportPointer(std::uintptr_t* cell,std::uintptr_t expected,std::uintptr_t replacement) {
    if(!cell || reinterpret_cast<std::uintptr_t>(cell)%alignof(void*) || !expected || !replacement)return false;
    DWORD protection{};if(!VirtualProtect(cell,sizeof(void*),PAGE_READWRITE,&protection))return false;
    const auto prior=InterlockedCompareExchangePointer(reinterpret_cast<void*volatile*>(cell),reinterpret_cast<void*>(replacement),reinterpret_cast<void*>(expected));
    DWORD ignored{};if(!VirtualProtect(cell,sizeof(void*),protection,&ignored)){
        if(reinterpret_cast<std::uintptr_t>(prior)==expected)InterlockedCompareExchangePointer(reinterpret_cast<void*volatile*>(cell),reinterpret_cast<void*>(expected),reinterpret_cast<void*>(replacement));
        VirtualProtect(cell,sizeof(void*),protection,&ignored);return false;
    }return reinterpret_cast<std::uintptr_t>(prior)==expected;
}
}
