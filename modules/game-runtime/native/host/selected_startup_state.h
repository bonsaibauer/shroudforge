#pragma once
#include "selected_startup.h"
#include <atomic>

namespace SelectedStartup {
// Fixed host operations. The bootstrap C ABI accepts no callback/function table.
struct Operations {
    void* context{};
    bool (*metadata)(void*, bool){};
    bool (*generic_ready)(void*){};
    bool (*map)(void*){};
    bool (*graphics)(void*){};
    bool (*stage_native_plugins)(void*){};
    bool (*native_plugins)(void*){};
    bool (*revalidate_registered_spans)(void*){};
    void (*stop)(void*){};
};
class State final {
    enum class Phase { Dormant, Preparing, Prepared, Activating, Active, Failed, Closed };
    std::atomic<Phase> phase_{Phase::Dormant};
    std::atomic<const char*> failure_{nullptr};
    const char* stage_{"metadata"};
    PlanV2 plan_{}; // Immutable after the sole Preparing transition.
    bool fail(const Operations& ops) noexcept {
        const char* empty = nullptr;
        failure_.compare_exchange_strong(empty, stage_);
        auto current = phase_.load();
        while (current != Phase::Closed && !phase_.compare_exchange_weak(current, Phase::Failed)) {}
        try { if (ops.stop) ops.stop(ops.context); } catch (...) {}
        return false;
    }
public:
    const char* FailureStage() const noexcept { const auto stage = failure_.load(); return stage ? stage : "none"; }
    bool Attempted() const noexcept { return phase_.load() != Phase::Dormant; }
    std::uint32_t ActiveServices() const noexcept { return phase_.load()==Phase::Active ? plan_.services : 0; }
    bool Prepare(const PlanV2& plan, const Operations& ops, std::uint32_t& completed) noexcept {
        completed = 0;
        if (!Valid(plan)) return false;
        auto expected = Phase::Dormant;
        if (!phase_.compare_exchange_strong(expected, Phase::Preparing)) return false;
        plan_ = plan;
        try {
            if (plan.services & NativePluginPreentry) {
                stage_ = "native-plugin-stage";
                if (!ops.stage_native_plugins || !ops.stage_native_plugins(ops.context)) return fail(ops);
            }
            if (!ops.metadata || !ops.metadata(ops.context, (plan.services & GenericHooks) != 0)) return fail(ops);
            stage_ = "generic";
            if ((plan.services & GenericHooks) && (!ops.generic_ready || !ops.generic_ready(ops.context))) return fail(ops);
            const auto run = [&](std::uint32_t bit, const char* name, bool (*call)(void*)) {
                if (!(plan.services & bit)) return true;
                stage_ = name;
                return call && call(ops.context);
            };
            if (!run(MapCapture, "map", ops.map) || !run(Graphics, "graphics", ops.graphics)) return fail(ops);
            if (plan.services & NativePluginPreentry) {
                stage_ = "native-plugins";
                if (!ops.native_plugins || !ops.native_plugins(ops.context)) return fail(ops);
            }
            stage_ = "prepare-transition";
            expected = Phase::Preparing;
            if (!phase_.compare_exchange_strong(expected, Phase::Prepared)) return fail(ops);
            completed = PreparedServices(plan);
            return true;
        } catch (...) { return fail(ops); }
    }
    bool Activate(const PlanV2& plan, const Operations& ops, std::uint32_t& completed) noexcept {
        completed = 0;
        auto expected = Phase::Prepared;
        if (!phase_.compare_exchange_strong(expected, Phase::Activating)) return false;
        // No native address/current configuration is reacquired at activation.
        stage_ = "activation-plan";
        if (!Same(plan_, plan)) return fail(ops);
        try {
            stage_ = "executable-span-revalidation";
            if (!ops.revalidate_registered_spans || !ops.revalidate_registered_spans(ops.context)) return fail(ops);
            stage_ = "activate-transition";
            expected = Phase::Activating;
            if (!phase_.compare_exchange_strong(expected, Phase::Active)) return fail(ops);
            completed = plan.services;
            return true;
        } catch (...) { return fail(ops); }
    }
    void Close() noexcept { phase_.store(Phase::Closed); }
};
}
