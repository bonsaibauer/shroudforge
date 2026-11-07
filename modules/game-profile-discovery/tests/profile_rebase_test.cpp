// Original-image fixtures with independent code, data, reflection and system
// registrations. No game build IDs or production addresses appear here.
#include "game-profile-discovery/rebase_profile_contract.h"
#include "game-profile-discovery/prove_profile_layout.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <fstream>
#include <iostream>

namespace {
namespace fs = std::filesystem;
using json = nlohmann::json;
using namespace GameProfileDiscovery::Detail;

void check(bool value, const char* message) { if (!value) throw std::runtime_error(message); }

struct FixtureImage {
    std::vector<std::uint8_t> bytes = std::vector<std::uint8_t>(0x4000);
    std::uint32_t shift{}, code_shift{}, global{};
    static constexpr std::uint64_t base = 0x140000000;
    static std::size_t file(std::uint32_t rva) {
        if (rva >= 0x1000 && rva < 0x2000) return 0x400 + rva - 0x1000;
        if (rva >= 0x3000 && rva < 0x5000) return 0x1400 + rva - 0x3000;
        if (rva >= 0x5000 && rva < 0x5400) return 0x3400 + rva - 0x5000;
        if (rva >= 0x6000 && rva < 0x6400) return 0x3800 + rva - 0x6000;
        if (rva >= 0x7000 && rva < 0x7400) return 0x3c00 + rva - 0x7000;
        throw std::runtime_error("fixture rva");
    }
    template<class T> void raw(std::size_t offset, T value) {
        check(offset + sizeof(value) <= bytes.size(), "fixture file extent");
        std::memcpy(bytes.data() + offset, &value, sizeof(value));
    }
    template<class T> void put(std::uint32_t rva, T value) { raw(file(rva), value); }
    void pointer(std::uint32_t at, std::uint32_t target) { put<std::uint64_t>(at, base + target); }
    void text(std::uint32_t at, const std::string& value) {
        std::memcpy(bytes.data() + file(at), value.data(), value.size());
        bytes.at(file(at) + value.size()) = 0;
    }
    void span(std::uint32_t at, std::uint32_t name, const std::string& value) {
        text(name, value); pointer(at, name); put<std::uint64_t>(at + 8, value.size());
    }
    void remove_unwind_entry(unsigned ordinal) {
        unsigned written{};
        for (unsigned i = 0; i != 4; ++i) {
            if (i == ordinal) continue;
            RUNTIME_FUNCTION part{}; part.BeginAddress = function(i); part.EndAddress = function(i) + 48;
            part.UnwindData = 0x7000 + i * 8;
            put(0x6000 + written++ * sizeof(part), part);
        }
        raw(0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
            offsetof(IMAGE_OPTIONAL_HEADER64, DataDirectory) + IMAGE_DIRECTORY_ENTRY_EXCEPTION * sizeof(IMAGE_DATA_DIRECTORY),
            IMAGE_DATA_DIRECTORY{0x6000, 3 * sizeof(RUNTIME_FUNCTION)});
    }
    std::uint32_t function(unsigned ordinal) const { return 0x1000 + code_shift + ordinal * 0x40; }
    void code(unsigned ordinal, std::uint32_t literal, bool call = false, unsigned destination = 1,
              std::uint8_t member = 8, bool root_owner = true) {
        const auto start = function(ordinal);
        std::fill_n(bytes.begin() + file(start), 48, std::uint8_t{0x90});
        put<std::uint8_t>(start, 0xb8); put<std::uint32_t>(start + 1, literal); // mov eax, literal
        put<std::uint8_t>(start + 5, 0x48); put<std::uint8_t>(start + 6, 0x8d); put<std::uint8_t>(start + 7, 0x0d);
        put<std::int32_t>(start + 8, static_cast<std::int32_t>(global + (root_owner ? 0 : 16) - (start + 12))); // lea rcx,[rip+owner]
        put<std::uint8_t>(start + 12, 0x48); put<std::uint8_t>(start + 13, 0x8b);
        put<std::uint8_t>(start + 14, 0x49); put<std::uint8_t>(start + 15, member); // mov rcx,[rcx+member]
        if (call) {
            put<std::uint8_t>(start + 16, 0xe8);
            put<std::int32_t>(start + 17, static_cast<std::int32_t>(function(destination) - (start + 21)));
        }
        put<std::uint8_t>(start + 32, 0xc3);
    }
    void indexed_switch(bool embedded, bool different_target = false) {
        const auto start = function(0);
        const auto table = embedded ? start + 40 : 0x3c00 + shift;
        std::fill_n(bytes.begin() + file(start), 48, std::uint8_t{0x90});
        put<std::uint8_t>(start, 0x83); put<std::uint8_t>(start + 1, 0xf9); put<std::uint8_t>(start + 2, 1); // cmp ecx,1
        put<std::uint16_t>(start + 3, 0x2177); // ja default at+38
        put<std::uint8_t>(start + 5, 0x4c); put<std::uint8_t>(start + 6, 0x8d); put<std::uint8_t>(start + 7, 0x15);
        put<std::int32_t>(start + 8, -static_cast<std::int32_t>(start + 12)); // lea r10,[rip+imagebase]
        put<std::uint8_t>(start + 12, 0x41); put<std::uint8_t>(start + 13, 0x8b);
        put<std::uint8_t>(start + 14, 0x84); put<std::uint8_t>(start + 15, 0x8a);
        put<std::uint32_t>(start + 16, table); // mov eax,[r10+rcx*4+tableRva]
        put<std::uint8_t>(start + 20, 0x4c); put<std::uint16_t>(start + 21, 0xd001); // add rax,r10
        put<std::uint16_t>(start + 23, 0xe0ff); // jmp rax
        put<std::uint8_t>(start + 32, 0xb8); put<std::uint32_t>(start + 33, 1);
        put<std::uint8_t>(start + 37, 0xc3); put<std::uint8_t>(start + 38, 0xc3);
        put<std::uint32_t>(table, start + 32);
        put<std::uint32_t>(table + 4, different_target ? function(1) : start + 38);
    }
    void two_stage_switch(bool different_classifier = false) {
        const auto start = function(3);
        std::fill_n(bytes.begin() + file(start), 96, std::uint8_t{0x90});
        const auto image_base = [&](unsigned offset, std::uint8_t modrm) {
            put<std::uint8_t>(start + offset, 0x48); put<std::uint8_t>(start + offset + 1, 0x8d);
            put<std::uint8_t>(start + offset + 2, modrm);
            put<std::int32_t>(start + offset + 3, -static_cast<std::int32_t>(start + offset + 7));
        };
        put<std::uint32_t>(start, 0x03fa8366); // cmp dx,3
        put<std::uint16_t>(start + 4, 0x3a77); // ja default+64
        put<std::uint8_t>(start + 6, 0x0f); put<std::uint16_t>(start + 7, 0xcab7); // movzx ecx,dx
        image_base(9, 0x05);
        put<std::uint32_t>(start + 16, 0x0884b60f); // movzx eax,byte[rax+rcx+classifier]
        put<std::uint32_t>(start + 20, start + 92);
        image_base(24, 0x0d);
        put<std::uint8_t>(start + 31, 0x8b); put<std::uint16_t>(start + 32, 0x818c);
        put<std::uint32_t>(start + 34, start + 80); // mov ecx,[rcx+rax*4+targets]
        image_base(38, 0x05);
        put<std::uint8_t>(start + 45, 0x48); put<std::uint16_t>(start + 46, 0xc803); // add rcx,rax
        put<std::uint16_t>(start + 48, 0xe1ff); // jmp rcx
        put<std::uint8_t>(start + 64, 0xc3);
        put<std::uint8_t>(start + 65, 0xb8); put<std::uint32_t>(start + 66, 1);
        put<std::uint8_t>(start + 70, 0xc3);
        put<std::uint32_t>(start + 80, start + 64);
        put<std::uint32_t>(start + 84, start + 65);
        put<std::uint32_t>(start + 88, start + 64);
        put<std::uint32_t>(start + 92, different_classifier ? 0x00020200 : 0x00020100);
        RUNTIME_FUNCTION part{}; part.BeginAddress = start; part.EndAddress = start + 96; part.UnwindData = 0x7018;
        put(0x6000 + 3 * sizeof(part), part);
    }
    void layout_lookup(unsigned variant) {
        const auto start = function(1);
        std::fill_n(bytes.begin() + file(start), 48, std::uint8_t{0x90});
        const auto emit = [&](unsigned offset, std::initializer_list<std::uint8_t> code) {
            std::copy(code.begin(), code.end(), bytes.begin() + file(start + offset));
        };
        if (variant >= 7) {
            const auto table = 0x3f00 + shift;
            std::fill_n(bytes.begin() + file(start), 64, std::uint8_t{0x90});
            RUNTIME_FUNCTION range{start, start + 64, 0x7008};
            put(0x6000 + sizeof(RUNTIME_FUNCTION), range);
            emit(0, {0x41, 0x56, 0x4c, 0x8b, 0xf1});
            emit(5, {0x8b, 0x01, 0x8b, 0x41, 4, 0x8b, 0x41, 8});
            if (variant != 10) emit(13, {0x89, 0xd2}); // explicit zeroextension of the actual RDX index
            emit(15, {0x83, 0xfa, 1, static_cast<std::uint8_t>(variant == 9 ? 0x76 : 0x77), 20});
            emit(20, {0x4c, 0x8d, 0x15});
            put<std::int32_t>(start + 23, -static_cast<std::int32_t>(start + 27)); // lea r10,[rip+imagebase]
            emit(27, {0x41, 0x8b, 0x84, 0x92});
            put<std::uint32_t>(start + 31, table); // mov eax,[r10+rdx*4+table]
            emit(35, {0x4c, 0x01, 0xd0, 0xff, static_cast<std::uint8_t>(variant == 8 ? 0xe1 : 0xe0)});
            emit(40, {0x41, 0x5e, 0xc3, 0x41, 0x8b, 0x46, 16, 0x41, 0x5e, 0xc3});
            put<std::uint32_t>(table, start + 43);
            put<std::uint32_t>(table + 4, start + 40);
        } else if (variant == 0 || variant == 6) {
            emit(0, {0x41, 0x56, 0x4c, 0x8b, 0xf1}); // push r14. mov r14,rcx
            emit(5, {0x8b, 0x01, 0x8b, 0x41, 4, 0x8b, 0x41, 8});
            emit(13, {0x85, 0xd2, 0x75, 3, 0x41, 0x5e, 0xc3}); // jne success. pop r14. ret
            emit(20, {0x41, 0x8b, 0x46, 16, 0x41, 0x5e, 0xc3}); // success reads saved original arg0
        } else if (variant <= 2) {
            emit(0, {0x41, 0x56, 0x85, 0xd2, 0x74, 5}); // push r14. test edx,edx. jz second
            emit(6, {0x4c, 0x8b, 0xf1, 0xeb, 3}); // mov r14,rcx. jmp join
            emit(11, {0x4c, 0x8b, static_cast<std::uint8_t>(variant == 1 ? 0xf1 : 0xf2)}); // second uses rcx or rdx
            emit(14, {0x41, 0x8b, 0x06, 0x41, 0x8b, 0x46, 4, 0x41, 0x8b, 0x46, 8,
                0x41, 0x8b, 0x46, 16, 0x41, 0x5e, 0xc3});
        } else {
            emit(0, {0x41, 0x56, 0x4c, 0x8b, 0xf1}); // push r14. mov r14,rcx
            emit(5, {0x41, 0x8b, 0x06, 0x41, 0x8b, 0x46, 4, 0x41, 0x8b, 0x46, 8,
                0x41, 0x8b, 0x46, 16});
            if (variant == 4) emit(20, {0x49, 0x83, 0xc6, 8}); // changed pointer on loop backedge
            emit(24, {0x85, 0xd2, 0x75, 0xe9, 0x41, 0x5e, 0xc3}); // jne loop+5. pop r14. ret
            if (variant == 5) emit(24, {0xff, 0xe0}); // unknown indirect target never proves an argument alias
        }
        if (variant == 6) {
            RUNTIME_FUNCTION first{start, start + 20, 0x7008};
            RUNTIME_FUNCTION continuation{start + 20, start + 48, 0x7020};
            put(0x6000 + sizeof(RUNTIME_FUNCTION), first);
            put(0x6000 + 4 * sizeof(RUNTIME_FUNCTION), continuation);
            put<std::uint8_t>(0x7020, 1 | (UNW_FLAG_CHAININFO << 3));
            put(0x7024, first);
            raw(0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
                offsetof(IMAGE_OPTIONAL_HEADER64, DataDirectory) + IMAGE_DIRECTORY_ENTRY_EXCEPTION * sizeof(IMAGE_DATA_DIRECTORY),
                IMAGE_DATA_DIRECTORY{0x6000, 5 * sizeof(RUNTIME_FUNCTION)});
        }
    }
    void type(unsigned index, const std::string& name, unsigned size, unsigned primitive) {
        const auto rva = 0x3000 + shift + index * 0x90;
        const auto text_rva = 0x3600 + shift + index * 0x40;
        span(rva, text_rva, name);
        span(rva + 0x10, text_rva, name);
        span(rva + 0x20, text_rva, name);
        put<std::uint32_t>(rva + 0x40, size);
        put<std::uint16_t>(rva + 0x44, 4);
        put<std::uint16_t>(rva + 0x46, 4);
        put<std::uint8_t>(rva + 0x4c, static_cast<std::uint8_t>(primitive));
        put<std::uint32_t>(rva + 0x50, 1000 + index);
        pointer(0x3300 + shift + index * 8, rva);
    }
    explicit FixtureImage(bool target = false) {
        shift = target ? 0x200 : 0;
        code_shift = target ? 0x200 : 0;
        global = target ? 0x5020 : 0x5000;
        IMAGE_DOS_HEADER dos{}; dos.e_magic = IMAGE_DOS_SIGNATURE; dos.e_lfanew = 0x80;
        raw(0, dos);
        IMAGE_NT_HEADERS64 nt{};
        nt.Signature = IMAGE_NT_SIGNATURE; nt.FileHeader.Machine = IMAGE_FILE_MACHINE_AMD64;
        nt.FileHeader.NumberOfSections = 5; nt.FileHeader.TimeDateStamp = target ? 200 : 100;
        nt.FileHeader.SizeOfOptionalHeader = sizeof(IMAGE_OPTIONAL_HEADER64);
        nt.OptionalHeader.Magic = IMAGE_NT_OPTIONAL_HDR64_MAGIC;
        nt.OptionalHeader.ImageBase = base; nt.OptionalHeader.SizeOfImage = 0x8000;
        nt.OptionalHeader.SizeOfHeaders = 0x400; nt.OptionalHeader.SectionAlignment = 0x1000;
        nt.OptionalHeader.FileAlignment = 0x200;
        nt.OptionalHeader.NumberOfRvaAndSizes = IMAGE_NUMBEROF_DIRECTORY_ENTRIES;
        nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION] = {0x6000, 4 * sizeof(RUNTIME_FUNCTION)};
        raw(0x80, nt);
        struct Row { const char* name; unsigned rva, raw, length, flags; };
        const std::array<Row, 5> sections{{
            {".text", 0x1000, 0x400, 0x1000, IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ},
            {".rdata", 0x3000, 0x1400, 0x2000, IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ},
            {".data", 0x5000, 0x3400, 0x400, IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE},
            {".pdata", 0x6000, 0x3800, 0x400, IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ},
            {".xdata", 0x7000, 0x3c00, 0x400, IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ}}};
        for (unsigned i = 0; i < sections.size(); ++i) {
            IMAGE_SECTION_HEADER part{}; const auto& row = sections[i];
            std::memcpy(part.Name, row.name, std::strlen(row.name));
            part.VirtualAddress = row.rva; part.PointerToRawData = row.raw;
            part.Misc.VirtualSize = part.SizeOfRawData = row.length; part.Characteristics = row.flags;
            raw(0x80 + sizeof(nt) + i * sizeof(part), part);
        }
        for (unsigned i = 0; i != 4; ++i) {
            code(i, 100 + i, i == 0, 1, 8, i != 3);
            RUNTIME_FUNCTION part{}; part.BeginAddress = function(i); part.EndAddress = function(i) + 48;
            part.UnwindData = 0x7000 + i * 8;
            put(0x6000 + i * sizeof(part), part);
            put<std::uint8_t>(part.UnwindData, 1);
        }
        type(0, "BlobString", 16, 25); type(1, "uint32", 4, 6); type(2, "keen::CustomState", 16, 18);
        pointer(0x3340 + shift, 0x3300 + shift); put<std::uint64_t>(0x3348 + shift, 3);
        pointer(0x3120 + shift + 0x58, 0x3a00 + shift);
        put<std::uint32_t>(0x3120 + shift + 0x48, 1);
        span(0x3a00 + shift, 0x36e0 + shift, "value");
        pointer(0x3a10 + shift, 0x3090 + shift); put<std::uint64_t>(0x3a18 + shift, 8);
        for (unsigned i = 0; i != 3; ++i) pointer(0x3380 + shift + i * 8, function(i));
        span(0x3800 + shift, 0x3700 + shift, "settings_update");
        pointer(0x3810 + shift, function(2));
        pointer(0x3858 + shift, 0x3900 + shift); put<std::uint64_t>(0x3860 + shift, 1);
        span(0x3900 + shift, 0x3740 + shift, "ecs::CustomState");
        pointer(0x3898 + shift, 0x3920 + shift); put<std::uint64_t>(0x38a0 + shift, 1);
        span(0x3920 + shift, 0x3740 + shift, "ecs::CustomState");
    }
};

struct Fixture {
    fs::path root = fs::temp_directory_path() / ("profile-rebase-test-" + std::to_string(GetCurrentProcessId()) + "-" + std::to_string(GetTickCount64()));
    Image source, target;
    json baseline{{"callbackRva", 0x1000}, {"globalRva", 0x5000}, {"ownerVtableRva", 0x3380},
        {"metadataRva", 0x3120}, {"registrySlotRva", 0x3310}, {"descriptorRva", 0x3800},
        {"ownerLayout", {{"rootObjectRva", 0x5000}, {"rootVtableRva", 0x3380}, {"clientPointerOffset", 8}}},
        {"enabled", true}, {"existingContract", "preserve"}};
    Fixture() {
        check(!fs::exists(root), "unique fixture root");
        fs::create_directories(root);
        source = image(FixtureImage{}, "source");
        target = image(FixtureImage{true}, "target");
    }
    ~Fixture() { std::error_code ignored; fs::remove_all(root, ignored); }
    Image image(const FixtureImage& input, const char* name) {
        const auto directory = root / name;
        fs::create_directories(directory);
        const auto path = directory / "enshrouded.exe";
        std::ofstream stream(path, std::ios::binary | std::ios::trunc);
        stream.write(reinterpret_cast<const char*>(input.bytes.data()), input.bytes.size()); stream.close();
        std::ofstream(directory / "enshrouded.kfc", std::ios::binary) << "KFC4";
        return ReadImage(path);
    }
};

void independent_code_data_and_layout_addresses() {
    Fixture fixture;
    json result, evidence;
    check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence), "complete independent rebase must succeed");
    check(result["callbackRva"] == 0x1200 && result["globalRva"] == 0x5020, "original code and RIP target must be independently relocated");
    check(result["ownerVtableRva"] == 0x3580, "vtable requires three corresponding original methods");
    check(result["metadataRva"] == 0x3320 && result["registrySlotRva"] == 0x3510, "reflection metadata and registry slots must be freshly discovered");
    check(result["descriptorRva"] == 0x3a00, "system descriptor follows original full name and dependency contract");
    check(result["ownerLayout"]["clientPointerOffset"] == 8 && result["enabled"] == true && result["existingContract"] == "preserve", "baseline contract is complete");
    check(evidence["targetProfileRead"] == false, "rebaser accepts no target profile oracle");
}

void unrelated_code_changes_are_not_profile_failures() {
    Fixture fixture;
    FixtureImage changed(true); changed.code(3, 999, false, 1, 8, false);
    fixture.target = fixture.image(changed, "target");
    json result, evidence;
    check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence), "unrelated changed function does not invalidate independently matched contract");
}

void changed_direct_callee_is_not_masked_away() {
    Fixture fixture;
    FixtureImage changed(true); changed.code(0, 100, true, 2);
    fixture.target = fixture.image(changed, "target");
    json result, evidence;
    check(!RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence), "same instructions with a different direct callee must reject");
    check(evidence.dump().find("original-direct-call-target-changed") != std::string::npos, "direct target conflict is explicit");
}

void ambiguous_code_and_changed_reflection_reject() {
    for (const bool layout : {false, true}) {
        Fixture fixture;
        FixtureImage changed(true);
        if (layout) changed.put<std::uint64_t>(0x3a18 + changed.shift, 12);
        else changed.code(3, 101);
        fixture.target = fixture.image(changed, "target");
        json result, evidence;
        check(!RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence), "ambiguous function or changed original field must reject");
    }
}

void private_owner_field_change_is_not_hidden_by_another_literal() {
    Fixture fixture;
    FixtureImage before, after(true);
    before.code(2, 102, false, 1, 32, false);
    after.code(2, 102, false, 1, 32, false);
    before.code(3, 103, false, 1, 32, true);
    after.code(3, 103, false, 1, 40, true);
    fixture.source = fixture.image(before, "source");
    fixture.target = fixture.image(after, "target");
    fixture.baseline["ownerLayout"]["clientPointerOffset"] = 32;
    json result, evidence;
    check(!RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence),
        "changed root-owned field must reject even if the old value survives in another object");
    check(evidence.dump().find("clientPointerOffset") != std::string::npos,
        "private-field failure identifies its actual owner-bound profile field");
}

void owner_bound_layout_pairs_survive_consumer_contract_validation() {
    for (const bool registry_owner : {false, true}) {
        std::map<std::uint32_t, Procedure> source_functions, target_functions;
        Procedure source_owner{.root = 0x1000, .ranges = {{0x1000, 0x1010}}, .signature = "owner"};
        source_owner.call_arguments.push_back(CallArguments{.instruction = 0x1004, .target = 0x1100});
        Procedure target_owner{.root = 0x2000, .ranges = {{0x2000, 0x2010}}, .signature = "owner"};
        target_owner.call_arguments.push_back(CallArguments{.instruction = 0x2004, .target = 0x2100});
        if (registry_owner) {
            source_owner.references.push_back(Reference{.target = 0x3000});
            target_owner.references.push_back(Reference{.target = 0x4000});
        }
        const auto value = registry_owner ? 24 : 320;
        Procedure source_consumer{.root = 0x1100, .ranges = {{0x1100, 0x1110}}, .signature = "consumer"};
        source_consumer.layout_uses.push_back(LayoutUse{0x1105, 0, value, -1, "original-memory-displacement", 8, true, false});
        Procedure target_consumer{.root = 0x2100, .ranges = {{0x2100, 0x2110}}, .signature = "consumer"};
        target_consumer.layout_uses.push_back(LayoutUse{0x2105, 0, value, -1, "original-memory-displacement", 8, true, false});
        source_functions.emplace(source_owner.root, source_owner);
        source_functions.emplace(source_consumer.root, source_consumer);
        target_functions.emplace(target_owner.root, target_owner);
        target_functions.emplace(target_consumer.root, target_consumer);
        const std::map<std::string, ReflectedType> no_types;
        auto verified_pairs = std::map<std::uint32_t, std::uint32_t>{{0x1000, 0x2000}};
        ProfileRebaseAnalysis analysis{source_functions, target_functions, verified_pairs, no_types, no_types, {}, {}, {}};
        json baseline{{"ecsWriteLayout", registry_owner ? json{{"threadRegistryEntriesRva", 0x3000}} :
            json{{"changeTrackerRva", 0x1000}}}};
        const auto path = registry_owner ? "/ecsWriteLayout/threadRegistryEntryStride" :
            "/ecsWriteLayout/changeTrackerQueueBOffset";
        json evidence;
        check(!ExactLayoutConsumerPair(analysis, 0x1100, 0x2100),
            "owner-bound leaf is not independently matched before layout proof");
        check(ProveProfileLayouts(baseline, analysis,
                {{path, value}}, evidence, verified_pairs),
            "complete owner callsite and matching layout operand establish the field");
        check(verified_pairs.at(0x1100) == 0x2100 && ExactLayoutConsumerPair(analysis, 0x1100, 0x2100),
            "the exported owner-bound pair passes the final consumer-contract check");
        check(evidence["fields"].size() == 1 && evidence["unresolved"].empty(),
            "owner-bound layout proof remains resolved after contract verification");
        target_functions.at(0x2100).signature = "changed-consumer";
        verified_pairs = {{0x1000, 0x2000}};
        check(!ProveProfileLayouts(baseline, analysis, {{path, value}}, evidence, verified_pairs),
            "a changed owner-bound helper must reject despite the preserved layout literal");
        check(!verified_pairs.contains(0x1100), "a changed helper is not exported as a verified counterpart");
    }
}

void layout_aliases_follow_return_join_and_loop_edges() {
    for (unsigned variant = 0; variant != 11; ++variant) {
        Fixture fixture;
        FixtureImage before, after(true);
        before.layout_lookup(variant); after.layout_lookup(variant);
        // An unrelated changed use of the same offset prevents global-use-set
        // fallback from hiding whether the callback's own argument was proven.
        before.code(3, 103, false, 1, 16, false);
        after.code(3, 999, false, 1, 16, false);
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        fixture.baseline["resourceLayout"] = {{"resourceServiceIdCallbackRva", before.function(0)},
            {"map", {{"bucketRel32Offset", 0}, {"bucketCountOffset", 4},
                {"keysRel32Offset", 8}, {"valuesRel32Offset", 16}}}};
        json result, evidence;
        const bool expected = variant == 0 || variant == 1 || variant == 3 || variant == 6 || variant == 7;
        const bool accepted = RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence);
        if (accepted != expected) std::cerr << "layout CFG fixture " << variant << '\n' << evidence.dump(2) << '\n';
        check(accepted == expected,
            "layout aliases require agreeing reachable paths, including alternative returns and loop backedges");
        if (expected) {
            bool owner_bound{};
            for (const auto& field : evidence.at("layoutProof").at("fields"))
                if (field.at("path") == "/resourceLayout/map/valuesRel32Offset") {
                    check(field.at("method") == "owner-bound-original-instruction-operands",
                        "return/join/loop fixture uses the registered callback owner instead of a global literal");
                    for (const auto& proof : field.at("witnesses"))
                        owner_bound |= proof.at("kind") == "original-argument-relative-field" && proof.at("sourceArgument") == 0;
                }
            check(owner_bound, "the successful header read retains the proven original argument alias");
        } else {
            bool unresolved{};
            for (const auto& field : evidence.at("layoutProof").at("unresolved"))
                unresolved |= field.at("path") == "/resourceLayout/map/valuesRel32Offset";
            check(unresolved, "conflicting joins, changing loop pointers and unknown branches retain unresolved layout evidence");
        }
    }
}

void indexed_queue_addresses_require_the_original_base_and_every_path() {
    enum class Variant {
        original, agreeing_join, wrong_bias, changed_bias, input_argument_index,
        input_argument_index_join, wrong_base, conflicting_base_join, conflicting_bias_join, changed_callee
    };
    const std::array cases{Variant::original, Variant::agreeing_join, Variant::wrong_bias,
        Variant::changed_bias, Variant::input_argument_index, Variant::input_argument_index_join, Variant::wrong_base,
        Variant::conflicting_base_join, Variant::conflicting_bias_join, Variant::changed_callee};
    for (const auto variant : cases) {
        Fixture fixture;
        FixtureImage before, after(true);
        const auto write_queue_helper = [&](FixtureImage& image, bool changed_image) {
            const auto start = image.function(1);
            std::fill_n(image.bytes.begin() + FixtureImage::file(start), 48, std::uint8_t{0x90});
            unsigned cursor{};
            const auto emit = [&](std::initializer_list<std::uint8_t> code) {
                check(cursor + code.size() <= 48, "indexed queue fixture fits its compiler range");
                std::copy(code.begin(), code.end(), image.bytes.begin() + FixtureImage::file(start + cursor));
                cursor += static_cast<unsigned>(code.size());
            };
            if (variant == Variant::input_argument_index_join) {
                emit({0x45, 0x85, 0xc9, 0x74, 5}); // test r9d,r9d. je second input argument
                emit({0x48, 0x8b, 0xc1, 0xeb, 3}); // mov rax,rcx. jmp index join
                emit({0x49, 0x8b, 0xc0}); // mov rax,r8. The phi still originates in input arguments.
            } else if (variant == Variant::input_argument_index)
                emit({0x49, 0x8b, 0xc0}); // mov rax,r8. A different input argument is not a local index.
            else
                emit({0x8b, 0x44, 0x24, 0x28}); // mov eax,[rsp+28h]. A concrete local/stack index origin.
            if (variant == Variant::wrong_base)
                emit({0x49, 0x8b, 0xd0}); // mov rdx,r8. The queue no longer belongs to input arg1.
            if (variant == Variant::conflicting_bias_join) {
                emit({0x45, 0x85, 0xc9, 0x74, 6}); // test r9d,r9d. je other bias
                emit({0x48, 0x83, 0xc0, 8, 0xeb, 4}); // add rax,8. jmp join
                emit({0x48, 0x83, 0xc0, 7}); // other incoming path has a different affine bias
            } else {
                const bool wrong_bias = variant == Variant::wrong_bias ||
                    (variant == Variant::changed_bias && changed_image);
                emit({0x48, 0x83, 0xc0, static_cast<std::uint8_t>(wrong_bias ? 7 : 8)}); // add rax,bias
            }
            emit({0x48, 0x8d, 0x04, 0x80}); // lea rax,[rax+rax*4]. Index*5 and bias*5.
            if (variant == Variant::agreeing_join || variant == Variant::conflicting_base_join) {
                emit({0x45, 0x85, 0xc9, 0x74, 5}); // test r9d,r9d. je second base
                emit({0x4c, 0x8b, 0xd2, 0xeb, 3}); // mov r10,rdx. jmp join
                emit({static_cast<std::uint8_t>(variant == Variant::agreeing_join ? 0x4c : 0x4d),
                    0x8b, static_cast<std::uint8_t>(variant == Variant::agreeing_join ? 0xd2 : 0xd0)});
                emit({0x49, 0x8d, 0x0c, 0xc2}); // lea rcx,[r10+rax*8]
            } else {
                emit({0x48, 0x8d, 0x0c, 0xc2}); // lea rcx,[rdx+rax*8]. No literal320 is encoded.
            }
            const auto append = image.function(variant == Variant::changed_callee && changed_image ? 3 : 2);
            emit({0xe8});
            image.put<std::int32_t>(start + cursor, static_cast<std::int32_t>(append - (start + cursor + 4)));
            cursor += 4;
            emit({0xc3});
        };
        write_queue_helper(before, false);
        write_queue_helper(after, true);
        // An unrelated changed procedure retains literal320. This makes the
        // global-use-set fallback fail, so only the tracker-owned address can
        // establish which argument owns the derived queue group.
        before.code(3, 103, false, 1, 8, false);
        after.code(3, 999, false, 1, 8, false);
        for (auto* image : {&before, &after}) {
            image->put<std::uint8_t>(image->function(3) + 24, 0xba); // mov edx,320
            image->put<std::uint32_t>(image->function(3) + 25, 320);
        }
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        fixture.baseline["ecsWriteLayout"] = {{"changeTrackerRva", before.function(1)},
            {"changeTrackerQueueBOffset", 320}};
        json result, evidence;
        const bool expected = variant == Variant::original || variant == Variant::agreeing_join;
        const bool accepted = RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence);
        if (accepted != expected)
            std::cerr << "indexed queue fixture " << static_cast<unsigned>(variant) << '\n' << evidence.dump(2) << '\n';
        check(accepted == expected,
            "indexed queue bias requires its original argument, agreeing CFG paths and original append callee");
        if (expected) {
            bool indexed_owner{};
            for (const auto& field : evidence.at("layoutProof").at("fields")) {
                if (field.at("path") != "/ecsWriteLayout/changeTrackerQueueBOffset") continue;
                check(field.at("method") == "owner-bound-original-instruction-operands",
                    "indexed queue fixture is proven by its tracker owner rather than unrelated literal320");
                for (const auto& proof : field.at("witnesses"))
                    indexed_owner |= proof.at("kind") == "original-indexed-argument-relative-field" &&
                        proof.at("sourceArgument") == 1 && proof.at("value") == 320 && proof.at("indexStride") == 40;
            }
            check(indexed_owner, "decoded ADD/LEA instructions establish queue bias320 and index stride40");
        }
    }
}

void registry_layouts_require_one_common_resource_typed_factory() {
    enum class Variant {
        shared_factory, changed_resource_hash, changed_resource_layout, ambiguous_factories,
        factory_reached_by_one_registration, wrong_factory_hash, changed_helper,
        late_factory_binding_conflict, single_registration_legacy, single_registration_too_deep
    };
    const std::array cases{Variant::shared_factory, Variant::changed_resource_hash,
        Variant::changed_resource_layout, Variant::ambiguous_factories,
        Variant::factory_reached_by_one_registration, Variant::wrong_factory_hash,
        Variant::changed_helper, Variant::late_factory_binding_conflict,
        Variant::single_registration_legacy, Variant::single_registration_too_deep};
    for (const auto variant : cases) {
        std::map<std::uint32_t, Procedure> source_functions, target_functions;
        const auto add_procedure = [&](std::uint32_t root, const char* signature) {
            source_functions.emplace(root, Procedure{.root = root,
                .ranges = {{root, root + 48}}, .signature = signature});
            target_functions.emplace(root + 0x1000, Procedure{.root = root + 0x1000,
                .ranges = {{root + 0x1000, root + 0x1030}}, .signature = signature});
        };
        const auto connect = [&](std::uint32_t owner, std::uint32_t child, unsigned offset = 4) {
            source_functions.at(owner).call_arguments.push_back(CallArguments{
                .instruction = owner + offset, .target = child});
            target_functions.at(owner + 0x1000).call_arguments.push_back(CallArguments{
                .instruction = owner + 0x1000 + offset, .target = child + 0x1000});
        };
        const auto factory = variant == Variant::late_factory_binding_conflict ? 0x0900u : 0x1200u;
        add_procedure(0x1000, "client-registry-registration");
        add_procedure(0x1100, "authority-registry-registration");
        add_procedure(factory, "resource-typed-factory");
        add_procedure(0x1300, "registry-constructor");
        add_procedure(0x1400, "registry-map-reader");
        add_procedure(0x1600, "unrelated-same-number-reader");
        target_functions.at(0x2600).signature = "changed-unrelated-same-number-reader";
        connect(0x1000, factory);
        connect(0x1100, factory);
        connect(factory, 0x1300);
        connect(0x1300, 0x1400);

        const std::string registry_name = "example::ArtifactRegistry";
        const std::string resource_name = "keen::ArtifactRegistryResource";
        constexpr std::uint32_t qualified_hash = 0x1234abcd;
        source_functions.at(factory).constants.insert(qualified_hash);
        target_functions.at(factory + 0x1000).constants.insert(qualified_hash);
        ReflectedType source_type{.metadata = 0x3000, .slot = 0x3100, .hash = qualified_hash,
            .size = 128, .name = resource_name,
            .layout = {{"size", 128}, {"fields", {{"payload", {{"offset", 12}}}}}}};
        auto target_type = source_type;
        target_type.metadata = 0x5000;
        target_type.slot = 0x5100;
        std::map<std::string, ReflectedType> source_types{{resource_name, source_type}};
        std::map<std::string, ReflectedType> target_types{{resource_name, target_type}};
        if (variant == Variant::changed_resource_hash) ++target_types.at(resource_name).hash;
        if (variant == Variant::changed_resource_layout)
            target_types.at(resource_name).layout["fields"]["payload"]["offset"] = 20;
        if (variant == Variant::ambiguous_factories) {
            add_procedure(0x1500, "another-resource-typed-factory");
            source_functions.at(0x1500).constants.insert(qualified_hash);
            target_functions.at(0x2500).constants.insert(qualified_hash);
            connect(0x1000, 0x1500, 12);
            connect(0x1100, 0x1500, 12);
            connect(0x1500, 0x1300);
        }
        if (variant == Variant::factory_reached_by_one_registration) {
            source_functions.at(0x1100).call_arguments.clear();
            target_functions.at(0x2100).call_arguments.clear();
        }
        if (variant == Variant::wrong_factory_hash) {
            source_functions.at(factory).constants = {qualified_hash + 1};
            target_functions.at(factory + 0x1000).constants = {qualified_hash + 1};
        }
        if (variant == Variant::changed_helper)
            target_functions.at(0x2400).signature = "changed-registry-map-reader";
        if (variant == Variant::late_factory_binding_conflict) {
            // The independently paired factory sorts before both registrations,
            // so its children can already be bound when the second original
            // owner call reveals an incompatible target-factory identity.
            add_procedure(0x0a00, "resource-typed-factory");
            source_functions.at(0x0a00).constants.insert(qualified_hash);
            target_functions.at(0x1a00).constants.insert(qualified_hash);
            connect(0x0a00, 0x1300);
            target_functions.at(0x2100).call_arguments.at(0).target = 0x1a00;
        }

        const bool legacy = variant == Variant::single_registration_legacy ||
            variant == Variant::single_registration_too_deep;
        if (legacy) {
            source_types.clear();
            target_types.clear();
            if (variant == Variant::single_registration_legacy) {
                source_functions.at(0x1000).call_arguments.clear();
                target_functions.at(0x2000).call_arguments.clear();
                connect(0x1000, 0x1300); // Old route reaches map reader in exactly two calls.
            }
        }
        const json baseline{{"resourceLayout", {
            {"itemRegistryServiceOffset", 24}, {"itemRegistryHandleOffset", 36},
            {"itemRegistryResourcePointerOffset", 48},
            {"itemIdMap", {{"registry", registry_name}, {"mapOffset", 64},
                {"bitsPointerOffset", 72}, {"capacityOffset", 80}, {"keysPointerOffset", 88},
                {"valuesPointerOffset", 96}, {"countOffset", 104}}}}}};
        const std::vector<std::pair<std::string, std::uint64_t>> fields{
            {"/resourceLayout/itemRegistryServiceOffset", 24},
            {"/resourceLayout/itemRegistryHandleOffset", 36},
            {"/resourceLayout/itemRegistryResourcePointerOffset", 48},
            {"/resourceLayout/itemIdMap/mapOffset", 64},
            {"/resourceLayout/itemIdMap/bitsPointerOffset", 72},
            {"/resourceLayout/itemIdMap/capacityOffset", 80},
            {"/resourceLayout/itemIdMap/keysPointerOffset", 88},
            {"/resourceLayout/itemIdMap/valuesPointerOffset", 96},
            {"/resourceLayout/itemIdMap/countOffset", 104}};
        for (std::size_t i = 0; i < fields.size(); ++i) {
            const auto owner = i < 3 ? 0x1300u : 0x1400u;
            const auto offset = static_cast<unsigned>((i < 3 ? i : i - 3) * 4);
            const auto value = static_cast<std::int64_t>(fields[i].second);
            for (const auto shift : {0u, 0x1000u}) {
                auto& functions = shift ? target_functions : source_functions;
                functions.at(owner + shift).layout_uses.push_back(LayoutUse{
                    owner + shift + offset, 1, value, -1, "original-memory-displacement", 8, true, false});
                // An unmapped equal-valued consumer blocks the global-use-set
                // fallback for every field, including the old single-root case.
                functions.at(0x1600 + shift).layout_uses.push_back(LayoutUse{
                    0x1600 + shift + static_cast<unsigned>(i * 4), 1, value, -1,
                    "original-memory-displacement", 8, true, false});
            }
        }
        auto verified_pairs = std::map<std::uint32_t, std::uint32_t>{{0x1000, 0x2000}, {0x1100, 0x2100}};
        if (variant == Variant::late_factory_binding_conflict)
            verified_pairs.emplace(factory, factory + 0x1000);
        ProfileRebaseAnalysis analysis{source_functions, target_functions, verified_pairs,
            source_types, target_types, {{registry_name, legacy ? std::set<std::uint32_t>{0x1000} :
                std::set<std::uint32_t>{0x1000, 0x1100}}}, {}, {}};
        json evidence;
        const bool expected = variant == Variant::shared_factory || variant == Variant::single_registration_legacy;
        const bool accepted = ProveProfileLayouts(baseline, analysis, fields, evidence, verified_pairs);
        if (accepted != expected)
            std::cerr << "registry factory fixture " << static_cast<unsigned>(variant) << '\n' << evidence.dump(2) << '\n';
        check(accepted == expected,
            "registry layouts require one factory shared by all registrations and the matching resource type");
        if (expected) {
            check(evidence.at("fields").size() == fields.size() && evidence.at("unresolved").empty(),
                "all nine registry fields have independent owner-bound evidence");
            for (const auto& field : evidence.at("fields"))
                check(field.at("method") == "owner-bound-original-instruction-operands" && !field.at("witnesses").empty(),
                    "the registry factory fixture cannot succeed through the equal-number global fallback");
            check(verified_pairs.at(0x1300) == 0x2300 && verified_pairs.at(0x1400) == 0x2400,
                "the original constructor and map helper are bound through their real owner calls");
            check(verified_pairs.contains(0x1200) != legacy,
                "a single registration preserves its two-level legacy route without inventing a typed factory");
        } else {
            check(!evidence.at("unresolved").empty(), "invalid registry roles retain unresolved layout evidence");
            if (variant == Variant::changed_helper)
                check(!verified_pairs.contains(0x1400), "a changed map helper is never exported as a verified counterpart");
            if (variant == Variant::single_registration_too_deep)
                check(evidence.at("unresolved").size() == 6,
                    "legacy single-root traversal is still bounded to two calls rather than silently widened");
            if (variant == Variant::late_factory_binding_conflict)
                check(evidence.at("unresolved").size() == fields.size() && evidence.at("fields").empty(),
                    "a late factory-identity conflict rejects every registry field despite already bound children");
        }
    }
}

void callsite_bound_unwindless_leaf_is_verified_completely() {
    for (const bool changed_leaf : {false, true}) {
        Fixture fixture;
        FixtureImage before, after(true);
        before.remove_unwind_entry(1); after.remove_unwind_entry(1);
        before.pointer(0x3380 + 24, before.function(3));
        after.pointer(0x3380 + after.shift + 24, after.function(3));
        if (changed_leaf) after.code(1, 101, false, 1, 9);
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        json result, evidence;
        const bool accepted = RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence);
        if (accepted == changed_leaf) std::cerr << evidence.dump(2) << '\n';
        check(accepted != changed_leaf,
            "unwindless leaf is bound by the parent callsite and its entire decoded body");
        if (!changed_leaf) check(evidence["verifiedLeafPairs"].get<unsigned>() > 0,
            "accepted unwindless callee has explicit complete-body evidence");
    }
}

void every_unwindless_conditional_path_is_verified() {
    for (const bool changed_branch : {false, true}) {
        Fixture fixture;
        FixtureImage before, after(true);
        for (auto* image : {&before, &after}) {
            image->remove_unwind_entry(1);
            image->pointer(0x3380 + image->shift + 24, image->function(3));
            const auto start = image->function(1);
            image->put<std::uint16_t>(start + 16, 0xc985); // test ecx,ecx
            image->put<std::uint16_t>(start + 18, 0x0874); // jz second return
            image->put<std::uint8_t>(start + 20, 0xb8);
            image->put<std::uint32_t>(start + 21, 1);
            image->put<std::uint8_t>(start + 25, 0xc3);
            image->put<std::uint8_t>(start + 28, 0xb8);
            image->put<std::uint32_t>(start + 29, 2);
            image->put<std::uint8_t>(start + 33, 0xc3);
        }
        if (changed_branch) after.put<std::uint32_t>(after.function(1) + 29, 3);
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        json result, evidence;
        check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence) != changed_branch,
            "a conditional branch beyond the first return is independently checked");
    }
}

void original_image_relative_jump_tables_require_every_target() {
    for (const bool embedded : {false, true}) for (const bool changed_target : {false, true}) {
        Fixture fixture;
        FixtureImage before, after(true);
        before.indexed_switch(embedded);
        after.indexed_switch(embedded, changed_target);
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        json result, evidence;
        check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence) != changed_target,
            "relocated indexed table requires every corresponding original code target");
    }
}

void invocation_byte_guard_tracks_the_proven_original_procedure() {
    for (const bool wrong_source_guard : {false, true}) {
        Fixture fixture;
        fixture.baseline["ecsWriteLayout"] = {{"changeTrackerRva", 0x1000},
            {"changeTrackerBytes", wrong_source_guard ? "b864000000488d0df53f0000" : "b864000000488d0df43f0000"}};
        json result, evidence;
        check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence) != wrong_source_guard,
            "byte guards require exact old bytes and independently proven new code");
        if (!wrong_source_guard)
            check(result["ecsWriteLayout"]["changeTrackerBytes"] == "b864000000488d0d143e0000",
                "relocated RIP bytes are freshly read from the proven target entry");
    }
}

void byte_classifier_and_target_table_are_separate_original_data() {
    for (const bool different_classifier : {false, true}) {
        Fixture fixture;
        FixtureImage before, after(true);
        before.two_stage_switch(); after.two_stage_switch(different_classifier);
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        fixture.baseline["callbackRva"] = before.function(3);
        json result, evidence;
        check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence) != different_classifier,
            "two-stage switch decodes typed original arrays and preserves every classifier value");
    }
}

void callsite_bound_data_requires_its_actual_original_contract() {
    for (unsigned kind = 0; kind < 3; ++kind) for (const bool changed_contract : {false, true}) {
        Fixture fixture;
        FixtureImage before, after(true);
        for (auto* image : {&before, &after}) {
            image->remove_unwind_entry(1);
            image->pointer(0x3380 + image->shift + 24, image->function(3));
            const auto start = image->function(1);
            const auto data = kind == 2 ? image->global + 0x50 : 0x3f00 + image->shift;
            unsigned length{};
            if (kind == 1) {
                image->put<std::uint8_t>(start, 0x48); image->put<std::uint8_t>(start + 1, 0x8d);
                image->put<std::uint8_t>(start + 2, 0x05);
                image->put<std::int32_t>(start + 3, static_cast<std::int32_t>(data - (start + 7)));
                image->text(data, changed_contract && image == &after ? "changed-anchor-value" : "original-anchor-value");
                length = 7;
            } else {
                image->put<std::uint16_t>(start, 0x058b); // mov eax,[rip+data]
                image->put<std::int32_t>(start + 2, static_cast<std::int32_t>(data - (start + 6)));
                length = 6;
                if (kind == 0) image->put<std::uint32_t>(data, changed_contract && image == &after ? 0x12345679 : 0x12345678);
                else {
                    image->put<std::uint16_t>(start + 6, 0x0589); // mov [rip+same-global],eax
                    const auto written = data + (changed_contract && image == &after ? 4 : 0);
                    image->put<std::int32_t>(start + 8, static_cast<std::int32_t>(written - (start + 12)));
                    length = 12;
                }
            }
            image->put<std::uint8_t>(start + length, 0xc3);
        }
        fixture.source = fixture.image(before, "source");
        fixture.target = fixture.image(after, "target");
        json result, evidence;
        check(RebaseProfile(fixture.source, fixture.target, fixture.baseline, result, evidence) != changed_contract,
            "literal, complete string and shared global read/write contracts require their own original evidence");
    }
}
}

int main() try {
    independent_code_data_and_layout_addresses();
    unrelated_code_changes_are_not_profile_failures();
    changed_direct_callee_is_not_masked_away();
    ambiguous_code_and_changed_reflection_reject();
    private_owner_field_change_is_not_hidden_by_another_literal();
    owner_bound_layout_pairs_survive_consumer_contract_validation();
    layout_aliases_follow_return_join_and_loop_edges();
    indexed_queue_addresses_require_the_original_base_and_every_path();
    registry_layouts_require_one_common_resource_typed_factory();
    callsite_bound_unwindless_leaf_is_verified_completely();
    every_unwindless_conditional_path_is_verified();
    original_image_relative_jump_tables_require_every_target();
    invocation_byte_guard_tracks_the_proven_original_procedure();
    byte_classifier_and_target_table_are_separate_original_data();
    callsite_bound_data_requires_its_actual_original_contract();
    std::cout << "native profile rebase: code, references, vtables, reflection, descriptors and rejection checks passed\n";
    return 0;
} catch (const std::exception& error) {
    std::cerr << "native profile rebase failed: " << error.what() << '\n';
    return 1;
}
