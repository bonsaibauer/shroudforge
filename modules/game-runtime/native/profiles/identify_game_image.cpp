#include "profiles/identify_game_image.h"
#include "profiles/game_profile.h"
#include <bcrypt.h>
#include <array>
#include <filesystem>
#include <fstream>
#include <vector>

namespace KfcRuntimeCompatibility::EnshroudedClient {
namespace {
std::string sha256_file(const std::filesystem::path& path) {
    BCRYPT_ALG_HANDLE algorithm{};
    BCRYPT_HASH_HANDLE hash{};
    DWORD object_size{}, digest_size{}, received{};
    if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0) return {};
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_size), sizeof(object_size), &received, 0) < 0 ||
        BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH, reinterpret_cast<PUCHAR>(&digest_size), sizeof(digest_size), &received, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::vector<UCHAR> object(object_size), digest(digest_size), chunk(1u << 20);
    if (BCryptCreateHash(algorithm, &hash, object.data(), object_size, nullptr, 0, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::ifstream input(path, std::ios::binary);
    bool ok = static_cast<bool>(input);
    while (ok && input) {
        input.read(reinterpret_cast<char*>(chunk.data()), static_cast<std::streamsize>(chunk.size()));
        const auto count = input.gcount();
        if (count > 0 && BCryptHashData(hash, chunk.data(), static_cast<ULONG>(count), 0) < 0) ok = false;
    }
    ok = ok && input.eof() && BCryptFinishHash(hash, digest.data(), digest_size, 0) >= 0;
    BCryptDestroyHash(hash);
    BCryptCloseAlgorithmProvider(algorithm, 0);
    if (!ok) return {};
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    result.reserve(digest.size() * 2);
    for (const auto byte : digest) { result.push_back(digits[byte >> 4]); result.push_back(digits[byte & 15]); }
    return result;
}
}

bool IdentifyGameImage(const wchar_t* process_path, const std::uint8_t*& base, const IMAGE_NT_HEADERS64*& nt) {
    const auto process_hash = sha256_file(process_path);
    base = reinterpret_cast<const std::uint8_t*>(GetModuleHandleW(nullptr));
    if (!base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE || nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return false;
    // Loaded-image identity is needed independently of discovery or a compatible
    // execution profile exists. Profiles only authorize engine adapters.
    image_timestamp = nt->FileHeader.TimeDateStamp;
    image_size = nt->OptionalHeader.SizeOfImage;
    image_sha256 = process_hash;
    image_target = std::filesystem::path(process_path).filename().string();
    exact_build_match = false;
    kfc_format = 0;
    auto kfc_path = std::filesystem::path(process_path);
    kfc_path.replace_extension(L".kfc");
    std::ifstream kfc_file(kfc_path, std::ios::binary);
    std::array<char, 4> kfc_header{};
    if (kfc_file.read(kfc_header.data(), kfc_header.size()) &&
        kfc_header[0] == 'K' && kfc_header[1] == 'F' && kfc_header[2] == 'C' &&
        (kfc_header[3] == '3' || kfc_header[3] == '4'))
        kfc_format = static_cast<std::uint8_t>(kfc_header[3] - '0');
    return true;
}
}
