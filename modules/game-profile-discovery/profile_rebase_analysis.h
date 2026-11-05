#pragma once
#include <array>
#include <cstdint>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

namespace GameProfileDiscovery::Detail {
struct Reference { unsigned range{}, offset{}; std::uint32_t target{}; bool branch{}, indexed_image{}; unsigned table_entries{}, table_width{4}, read_bytes{}; bool address_only{}; unsigned write_bytes{}; };
struct LayoutUse {
    std::uint32_t instruction{};
    unsigned operand{};
    std::int64_t value{};
    int argument{-1};
    std::string kind;
    unsigned memory_width_bytes{};
    bool memory_read{}, memory_write{};
    std::int64_t index_stride{};
};
struct CallArguments {
    std::uint32_t instruction{}, target{};
    std::array<std::optional<std::int64_t>, 4> offsets;
    std::array<std::optional<unsigned>, 4> original_arguments;
};
struct Procedure {
    std::uint32_t root{};
    std::vector<std::pair<std::uint32_t, std::uint32_t>> ranges;
    std::string signature;
    std::vector<Reference> references;
    std::set<std::int64_t> constants;
    std::vector<LayoutUse> layout_uses;
    std::vector<CallArguments> call_arguments;
    std::size_t code_bytes{};
    bool complete{true};
    bool references_complete{true};
    std::uint32_t reference_failure_rva{};
    std::string reference_failure;
};
struct ReflectedType {
    std::uint32_t metadata{}, slot{}, hash{}, size{};
    std::string name;
    nlohmann::json layout;
};
struct ProfileRebaseAnalysis {
    const std::map<std::uint32_t, Procedure>& source_functions;
    const std::map<std::uint32_t, Procedure>& target_functions;
    const std::map<std::uint32_t, std::uint32_t>& exact_counterparts;
    const std::map<std::string, ReflectedType>& source_types;
    const std::map<std::string, ReflectedType>& target_types;
    std::map<std::string, std::set<std::uint32_t>> named_procedures;
    std::map<std::string, std::set<std::uint32_t>> named_systems;
    std::map<std::string, std::uint32_t> established_rvas;
};
}
