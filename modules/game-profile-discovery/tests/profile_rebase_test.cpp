// Original-image fixtures with independent code, data, reflection and system
// registrations. No game build IDs or production addresses appear here.
#include "game-profile-discovery/rebase_profile_contract.h"
#include <windows.h>
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
