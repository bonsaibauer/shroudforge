#pragma once
#include <cstdint>
namespace OverlayRuntime {
struct PresentSteps {
    void* context{};
    std::int32_t (*fence)(void*){};
    bool (*record)(void*){};
    std::int32_t (*reset)(void*){};
    std::int32_t (*submit)(void*){};
};
enum class PresentResult { Submitted, Busy, Failed, DeviceLost };
inline PresentResult SubmitPresent(const PresentSteps& steps) {
    const auto fence=steps.fence(steps.context);
    if(fence==1) return PresentResult::Busy;
    if(fence!=0) return fence==-4 ? PresentResult::DeviceLost : PresentResult::Failed;
    if(!steps.record(steps.context)) return PresentResult::Failed;
    const auto reset=steps.reset(steps.context);
    if(reset!=0) return reset==-4 ? PresentResult::DeviceLost : PresentResult::Failed;
    const auto submit=steps.submit(steps.context);
    return submit==0 ? PresentResult::Submitted : submit==-4 ? PresentResult::DeviceLost : PresentResult::Failed;
}
}
