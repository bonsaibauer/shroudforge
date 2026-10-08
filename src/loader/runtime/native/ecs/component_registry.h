#pragma once

// The engine owns two layouts for a registration: entity storage and template
// configuration. A missing storage layout is intentional, not a missing type.
// This reader is shared by the provider and the read-only external verifier.
#include <cstdint>
#include <cstring>
#include <functional>
#include <string>
#include <unordered_set>
#include <vector>

namespace ComponentRegistry {
using Reader = std::function<bool(std::uintptr_t, void*, std::size_t)>;
struct Type {
    std::string name;
    std::uint32_t hash{}, size{}, metadata_rva{};
};
struct Callback { std::uint32_t slot_offset{}, function_rva{}; bool in_record{}; };
struct Entry {
    std::uint16_t index{};
    std::string name;
    std::uint32_t hash{}, size{}, flags{}, storage_flags{};
    Type storage, configuration;
    std::vector<Callback> callbacks;
};
struct Snapshot {
    std::uintptr_t owner{}, records{};
    std::vector<Entry> entries;
};

inline std::uint32_t hash_name(const std::string& name) {
    std::uint32_t hash = 0x811c9dc5u;
    for (const unsigned char byte : name) hash = (hash ^ byte) * 0x1000193u;
    return hash;
}

// Layout v1, independently observed in client 1076226 and server 1024233.
// No component names, indices, absolute addresses, or build hashes are input.
inline bool Read(std::uintptr_t manager, std::uintptr_t image, std::size_t image_size,
                 const Reader& read, Snapshot& output, std::string& error) {
    auto fail = [&](const char* reason) { error = reason; return false; };
    auto scalar = [&]<class T>(std::uintptr_t address, T& value) {
        return address && read(address, &value, sizeof(value));
    };
    auto in_image = [&](std::uintptr_t address, std::size_t size) {
        return address >= image && size <= image_size && address - image <= image_size - size;
    };
    auto text = [&](std::uintptr_t address, std::uint64_t size, std::string& result) {
        if (!size || size > 512 || !in_image(address, static_cast<std::size_t>(size))) return false;
        result.resize(static_cast<std::size_t>(size));
        if (!read(address, result.data(), result.size())) return false;
        // Registration spans include the terminator; reflection spans do not.
        if (result.back() == '\0') result.pop_back();
        return !result.empty() && result.find('\0') == std::string::npos;
    };
    auto type = [&](std::uintptr_t address, Type& result) {
        if (!address) return true;
        if (!in_image(address, 0x90)) return false;
        std::uintptr_t name{};
        std::uint64_t length{};
        if (!scalar(address + 0x20, name) || !scalar(address + 0x28, length) ||
            !text(name, length, result.name) || !result.name.starts_with("keen::ecs::") ||
            !scalar(address + 0x40, result.size) || !result.size || result.size > (1u << 24) ||
            !scalar(address + 0x50, result.hash) || result.hash != hash_name(result.name)) return false;
        result.metadata_rva = static_cast<std::uint32_t>(address - image);
        return true;
    };
    std::uint32_t nt_offset{};
    std::uint16_t section_count{}, optional_size{};
    if (!scalar(image + 0x3c, nt_offset) || !in_image(image + nt_offset, 24) ||
        !scalar(image + nt_offset + 6, section_count) || section_count > 96 ||
        !scalar(image + nt_offset + 20, optional_size)) return fail("invalid image section headers");
    std::vector<std::pair<std::uint32_t, std::uint32_t>> code_ranges;
    for (std::size_t i = 0; i < section_count; ++i) {
        const auto header = image + nt_offset + 24 + optional_size + i * 40;
        std::uint32_t size{}, rva{}, flags{};
        if (!in_image(header, 40) || !scalar(header + 8, size) || !scalar(header + 12, rva) ||
            !scalar(header + 36, flags) || !in_image(image + rva, size)) return fail("invalid image section");
        if (flags & 0x20000000u) code_ranges.emplace_back(rva, rva + size);
    }
    auto is_code = [&](std::uintptr_t address) {
        if (!in_image(address, 1)) return false;
        const auto rva = address - image;
        for (const auto& [begin, end] : code_ranges) if (begin <= rva && rva < end) return true;
        return false;
    };
    Snapshot snapshot;
    std::uint64_t count{}, capacity{}, sizes_count{}, types_count{}, callbacks_count{};
    std::uintptr_t sizes{}, types{}, callbacks{};
    if (!scalar(manager, snapshot.owner) || !snapshot.owner ||
        !scalar(snapshot.owner + 8, snapshot.records) ||
        !scalar(snapshot.owner + 16, count) || !count || count > 1024 ||
        !scalar(snapshot.owner + 24, capacity) || capacity < count || capacity > 4096 ||
        !scalar(snapshot.owner + 232, sizes) || !scalar(snapshot.owner + 240, sizes_count) ||
        !scalar(snapshot.owner + 256, types) || !scalar(snapshot.owner + 264, types_count) ||
        !scalar(snapshot.owner + 280, callbacks) || !scalar(snapshot.owner + 288, callbacks_count) ||
        sizes_count != count || types_count != count || callbacks_count != count)
        return fail("component registry owner/array headers do not match layout v1");
    std::vector<std::uint8_t> records(count * 256);
    std::vector<std::uintptr_t> storage_types(count);
    std::vector<std::uint16_t> storage_sizes(count);
    std::vector<std::uint64_t> callback_slots(count * 5);
    if (!read(snapshot.records, records.data(), records.size()) ||
        !read(types, storage_types.data(), count * 8) ||
        !read(sizes, storage_sizes.data(), count * 2) ||
        !read(callbacks, callback_slots.data(), count * 40))
        return fail("component registry arrays unreadable");
    std::unordered_set<std::string> names, runtime_names;
    std::unordered_set<std::uint32_t> hashes;
    for (std::size_t i = 0; i < count; ++i) {
        const auto* data = records.data() + i * 256;
        auto u64 = [&](std::size_t offset) { std::uint64_t value{}; std::memcpy(&value, data + offset, 8); return value; };
        auto u32 = [&](std::size_t offset) { std::uint32_t value{}; std::memcpy(&value, data + offset, 4); return value; };
        Entry entry;
        entry.index = static_cast<std::uint16_t>(i);
        entry.hash = u32(32);
        entry.size = u32(56) & 0xffffu;
        entry.storage_flags = u32(56) >> 16;
        entry.flags = u32(60);
        if (!text(u64(16), u64(24), entry.name) || !entry.name.starts_with("keen::ecs::") ||
            entry.hash != hash_name(entry.name) || !names.insert(entry.name).second ||
            !hashes.insert(entry.hash).second || !type(u64(40), entry.storage) ||
            !type(u64(48), entry.configuration) ||
            (entry.storage.name.empty() && entry.configuration.name.empty()))
            return fail("invalid, ambiguous, or incomplete component registration metadata");
        const auto& identity = entry.configuration.name.empty() ? entry.storage : entry.configuration;
        if (identity.name != entry.name || identity.hash != entry.hash ||
            entry.size != entry.storage.size || storage_sizes[i] != entry.size ||
            storage_types[i] != u64(40) ||
            (!entry.storage.name.empty() && !runtime_names.insert(entry.storage.name).second))
            return fail("component registration disagrees with reflected identity or parallel storage arrays");
        // These pointers establish ownership of code, not its ABI. Never call them here.
        for (std::uint32_t slot = 0; slot < 4; ++slot) {
            const auto pointer = callback_slots[i * 5 + slot];
            if (!pointer) continue;
            if (!is_code(pointer)) return fail("callback points outside executable code");
            entry.callbacks.push_back({slot * 8, static_cast<std::uint32_t>(pointer - image), false});
        }
        for (std::uint32_t offset = 88; offset < 256; offset += 8)
            if (is_code(u64(offset)))
                entry.callbacks.push_back({offset, static_cast<std::uint32_t>(u64(offset) - image), true});
        snapshot.entries.push_back(std::move(entry));
    }
    std::uintptr_t owner_again{}, records_again{}, types_again{}, sizes_again{}, callbacks_again{};
    std::uint64_t count_again{};
    std::vector<std::uint8_t> readback(records.size());
    if (!scalar(manager, owner_again) || owner_again != snapshot.owner ||
        !scalar(snapshot.owner + 8, records_again) || records_again != snapshot.records ||
        !scalar(snapshot.owner + 16, count_again) || count_again != count ||
        !scalar(snapshot.owner + 232, sizes_again) || sizes_again != sizes ||
        !scalar(snapshot.owner + 256, types_again) || types_again != types ||
        !scalar(snapshot.owner + 280, callbacks_again) || callbacks_again != callbacks ||
        !read(snapshot.records, readback.data(), readback.size()))
        return fail("component registry changed during capture");
    // Runtime bookkeeping later in the record may change; the identity/layout prefix must not.
    for (std::size_t i = 0; i < count; ++i)
        if (std::memcmp(records.data() + i * 256, readback.data() + i * 256, 88))
            return fail("component registration identity changed during capture");
    std::vector<std::uintptr_t> types_readback(count);
    std::vector<std::uint16_t> sizes_readback(count);
    if (!read(types, types_readback.data(), count * 8) || types_readback != storage_types ||
        !read(sizes, sizes_readback.data(), count * 2) || sizes_readback != storage_sizes)
        return fail("parallel component storage arrays changed during capture");
    output = std::move(snapshot);
    error.clear();
    return true;
}
}
