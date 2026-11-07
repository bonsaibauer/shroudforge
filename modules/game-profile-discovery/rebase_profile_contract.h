#pragma once
#include "verify_image_equivalence.h"

namespace GameProfileDiscovery::Detail {
// Derives addresses from two original images. No target profile, game call,
// build-number table or externally supplied address map is accepted.
bool RebaseProfile(const Image& source, const Image& target,
                   const nlohmann::json& baseline, nlohmann::json& rebased,
                   nlohmann::json& evidence);
}
