#include "verify_image_equivalence.h"
#include <windows.h>
#include <bcrypt.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <fstream>
#include <limits>
#include <stdexcept>

namespace GameProfileDiscovery::Detail {
namespace {
template <typename T> T read(const std::vector<std::uint8_t>& bytes, std::size_t position) {
    if (position > bytes.size() || sizeof(T) > bytes.size() - position)
        throw std::runtime_error("truncated-pe-structure");
    T value{};
    std::memcpy(&value, bytes.data() + position, sizeof(value));
    return value;
}

std::string sha256(const std::vector<std::uint8_t>& bytes) {
    BCRYPT_ALG_HANDLE algorithm{};
    BCRYPT_HASH_HANDLE hash{};
    DWORD object_size{}, digest_size{}, received{};
    if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0)
        throw std::runtime_error("sha256-provider-unavailable");
    const auto close = [&] {
        if (hash) BCryptDestroyHash(hash);
        BCryptCloseAlgorithmProvider(algorithm, 0);
    };
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_size),
            sizeof(object_size), &received, 0) < 0 ||
        BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH, reinterpret_cast<PUCHAR>(&digest_size),
            sizeof(digest_size), &received, 0) < 0 || digest_size != 32) {
        close();
        throw std::runtime_error("sha256-provider-properties-invalid");
    }
    std::vector<std::uint8_t> object(object_size), digest(digest_size);
    if (BCryptCreateHash(algorithm, &hash, object.data(), object_size, nullptr, 0, 0) < 0 ||
        BCryptHashData(hash, const_cast<PUCHAR>(bytes.data()), static_cast<ULONG>(bytes.size()), 0) < 0 ||
        BCryptFinishHash(hash, digest.data(), digest_size, 0) < 0) {
        close();
        throw std::runtime_error("sha256-failed");
    }
    close();
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    result.reserve(64);
    for (const auto byte : digest) {
        result.push_back(digits[byte >> 4]);
        result.push_back(digits[byte & 15]);
    }
    return result;
}

void zero(std::vector<std::uint8_t>& bytes, std::size_t position, std::size_t count) {
    if (position > bytes.size() || count > bytes.size() - position)
        throw std::runtime_error("normalized-pe-field-outside-headers");
    std::fill_n(bytes.begin() + position, count, std::uint8_t{});
}
}

Image ReadImage(const std::filesystem::path& path, bool read_adjacent_kfc) {
    std::ifstream stream(path, std::ios::binary | std::ios::ate);
    if (!stream) throw std::runtime_error("executable-unreadable");
    const auto length = stream.tellg();
    if (length < static_cast<std::streamoff>(sizeof(IMAGE_DOS_HEADER)) || length > (1ll << 30))
        throw std::runtime_error("executable-size-out-of-bounds");
    Image image;
    image.file_bytes.resize(static_cast<std::size_t>(length));
    stream.seekg(0);
    if (!stream.read(reinterpret_cast<char*>(image.file_bytes.data()), length))
        throw std::runtime_error("executable-read-incomplete");
    const auto& bytes = image.file_bytes;
    const auto dos = read<IMAGE_DOS_HEADER>(bytes, 0);
    if (dos.e_magic != IMAGE_DOS_SIGNATURE || dos.e_lfanew < 0)
        throw std::runtime_error("invalid-dos-header");
    const auto nt_offset = static_cast<std::size_t>(dos.e_lfanew);
    const auto nt = read<IMAGE_NT_HEADERS64>(bytes, nt_offset);
    if (nt.Signature != IMAGE_NT_SIGNATURE || nt.FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64 ||
        nt.FileHeader.SizeOfOptionalHeader != sizeof(IMAGE_OPTIONAL_HEADER64) ||
        nt.OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC ||
        nt.OptionalHeader.NumberOfRvaAndSizes != IMAGE_NUMBEROF_DIRECTORY_ENTRIES ||
        !nt.FileHeader.NumberOfSections || nt.FileHeader.NumberOfSections > 96 ||
        nt.OptionalHeader.SizeOfImage < 4096 || nt.OptionalHeader.SizeOfImage > (1u << 30))
        throw std::runtime_error("unsupported-pe-image-layout");
    const auto sections_offset = nt_offset + sizeof(IMAGE_NT_HEADERS64);
    const auto header_size = nt.OptionalHeader.SizeOfHeaders;
    if (header_size > bytes.size() || sections_offset > header_size ||
        nt.FileHeader.NumberOfSections * sizeof(IMAGE_SECTION_HEADER) > header_size - sections_offset)
        throw std::runtime_error("invalid-pe-header-extent");
    const auto& certificate = nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_SECURITY];
    if ((certificate.Size && (!certificate.VirtualAddress || certificate.VirtualAddress < header_size ||
            certificate.VirtualAddress > bytes.size() || certificate.Size > bytes.size() - certificate.VirtualAddress)) ||
        (!certificate.Size && certificate.VirtualAddress))
        throw std::runtime_error("invalid-authenticode-file-extent");
    image.identity = {path.filename().string(), sha256(bytes), nt.FileHeader.TimeDateStamp,
                      nt.OptionalHeader.SizeOfImage, 0};
    image.normalized_headers.assign(bytes.begin(), bytes.begin() + header_size);
    zero(image.normalized_headers, nt_offset + offsetof(IMAGE_NT_HEADERS64, FileHeader) +
         offsetof(IMAGE_FILE_HEADER, TimeDateStamp), sizeof(DWORD));
    zero(image.normalized_headers, nt_offset + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
         offsetof(IMAGE_OPTIONAL_HEADER64, CheckSum), sizeof(DWORD));
    zero(image.normalized_headers, nt_offset + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
         offsetof(IMAGE_OPTIONAL_HEADER64, DataDirectory) + IMAGE_DIRECTORY_ENTRY_SECURITY * sizeof(IMAGE_DATA_DIRECTORY),
         sizeof(IMAGE_DATA_DIRECTORY));
    for (unsigned index = 0; index < nt.FileHeader.NumberOfSections; ++index) {
        const auto section = read<IMAGE_SECTION_HEADER>(bytes, sections_offset + index * sizeof(IMAGE_SECTION_HEADER));
        const auto extent = std::max(section.Misc.VirtualSize, section.SizeOfRawData);
        if (section.VirtualAddress > image.identity.size || extent > image.identity.size - section.VirtualAddress ||
            section.PointerToRawData > bytes.size() || section.SizeOfRawData > bytes.size() - section.PointerToRawData ||
            (section.SizeOfRawData && section.PointerToRawData < header_size))
            throw std::runtime_error("pe-section-outside-image-or-file");
        if (certificate.Size && section.SizeOfRawData &&
            certificate.VirtualAddress < section.PointerToRawData + section.SizeOfRawData &&
            section.PointerToRawData < certificate.VirtualAddress + certificate.Size)
            throw std::runtime_error("authenticode-overlaps-mapped-section");
        for (const auto& previous : image.sections) {
            const auto previous_extent = std::max(previous.virtual_size, static_cast<std::uint32_t>(previous.bytes.size()));
            if (extent && previous_extent && section.VirtualAddress < previous.rva + previous_extent &&
                previous.rva < section.VirtualAddress + extent)
                throw std::runtime_error("overlapping-pe-sections");
        }
        std::size_t name_size{};
        while (name_size < IMAGE_SIZEOF_SHORT_NAME && section.Name[name_size]) ++name_size;
        image.sections.push_back({std::string(reinterpret_cast<const char*>(section.Name), name_size),
            section.VirtualAddress, section.Misc.VirtualSize, section.Characteristics,
            std::vector<std::uint8_t>(bytes.begin() + section.PointerToRawData,
                bytes.begin() + section.PointerToRawData + section.SizeOfRawData)});
    }
    if (read_adjacent_kfc) {
        auto kfc_path = path;
        kfc_path.replace_extension(L".kfc");
        std::ifstream kfc(kfc_path, std::ios::binary);
        std::array<char, 4> header{};
        if (!kfc.read(header.data(), header.size()) || header[0] != 'K' || header[1] != 'F' ||
            header[2] != 'C' || (header[3] != '3' && header[3] != '4'))
            throw std::runtime_error("adjacent-kfc-format-unavailable");
        image.identity.kfc_format = static_cast<std::uint8_t>(header[3] - '0');
    }
    return image;
}

bool Equivalent(const Image& source, const Image& target, nlohmann::json& evidence) {
    evidence = {{"method", "identical-mapped-pe-image"}, {"equivalent", false},
        {"normalizedHeaderFields", {"coff-timestamp", "optional-checksum", "authenticode-security-directory"}},
        {"relocatedRvas", false}, {"structureOffsetsInferred", false}};
    if (source.normalized_headers != target.normalized_headers) {
        const auto difference = std::mismatch(source.normalized_headers.begin(), source.normalized_headers.end(),
            target.normalized_headers.begin(), target.normalized_headers.end());
        evidence["reason"] = "mapped-pe-header-layout-changed";
        evidence["firstHeaderDifference"] = static_cast<std::size_t>(difference.first - source.normalized_headers.begin());
        return false;
    }
    if (source.sections.size() != target.sections.size()) {
        evidence["reason"] = "pe-section-count-changed";
        return false;
    }
    std::size_t checked_bytes{};
    for (std::size_t index = 0; index < source.sections.size(); ++index) {
        const auto& before = source.sections[index];
        const auto& after = target.sections[index];
        if (before.rva != after.rva || before.virtual_size != after.virtual_size || before.flags != after.flags ||
            before.name != after.name || before.bytes != after.bytes) {
            evidence["reason"] = before.flags & IMAGE_SCN_MEM_EXECUTE ? "executable-section-changed" : "data-section-changed";
            evidence["section"] = before.name;
            const auto mismatch = std::mismatch(before.bytes.begin(), before.bytes.end(), after.bytes.begin(), after.bytes.end());
            evidence["firstSectionDifferenceRva"] = before.rva + static_cast<std::size_t>(mismatch.first - before.bytes.begin());
            return false;
        }
        checked_bytes += before.bytes.size();
    }
    evidence["equivalent"] = true;
    evidence["reason"] = "all-mapped-sections-and-layout-identical";
    evidence["checkedSectionBytes"] = checked_bytes;
    evidence["checkedSections"] = source.sections.size();
    return true;
}
}
