// Real file hashing and image identity over a synthetic PE header. No game load.
#include <windows.h>
#include <array>
#include <filesystem>
#include <fstream>
#include <cstdio>
#include <cstdlib>
#include <cstring>
namespace Fixture {
std::filesystem::path process;
std::array<std::uint8_t, 4096> header{};
bool no_module{}, no_path{}, truncated_path{}, execution_profile{};
unsigned loads{}, hooks{};
HMODULE module(LPCWSTR) { return no_module ? nullptr : reinterpret_cast<HMODULE>(header.data()); }
DWORD path(HMODULE, LPWSTR out, DWORD capacity) {
    if (no_path) return 0;
    if (truncated_path) return capacity;
    const auto text = process.wstring();
    if (text.size() + 1 >= capacity) return capacity;
    std::memcpy(out, text.c_str(), (text.size() + 1) * sizeof(wchar_t));
    return static_cast<DWORD>(text.size());
}
}
#define GetModuleHandleW Fixture::module
#define GetModuleFileNameW Fixture::path
#include "profiles/identify_game_image.cpp"
#include "ecs/initialize_metadata.h"
#undef GetModuleFileNameW
#undef GetModuleHandleW
#include "ecs/generic_write_policy.h"
namespace KfcRuntimeCompatibility::EnshroudedClient {
bool Load() { ++Fixture::loads; return Fixture::execution_profile; }
}
namespace EngineHooks { bool Install() { ++Fixture::hooks; return true; } }
namespace {
void check(bool ok) { if (!ok) { std::fputs("Metadata startup check failed\n", stderr); std::exit(1); } }
struct Inputs {
    std::filesystem::path root = std::filesystem::temp_directory_path() /
        ("sf-metadata-" + std::to_string(GetCurrentProcessId()) + "-" + std::to_string(GetTickCount64()));
    Inputs() {
        check(std::filesystem::create_directory(root));
        Fixture::process = root / "synthetic.exe";
        IMAGE_DOS_HEADER dos{}; dos.e_magic = IMAGE_DOS_SIGNATURE; dos.e_lfanew = 128;
        IMAGE_NT_HEADERS64 nt{}; nt.Signature = IMAGE_NT_SIGNATURE;
        nt.OptionalHeader.Magic = IMAGE_NT_OPTIONAL_HDR64_MAGIC;
        nt.OptionalHeader.SizeOfImage = 4096; nt.FileHeader.TimeDateStamp = 42;
        std::memcpy(Fixture::header.data(), &dos, sizeof(dos));
        std::memcpy(Fixture::header.data() + 128, &nt, sizeof(nt));
        std::ofstream exe(Fixture::process, std::ios::binary);
        exe.write(reinterpret_cast<const char*>(Fixture::header.data()), Fixture::header.size());
        check(bool(exe));
        std::ofstream kfc(root / "synthetic.kfc", std::ios::binary); kfc.write("KFC4", 4); check(bool(kfc));
    }
    ~Inputs() {
        std::error_code error;
        std::filesystem::remove(root / "synthetic.exe", error);
        std::filesystem::remove(root / "synthetic.kfc", error);
        std::filesystem::remove(root, error);
    }
};
}
int main() {
    Inputs inputs;
    namespace Profile = KfcRuntimeCompatibility::EnshroudedClient;
    std::uintptr_t base{}; std::size_t size{}; std::atomic<bool> stopped{true};
    const auto prepare = [&](bool hooks) { return EcsImplementation::InitializeMetadata(hooks, base, size, stopped); };
    check(prepare(false) && !stopped && base && size == 4096);
    check(Profile::image_sha256.size() == 64 && Profile::image_target == "synthetic.exe" &&
        Profile::image_timestamp == 42 && Profile::kfc_format == 4 && !Profile::exact_build_match);
    check(Fixture::loads == 0 && Fixture::hooks == 0);
    Fixture::no_path = true; check(!prepare(false)); Fixture::no_path = false;
    Fixture::truncated_path = true; check(!prepare(false)); Fixture::truncated_path = false;
    Fixture::no_module = true; check(!prepare(false)); Fixture::no_module = false;
    check(prepare(true) && Fixture::loads == 1 && Fixture::hooks == 0);
    Fixture::execution_profile = true; check(prepare(true) && Fixture::hooks == 1);
    check(std::filesystem::remove(Fixture::process));
    check(!prepare(false)); // A readable mapped header cannot substitute for file identity.

    EcsImplementation::GenericWritePolicy policy;
    check(policy.Allows(false));
    policy.Selected(false); check(!policy.Allows(false) && !policy.Allows(true));
    policy.Selected(true); check(!policy.Allows(false) && policy.Allows(true) && !policy.Allows(true, false));
    policy.Legacy(); check(policy.Allows(false));
    std::puts("PASS: metadata identity without hooks, missing inputs refused, selected write fence");
}
