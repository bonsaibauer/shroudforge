#pragma once
#include <atomic>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string_view>

namespace MinimapMenu10
{
    constexpr std::uintptr_t RendererRva = 0xc4e0c0;
    constexpr std::uintptr_t UiContextRva = 0x281b498;
    constexpr std::uintptr_t HolderOffset = 0xd0;
    constexpr std::uintptr_t PlayStateOffset = 0x38;
    constexpr std::uintptr_t ScreenOffset = 0x1cd8;
    constexpr std::uint64_t MaxAgeMs = 500;
    constexpr std::uint64_t MaxTick = UINT64_MAX >> 8;

    using GuardReader = bool (*)(const std::uint8_t*, void*, std::size_t);

    namespace detail
    {
        using namespace std::literals;
        struct Guard { std::size_t rva; std::string_view bytes; };
        inline bool MatchGuardTable(
            const std::uint8_t* image, std::size_t imageSize,
            const std::uint8_t* rendererPrefix, std::size_t rendererPrefixSize,
            const std::uint8_t* callPrefix, std::size_t callPrefixSize, GuardReader reader, bool currentBuild)
        {
            constexpr Guard oldGuards[] = {
            {0xc4e0c0, "\x4c\x8b\xdc\x55\x49\x8d\xab\xe8\xfd\xff\xff\x48\x81\xec\x10\x03\x00\x00"sv},
            {0xc4e0d2, "\x48\x8b\x05\xbf\xd3\xbc\x01\x48\x8b\x88\xd0\x00\x00\x00\x48\x8b\x01\x48\x85\xc0\x0f\x84\x94\x0e\x00\x00\x4d\x89\x73\xc8\x4c\x8b\x70\x38\x4c\x89\xb5\x30\x02\x00\x00\x4d\x85\xf6\x0f\x84\x74\x0e\x00\x00\x4d\x89\x63\xd8"sv},
            {0xc4e108, "\x4d\x8d\xa6\xd8\x1c\x00\x00\x41\x0f\xb6\x04\x24\x4c\x89\x65\x80\x3c\x33\x0f\x84\x50\x0e\x00\x00\x84\xc0\x0f\x84\x48\x0e\x00\x00"sv},
            {0xbc0446, "\x48\x89\x05\x4b\xb0\xc5\x01"sv},
            {0xbc0969, "\x4c\x89\x25\x28\xab\xc5\x01"sv},
            {0xbc078b, "\x41\x0f\xb6\x47\x0c\xff\xc8\x83\xf8\x06\x0f\x87\xa4\x01\x00\x00\x48\x8d\x15\x5e\xf8\x43\xff\x48\x98\x8b\x8c\x82\xd4\x09\xbc\x00\x48\x03\xca\xff\xe1"sv},
            {0xbc09d4, "\xb0\x07\xbc\x00\xba\x07\xbc\x00\xc4\x07\xbc\x00\xce\x07\xbc\x00\x3b\x08\xbc\x00\xa8\x08\xbc\x00\x3a\x09\xbc\x00"sv},
            {0xbc093a, "\xe8\x81\xd7\x08\x00\xe8\x3c\x78\x0b\x00"sv},
            {0xd6ade0, "\x48\x89\x0d\xb1\x06\xab\x01\xc3"sv}
            };
            constexpr Guard currentGuards[] = {
            {0xc4fc40, "\x4c\x8b\xdc\x55\x49\x8d\xab\xe8\xfd\xff\xff\x48\x81\xec\x10\x03\x00\x00"sv},
            {0xc4fc52, "\x48\x8b\x05\x3f\xe8\xbc\x01\x48\x8b\x88\xd0\x00\x00\x00\x48\x8b\x01\x48\x85\xc0\x0f\x84\x94\x0e\x00\x00\x4d\x89\x73\xc8\x4c\x8b\x70\x38\x4c\x89\xb5\x30\x02\x00\x00\x4d\x85\xf6\x0f\x84\x74\x0e\x00\x00\x4d\x89\x63\xd8"sv},
            {0xc4fc88, "\x4d\x8d\xa6\xd8\x1c\x00\x00\x41\x0f\xb6\x04\x24\x4c\x89\x65\x80\x3c\x33\x0f\x84\x50\x0e\x00\x00\x84\xc0\x0f\x84\x48\x0e\x00\x00"sv},
            {0xbc1fc6, "\x48\x89\x05\xcb\xc4\xc5\x01"sv},
            {0xbc24e9, "\x4c\x89\x25\xa8\xbf\xc5\x01"sv},
            {0xbc230b, "\x41\x0f\xb6\x47\x0c\xff\xc8\x83\xf8\x06\x0f\x87\xa4\x01\x00\x00\x48\x8d\x15\xde\xdc\x43\xff\x48\x98\x8b\x8c\x82\x54\x25\xbc\x00\x48\x03\xca\xff\xe1"sv},
            {0xbc2554, "\x30\x23\xbc\x00\x3a\x23\xbc\x00\x44\x23\xbc\x00\x4e\x23\xbc\x00\xbb\x23\xbc\x00\x28\x24\xbc\x00\xba\x24\xbc\x00"sv},
            {0xbc24ba, "\xe8\x81\xd7\x08\x00\xe8\x4c\x77\x0b\x00"sv},
            {0xd6c860, "\x48\x89\x0d\x31\x1c\xab\x01\xc3"sv}
            };
            const auto& guards = currentBuild ? currentGuards : oldGuards;
            const auto rendererRva = currentBuild ? 0xc4fc40u : RendererRva;
            const auto callRva = currentBuild ? 0xbc24bau : 0xbc093au;
            if (!image) return false;

            if (rendererPrefix || rendererPrefixSize != 0)
            {
                const Guard* rendererGuard = nullptr;
                for (const auto& guard : guards)
                    if (guard.rva == rendererRva) { rendererGuard = &guard; break; }
                if (!rendererPrefix || !rendererGuard || rendererPrefixSize != rendererGuard->bytes.size() ||
                    rendererGuard->rva > imageSize || rendererGuard->bytes.size() > imageSize - rendererGuard->rva ||
                    std::memcmp(rendererPrefix, rendererGuard->bytes.data(), rendererGuard->bytes.size()) != 0)
                    return false;
            }

            if (callPrefix || callPrefixSize) {
                // Only the independently proved retained CALL's original5B can
                // be overlaid. Its following CALL remains an ordinary guard.
                if (!callPrefix || callPrefixSize != 5) return false;
                for (const auto& guard : guards)
                    if (guard.rva == callRva && std::memcmp(callPrefix, guard.bytes.data(), 5)) return false;
            }
            for (const auto& guard : guards)
            {
                if (rendererPrefix && guard.rva == rendererRva) continue;
                if (guard.rva > imageSize || guard.bytes.size() > imageSize - guard.rva || guard.bytes.size() > 64)
                    return false;
                std::array<std::uint8_t,64> actual{};
                if (reader) {
                    if (!reader(image + guard.rva, actual.data(), guard.bytes.size())) return false;
                } else std::memcpy(actual.data(), image + guard.rva, guard.bytes.size());
                if (callPrefix && guard.rva == callRva) std::memcpy(actual.data(), callPrefix, 5);
                if (std::memcmp(actual.data(), guard.bytes.data(), guard.bytes.size())) return false;
            }
            return true;
        }
    }

    // Check after exact-profile admission and before patching; the UI context is stack-local.
    inline bool MatchGuards(const std::uint8_t* image, std::size_t imageSize, GuardReader reader = nullptr, bool currentBuild = false)
    {
        return detail::MatchGuardTable(image, imageSize, nullptr, 0, nullptr, 0, reader, currentBuild);
    }

    // Only the exact original 18-byte renderer entry may be replaced after ownership proof.
    inline bool MatchGuardsWithRendererPrefix(
        const std::uint8_t* image, std::size_t imageSize,
        const std::uint8_t* rendererPrefix, std::size_t rendererPrefixSize,
        const std::uint8_t* ownedCallPrefix = nullptr, std::size_t ownedCallSize = 0, GuardReader reader = nullptr, bool currentBuild = false)
    {
        return detail::MatchGuardTable(image, imageSize, rendererPrefix, rendererPrefixSize, ownedCallPrefix, ownedCallSize, reader, currentBuild);
    }

    class Snapshot
    {
        std::atomic<std::uint64_t> packed{0};
    public:
        void Clear() { packed.store(0); }
        void Capture(std::uint8_t screen, std::uint64_t now, bool readSucceeded)
        {
            if (!readSucceeded || screen == 0 || screen > 53 || now > MaxTick) { Clear(); return; }
            packed.store((now << 8) | screen);
        }
        bool AllowsGameplay(std::uint64_t now) const
        {
            const auto value = packed.load();
            const auto captured = value >> 8;
            return (value & 0xff) == 1 && now >= captured && now - captured <= MaxAgeMs;
        }
    };
}
