#pragma once

// Pure value planner for the exact new-profile native iterator extent.
//
// The caller supplies one bounded metadata snapshot and the actual destination
// capacity. This helper performs no reads, allocations, native calls or caching.
// The eventual consumer must keep the snapshot, returned plan, private context
// and destination as distinct stack-local objects. It must also rely on the
// approved normal synchronous-call premise that native count/control/source
// metadata remains stable through Init and the private Next sequence. These
// checks do not provide TOCTOU immunity if another writer changes that metadata.
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace NativeIteratorExtent
{
    static_assert(sizeof(std::uintptr_t) == sizeof(std::uint64_t),
        "The native iterator contract requires a 64-bit process.");

    constexpr std::uintptr_t MinLikelyPointerExclusive = 0x10000;
    constexpr std::uintptr_t UserAddressLimitExclusive = 0x0000800000000000ULL;
    constexpr std::size_t PrivateContextBytes = 2 * sizeof(std::uint64_t);
    constexpr std::uint64_t QueryControlBytes = 0x840;
    constexpr std::uint64_t P3CountThroughEndBytes = 0x401;
    constexpr std::uint64_t P8CountThroughEndBytes = 0x81;
    constexpr std::uint32_t MaximumNativeCursor = (std::numeric_limits<std::uint32_t>::max)();

    struct Interval
    {
        std::uintptr_t begin = 0;
        std::uint64_t size = 0;
    };

    struct NativeCounts
    {
        // Exact native loaded widths in Init order: N0=Q+0x4B0, N1=Q+0x4B8,
        // N2=Q+0x4BA, N3=P3+0x400, N4=Q+0x7F8, N5=Q+0x7A8,
        // N6=qword Q+0x1A8, N7=Q+0x1B0, N8=P8+0x80. The caller supplies values;
        // this helper does not read any of those source locations.
        std::uint8_t n0 = 0;
        std::uint16_t n1 = 0;
        std::uint16_t n2 = 0;
        std::uint8_t n3 = 0;
        std::uint8_t n4 = 0;
        std::uint8_t n5 = 0;
        std::uint64_t n6 = 0;
        std::uint32_t n7 = 0;
        std::uint8_t n8 = 0;
    };

    struct MetadataSnapshot
    {
        // Address of the private two-qword context object passed to the native
        // iterator; query/cursor are its already-copied values. row_count is
        // the qword at Q+0x838, and cursor is the private DWORD widened here.
        std::uintptr_t private_context_storage = 0;
        std::uintptr_t query = 0;
        std::uint64_t cursor = 0;
        std::uintptr_t p3 = 0;
        std::uintptr_t p8 = 0;
        std::uintptr_t s6 = 0;
        NativeCounts counts{};
        std::uint64_t row_count = 0;
    };

    enum CopySourceIndex : std::size_t
    {
        P3Copy = 0,
        Q7F8Copy = 1,
        Q4C0Copy = 2,
        S6Copy = 3,
        P8Copy = 4,
        CopySourceCount = 5
    };

    struct IteratorExtentPlan
    {
        bool valid = false;
        std::uintptr_t private_context_storage = 0;
        std::uintptr_t query = 0;
        std::uintptr_t destination = 0;
        std::uint64_t destination_capacity = 0;
        std::uint64_t initial_cursor = 0;
        std::uint64_t row_count = 0;
        NativeCounts counts{};
        // [0] is the mandatory context qword end; [1..9] are the exclusive
        // ends after N0 through N8 respectively.
        std::array<std::uint64_t, 10> init_prefix_ends{};
        Interval private_context_range{};
        Interval destination_range{};
        Interval query_control_range{};
        Interval p3_control_range{};
        Interval p8_control_range{};
        Interval s6_source_range{};
        std::array<Interval, CopySourceCount> copy_source_ranges{};
    };

    enum class NextDisposition : std::uint8_t
    {
        Reject,
        Stop,
        Call
    };

    struct NextPlan
    {
        NextDisposition disposition = NextDisposition::Reject;
        std::uint32_t cursor_before = 0;
        std::uint32_t expected_cursor_after = 0;
        // Exclusive byte end from the start of the actual destination record.
        std::uint64_t destination_end = 0;
    };

    inline bool IsLikelyPointer(std::uintptr_t value) noexcept
    {
        return value > MinLikelyPointerExclusive && value < UserAddressLimitExclusive;
    }

    inline bool MakeInterval(std::uintptr_t begin, std::uint64_t size,
        Interval& output) noexcept
    {
        output = {};
        if (size == 0)
            return true;
        if (!IsLikelyPointer(begin))
            return false;
        const auto available = static_cast<std::uint64_t>(UserAddressLimitExclusive - begin);
        if (size > available)
            return false;
        output = {begin, size};
        return true;
    }

    inline bool MakeOffsetInterval(std::uintptr_t base, std::uint64_t offset,
        std::uint64_t size, Interval& output) noexcept
    {
        output = {};
        if (size == 0)
            return true;
        if (!IsLikelyPointer(base))
            return false;
        const auto available = static_cast<std::uint64_t>(UserAddressLimitExclusive - base);
        if (offset >= available)
            return false;
        const auto start = base + static_cast<std::uintptr_t>(offset);
        return MakeInterval(start, size, output);
    }

    inline bool Overlaps(const Interval& left, const Interval& right) noexcept
    {
        if (left.size == 0 || right.size == 0)
            return false;
        if (left.begin <= right.begin)
            return static_cast<std::uint64_t>(right.begin - left.begin) < left.size;
        return static_cast<std::uint64_t>(left.begin - right.begin) < right.size;
    }

    inline bool TryAddTerm(std::uint64_t current, std::uint64_t count,
        std::uint64_t coefficient, std::uint64_t capacity, std::uint64_t& next,
        std::uint64_t& termBytes) noexcept
    {
        if (coefficient == 0 || current > capacity ||
            count > (capacity - current) / coefficient)
            return false;
        termBytes = count * coefficient;
        next = current + termBytes;
        return true;
    }

    inline bool IntersectsAny(const Interval& candidate, const IteratorExtentPlan& plan) noexcept
    {
        if (Overlaps(candidate, plan.destination_range) ||
            Overlaps(candidate, plan.private_context_range) ||
            Overlaps(candidate, plan.query_control_range) ||
            Overlaps(candidate, plan.p3_control_range) ||
            Overlaps(candidate, plan.p8_control_range) ||
            Overlaps(candidate, plan.s6_source_range))
            return true;
        for (const auto& source : plan.copy_source_ranges)
            if (Overlaps(candidate, source))
                return true;
        return false;
    }

    inline IteratorExtentPlan TryBuildPlan(const MetadataSnapshot& snapshot,
        std::uintptr_t destination, std::size_t actualCapacity) noexcept
    {
        IteratorExtentPlan plan{};
        Interval snapshotStorage{};
        if (!MakeInterval(reinterpret_cast<std::uintptr_t>(&snapshot), sizeof(snapshot), snapshotStorage) ||
            !MakeInterval(snapshot.private_context_storage, PrivateContextBytes, plan.private_context_range) ||
            !MakeInterval(destination, static_cast<std::uint64_t>(actualCapacity), plan.destination_range) ||
            !MakeInterval(snapshot.query, QueryControlBytes, plan.query_control_range) ||
            snapshot.row_count > MaximumNativeCursor || snapshot.cursor > MaximumNativeCursor ||
            snapshot.cursor > snapshot.row_count)
            return {};

        const std::uint64_t capacity = static_cast<std::uint64_t>(actualCapacity);
        if (capacity < sizeof(std::uint64_t))
            return {};

        std::array<std::uint64_t, 9> counts = {
            snapshot.counts.n0, snapshot.counts.n1, snapshot.counts.n2,
            snapshot.counts.n3, snapshot.counts.n4, snapshot.counts.n5,
            snapshot.counts.n6, snapshot.counts.n7, snapshot.counts.n8};
        constexpr std::array<std::uint64_t, 9> coefficients = {8, 8, 48, 8, 8, 8, 8, 8, 2};
        std::array<std::uint64_t, 9> termBytes{};
        std::uint64_t total = sizeof(std::uint64_t);
        plan.init_prefix_ends[0] = total;
        for (std::size_t i = 0; i < counts.size(); ++i)
        {
            std::uint64_t nextTotal = 0;
            if (!TryAddTerm(total, counts[i], coefficients[i], capacity,
                nextTotal, termBytes[i]))
                return {};
            total = nextTotal;
            plan.init_prefix_ends[i + 1] = total;
        }

        const auto p3ControlSize = (termBytes[3] < P3CountThroughEndBytes)
            ? P3CountThroughEndBytes : termBytes[3];
        const auto p8ControlSize = (termBytes[8] < P8CountThroughEndBytes)
            ? P8CountThroughEndBytes : termBytes[8];
        if (!MakeInterval(snapshot.p3, p3ControlSize, plan.p3_control_range) ||
            !MakeInterval(snapshot.p8, p8ControlSize, plan.p8_control_range))
            return {};

        if (termBytes[6] != 0 &&
            !MakeInterval(snapshot.s6, termBytes[6], plan.s6_source_range))
            return {};
        if (!MakeInterval(snapshot.p3, termBytes[3], plan.copy_source_ranges[P3Copy]) ||
            !MakeOffsetInterval(snapshot.query, 0x7b0, termBytes[4],
                plan.copy_source_ranges[Q7F8Copy]) ||
            !MakeOffsetInterval(snapshot.query, 0x4c0, termBytes[5],
                plan.copy_source_ranges[Q4C0Copy]) ||
            !MakeInterval(snapshot.s6, termBytes[6], plan.copy_source_ranges[S6Copy]) ||
            !MakeInterval(snapshot.p8, termBytes[8], plan.copy_source_ranges[P8Copy]))
            return {};

        const auto& destinationRange = plan.destination_range;
        const auto& contextRange = plan.private_context_range;
        if (Overlaps(destinationRange, contextRange) ||
            Overlaps(destinationRange, plan.query_control_range) ||
            Overlaps(destinationRange, plan.p3_control_range) ||
            Overlaps(destinationRange, plan.p8_control_range) ||
            Overlaps(destinationRange, plan.s6_source_range) ||
            Overlaps(contextRange, plan.query_control_range) ||
            Overlaps(contextRange, plan.p3_control_range) ||
            Overlaps(contextRange, plan.p8_control_range) ||
            Overlaps(contextRange, plan.s6_source_range))
            return {};
        for (const auto& source : plan.copy_source_ranges)
        {
            if (Overlaps(destinationRange, source) || Overlaps(contextRange, source))
                return {};
        }
        if (IntersectsAny(snapshotStorage, plan))
            return {};

        plan.valid = true;
        plan.private_context_storage = snapshot.private_context_storage;
        plan.query = snapshot.query;
        plan.destination = destination;
        plan.destination_capacity = capacity;
        plan.initial_cursor = snapshot.cursor;
        plan.row_count = snapshot.row_count;
        plan.counts = snapshot.counts;
        return plan;
    }

    inline NextPlan PlanNext(const IteratorExtentPlan& plan, std::uint64_t cursor,
        std::uint32_t cursorLimit) noexcept
    {
        NextPlan next{};
        if (!plan.valid || cursor < plan.initial_cursor ||
            cursor > MaximumNativeCursor || cursor > plan.row_count)
            return next;
        if (cursor == plan.row_count || cursor >= cursorLimit)
        {
            next.disposition = NextDisposition::Stop;
            next.cursor_before = static_cast<std::uint32_t>(cursor);
            return next;
        }
        if (cursor >= MaximumNativeCursor)
            return next;

        const auto listEnd = plan.init_prefix_ends[2];
        std::uint64_t destinationEnd = listEnd;
        if (plan.counts.n2 != 0)
        {
            if (cursor == 0)
                destinationEnd = plan.init_prefix_ends[3];
            else
                destinationEnd = listEnd +
                    48 * (static_cast<std::uint64_t>(plan.counts.n2) - 1) + 0x20;
        }
        if (destinationEnd > plan.destination_capacity)
            return next;
        next.disposition = NextDisposition::Call;
        next.cursor_before = static_cast<std::uint32_t>(cursor);
        next.expected_cursor_after = next.cursor_before + 1;
        next.destination_end = destinationEnd;
        return next;
    }

    inline bool ValidateNextAdvance(const IteratorExtentPlan& plan, const NextPlan& next,
        bool nativeReturned, std::uint64_t cursorAfter) noexcept
    {
        if (!plan.valid || next.disposition != NextDisposition::Call || !nativeReturned ||
            next.cursor_before < plan.initial_cursor ||
            next.cursor_before >= plan.row_count || next.cursor_before >= MaximumNativeCursor ||
            next.destination_end > plan.destination_capacity)
            return false;
        const auto expected = static_cast<std::uint64_t>(next.cursor_before) + 1;
        return expected == next.expected_cursor_after && cursorAfter == expected &&
            cursorAfter <= plan.row_count;
    }
}
