#pragma once

#include "camera_distance.h"

#include <windows.h>

#include <array>
#include <cstdint>
#include <cstring>

namespace FirstPersonControl {

inline constexpr std::uint32_t ClientTimestamp = 0x6aeb144c;
inline constexpr std::size_t ClientImageSize = 0x2A3F000;
inline constexpr char ClientSha256[] =
    "5d9ef2f140b8ed49e30dbee655841b89d08baec20f799a1817e171a18a850c77";

inline constexpr std::size_t CameraSystemDescriptor = 0x1E24E40;
inline constexpr std::size_t CameraSystemName = 0x1E25DB0;
inline constexpr std::size_t CameraSystemFunction = 0x2B7900;
inline constexpr std::size_t CameraTransformCall = 0x2B7A32;
inline constexpr std::size_t CameraTransformFunction = 0xE98EA0;
inline constexpr std::size_t ShoulderOffset = 0x148C;
inline constexpr std::size_t DistanceOffset = 0x14A4;

inline constexpr std::array<std::uint8_t, 5> OriginalCameraCall = {
    0xE8, 0x69, 0x14, 0xBE, 0x00,
};
inline constexpr std::size_t CameraCallSize = 5;
inline constexpr std::array<std::uint8_t, 18> CameraSystemEntry = {
    0x40, 0x55, 0x57, 0x48, 0x8D, 0xAC, 0x24, 0xB8, 0xFE,
    0xFF, 0xFF, 0x48, 0x81, 0xEC, 0x48, 0x02, 0x00, 0x00,
};
inline constexpr std::array<std::uint8_t, 23> CameraTransformEntry = {
    0x48, 0x8B, 0xC4, 0x48, 0x89, 0x58, 0x08, 0x48,
    0x89, 0x78, 0x10, 0x55, 0x48, 0x8D, 0x68, 0xA1,
    0x48, 0x81, 0xEC, 0xF0, 0x00, 0x00, 0x00,
};
inline constexpr std::array<std::uint8_t, 8> CameraDistanceRead = {
    0xF3, 0x0F, 0x10, 0xA3, 0xA4, 0x14, 0x00, 0x00,
};
inline constexpr std::array<std::uint8_t, 8> CameraShoulderRead = {
    0xF2, 0x0F, 0x10, 0xA3, 0x8C, 0x14, 0x00, 0x00,
};
inline constexpr std::array<std::uint8_t, 4> CameraArgument = {
    0x48, 0x8B, 0x55, 0x88,
};
inline constexpr std::array<std::uint8_t, 4> CameraOutputArgument = {
    0x48, 0x8B, 0x4D, 0x90,
};

inline bool MatchesClientPeIdentity(
    const std::uint8_t* image,
    std::size_t imageSize,
    const IMAGE_NT_HEADERS64** ntHeaders = nullptr) noexcept {
    if (!image || imageSize < sizeof(IMAGE_DOS_HEADER)) return false;

    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE ||
        dos->e_lfanew < static_cast<LONG>(sizeof(IMAGE_DOS_HEADER)) ||
        dos->e_lfanew > 0x1000 ||
        static_cast<std::size_t>(dos->e_lfanew) > imageSize ||
        sizeof(IMAGE_NT_HEADERS64) > imageSize - static_cast<std::size_t>(dos->e_lfanew)) {
        return false;
    }

    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE ||
        nt->FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64 ||
        nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC ||
        nt->FileHeader.TimeDateStamp != ClientTimestamp ||
        nt->OptionalHeader.SizeOfImage != ClientImageSize ||
        imageSize != ClientImageSize) {
        return false;
    }

    if (ntHeaders) *ntHeaders = nt;
    return true;
}

template <std::size_t N>
inline bool MatchesBytes(
    const std::uint8_t* image,
    std::size_t imageSize,
    std::size_t rva,
    const std::array<std::uint8_t, N>& bytes) noexcept {
    return image && rva <= imageSize && N <= imageSize - rva &&
        std::memcmp(image + rva, bytes.data(), N) == 0;
}

inline bool MatchesCameraLayout(
    const std::uint8_t* image,
    std::size_t imageSize,
    std::uintptr_t loadedBase,
    const std::array<std::uint8_t, CameraCallSize>& cameraCall,
    const std::array<std::uint8_t, CameraSystemEntry.size()>& cameraSystemEntry) noexcept {
    if (!MatchesClientPeIdentity(image, imageSize) || !loadedBase) return false;

    constexpr char name[] = "player_camera_transform";
    if (CameraSystemDescriptor > imageSize ||
        sizeof(std::uintptr_t) * 3 > imageSize - CameraSystemDescriptor ||
        CameraSystemName > imageSize || sizeof(name) > imageSize - CameraSystemName) {
        return false;
    }

    std::array<std::uintptr_t, 3> descriptor{};
    std::memcpy(descriptor.data(), image + CameraSystemDescriptor, sizeof(descriptor));
    if (descriptor[0] != loadedBase + CameraSystemName ||
        descriptor[1] != sizeof(name) - 1 ||
        descriptor[2] != loadedBase + CameraSystemFunction ||
        std::memcmp(image + CameraSystemName, name, sizeof(name)) != 0) {
        return false;
    }

    return MatchesBytes(image, imageSize, CameraSystemFunction, cameraSystemEntry) &&
        MatchesBytes(image, imageSize, CameraTransformCall, cameraCall) &&
        MatchesBytes(image, imageSize, CameraTransformFunction, CameraTransformEntry) &&
        MatchesBytes(image, imageSize, 0xE990B1, CameraDistanceRead) &&
        MatchesBytes(image, imageSize, 0xE9900D, CameraShoulderRead) &&
        MatchesBytes(image, imageSize, 0x2B7A15, CameraArgument) &&
        MatchesBytes(image, imageSize, 0x2B7A1C, CameraOutputArgument);
}

} // namespace FirstPersonControl
