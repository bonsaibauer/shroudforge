#include "engine-hooks/owned_executable_span.h"
#include "engine-hooks/owned_span_proof.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <memory>
#include <mutex>

namespace EngineHooks {
namespace {
struct SpanWitness {
    std::uint32_t rva{}, count{};
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_MAX_BYTES> original{},replacement{};
    OwnedExecutableSpanVerifier verifier{};
};
bool VerifySpan(const void* context) noexcept {
    const auto& witness=*static_cast<const SpanWitness*>(context);
    return witness.verifier && witness.verifier(witness.rva,witness.original.data(),
        witness.replacement.data(),witness.count);
}

bool ReadProcess(const void* source,void* destination,std::size_t size) noexcept {
    if(!source || !destination || !size)return false;
    SIZE_T read{};
    return ReadProcessMemory(GetCurrentProcess(),source,destination,size,&read) && read==size;
}

bool ExecutableReadable(DWORD protection) noexcept {
    if(protection&(PAGE_GUARD|PAGE_NOACCESS))return false;
    switch(protection&0xff) {
        case PAGE_EXECUTE_READ:
        case PAGE_EXECUTE_READWRITE:
        case PAGE_EXECUTE_WRITECOPY:return true;
        default:return false;
    }
}

bool ReadMainImageHeaders(HMODULE& module,IMAGE_NT_HEADERS64& nt,
    std::array<IMAGE_SECTION_HEADER,96>& sections,std::uint32_t& section_count) noexcept {
    module=GetModuleHandleW(nullptr);
    if(!module)return false;
    IMAGE_DOS_HEADER dos{};
    if(!ReadProcess(module,&dos,sizeof(dos)) || dos.e_magic!=IMAGE_DOS_SIGNATURE || dos.e_lfanew<0 ||
       static_cast<std::uint32_t>(dos.e_lfanew)<sizeof(dos) || static_cast<std::uint32_t>(dos.e_lfanew)>0x100000u)return false;
    const auto base=reinterpret_cast<std::uintptr_t>(module);
    if(base>UINTPTR_MAX-static_cast<std::uint32_t>(dos.e_lfanew))return false;
    const auto nt_address=base+static_cast<std::uint32_t>(dos.e_lfanew);
    if(!ReadProcess(reinterpret_cast<const void*>(nt_address),&nt,sizeof(nt)) || nt.Signature!=IMAGE_NT_SIGNATURE ||
       nt.FileHeader.Machine!=IMAGE_FILE_MACHINE_AMD64 || nt.OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR64_MAGIC ||
       nt.FileHeader.SizeOfOptionalHeader<sizeof(IMAGE_OPTIONAL_HEADER64) || !nt.FileHeader.NumberOfSections ||
       nt.FileHeader.NumberOfSections>sections.size() || !nt.OptionalHeader.SizeOfImage ||
       nt.OptionalHeader.SizeOfHeaders>nt.OptionalHeader.SizeOfImage)return false;
    const auto section_offset=static_cast<std::uint64_t>(dos.e_lfanew)+sizeof(DWORD)+sizeof(IMAGE_FILE_HEADER)+nt.FileHeader.SizeOfOptionalHeader;
    const auto section_bytes=static_cast<std::uint64_t>(nt.FileHeader.NumberOfSections)*sizeof(IMAGE_SECTION_HEADER);
    if(section_offset>nt.OptionalHeader.SizeOfHeaders || section_bytes>nt.OptionalHeader.SizeOfHeaders-section_offset ||
       section_offset>nt.OptionalHeader.SizeOfImage || section_bytes>nt.OptionalHeader.SizeOfImage-section_offset ||
       base>UINTPTR_MAX-section_offset)return false;
    if(!ReadProcess(reinterpret_cast<const void*>(base+section_offset),sections.data(),static_cast<std::size_t>(section_bytes)))return false;
    section_count=nt.FileHeader.NumberOfSections;
    return true;
}

bool InExecutableSection(const std::array<IMAGE_SECTION_HEADER,96>& sections,std::uint32_t section_count,
    std::uint32_t image_size,std::uint32_t rva,std::uint32_t byte_count) noexcept {
    const auto end=static_cast<std::uint64_t>(rva)+byte_count;
    unsigned matches{};
    for(std::uint32_t i=0;i<section_count;++i) {
        const auto& section=sections[i];
        const auto extent=(std::max)(section.Misc.VirtualSize,section.SizeOfRawData);
        const auto begin=static_cast<std::uint64_t>(section.VirtualAddress);
        const auto section_end=begin+extent;
        if(!extent || begin>=image_size || section_end>image_size)continue;
        if((section.Characteristics&IMAGE_SCN_MEM_EXECUTE) && rva>=begin && end<=section_end)++matches;
    }
    return matches==1;
}

bool ReadExecutableSpan(HMODULE module,std::uint32_t rva,std::uint32_t byte_count,
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_MAX_BYTES>& bytes) noexcept {
    const auto base=reinterpret_cast<std::uintptr_t>(module);
    if(base>UINTPTR_MAX-rva || base+rva>UINTPTR_MAX-byte_count)return false;
    auto address=base+rva;
    std::uint32_t copied{};
    while(copied<byte_count) {
        MEMORY_BASIC_INFORMATION region{};
        const auto current=address+copied;
        if(VirtualQuery(reinterpret_cast<const void*>(current),&region,sizeof(region))!=sizeof(region) ||
           region.State!=MEM_COMMIT || region.Type!=MEM_IMAGE || region.AllocationBase!=module ||
           !ExecutableReadable(region.Protect))return false;
        const auto region_begin=reinterpret_cast<std::uintptr_t>(region.BaseAddress);
        if(current<region_begin || region_begin>UINTPTR_MAX-region.RegionSize)return false;
        const auto region_end=region_begin+region.RegionSize;
        if(current>=region_end)return false;
        const auto chunk=static_cast<std::uint32_t>((std::min)(std::uint64_t(byte_count-copied),region_end-current));
        if(!chunk || !ReadProcess(reinterpret_cast<const void*>(current),bytes.data()+copied,chunk))return false;
        copied+=chunk;
    }
    return true;
}
}

bool PublishOwnedExecutableSpan(std::uint32_t rva,std::span<const std::uint8_t> original,
    std::span<const std::uint8_t> replacement,OwnedExecutableSpanVerifier verifier) noexcept {
    if(!verifier || original.empty() || !original.data() || !replacement.data() ||
       original.size()>SF_EXECUTABLE_SPAN_MAX_BYTES ||
       original.size()!=replacement.size() || rva>UINT32_MAX-original.size() ||
       !std::memcmp(original.data(),replacement.data(),original.size()))return false;
    try {
        auto witness=std::make_shared<SpanWitness>();
        witness->rva=rva;witness->count=static_cast<std::uint32_t>(original.size());
        std::copy(original.begin(),original.end(),witness->original.begin());
        std::copy(replacement.begin(),replacement.end(),witness->replacement.begin());
        witness->verifier=verifier;
        auto& proofs=ExecutableProofs();
        if(proofs.Publish(rva,original,replacement,witness,VerifySpan))return true;
        // Preserve idempotent framework publication, including concurrent
        // duplicates, without retaining a second collection or invoking code
        // under the common lifetime gate.
        std::lock_guard lock(proofs.Gate());
        const auto existing=proofs.FindLocked(proofs.SessionLocked(),rva,original);
        return existing && existing->verify==VerifySpan && existing->witness &&
            !std::memcmp(existing->replacement.data(),replacement.data(),replacement.size()) &&
            static_cast<const SpanWitness*>(existing->witness.get())->verifier==verifier;
    } catch(...) { return false; }
}

SFExecutableSpanStatusV1 QueryMainImageExecutableSpan(const SFExecutableSpanQueryV1& request,
    SFExecutableSpanResultV1& result) noexcept {
    if(request.size<sizeof(request) || request.version!=SF_EXECUTABLE_SPAN_SERVICE_VERSION ||
       !request.byte_count || request.byte_count>SF_EXECUTABLE_SPAN_MAX_BYTES ||
       request.rva>UINT32_MAX-request.byte_count)return SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT;
    try {
        HMODULE module{};IMAGE_NT_HEADERS64 nt{};std::array<IMAGE_SECTION_HEADER,96> sections{};std::uint32_t section_count{};
        if(!ReadMainImageHeaders(module,nt,sections,section_count))return SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE;
        const auto image_size=nt.OptionalHeader.SizeOfImage;
        if(request.rva>image_size || request.byte_count>image_size-request.rva)return SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT;

        if(!InExecutableSection(sections,section_count,image_size,request.rva,request.byte_count))
            return SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE;
        std::array<std::uint8_t,SF_EXECUTABLE_SPAN_MAX_BYTES> current{};
        if(!ReadExecutableSpan(module,request.rva,request.byte_count,current))return SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE;
        if(std::memcmp(current.data(),request.observed_current,request.byte_count))return SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE;

        auto& proofs=ExecutableProofs();
        std::shared_ptr<const OwnedSpanProofs::Record> exact;bool ambiguous{};
        {
            std::lock_guard lock(proofs.Gate());
            exact=proofs.FindRangeLocked(proofs.SessionLocked(),request.rva,request.byte_count,ambiguous);
        }

        std::uint32_t classification=SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE;
        const bool original_matches=!std::memcmp(current.data(),request.expected_original,request.byte_count);
        if(ambiguous)classification=SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE;
        else if(original_matches)classification=SF_EXECUTABLE_SPAN_UNCHANGED;
        else if(exact && exact->verify && exact->witness &&
            !std::memcmp(exact->original.data(),request.expected_original,request.byte_count) &&
            !std::memcmp(exact->replacement.data(),current.data(),request.byte_count) &&
            exact->verify(exact->witness.get())) {
            classification=SF_EXECUTABLE_SPAN_VERIFIED_HOST_OWNED;
        }
        std::array<std::uint8_t,SF_EXECUTABLE_SPAN_MAX_BYTES> after_validation{};
        if(!ReadExecutableSpan(module,request.rva,request.byte_count,after_validation) ||
           std::memcmp(after_validation.data(),current.data(),request.byte_count))return SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE;
        std::lock_guard lock(proofs.Gate());
        if(classification==SF_EXECUTABLE_SPAN_VERIFIED_HOST_OWNED && !proofs.LiveLocked(exact))
            classification=SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE;
        result={sizeof(result),classification};
        return SF_EXECUTABLE_SPAN_STATUS_OK;
    } catch(...) { return SF_EXECUTABLE_SPAN_STATUS_FAILED; }
}
}
