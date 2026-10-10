-- Asset preparation uses reflected engine fields; no EXE address or recipe ID.
assert(loader.features.patch, "Production Time requires the patch capability")
local seconds = shroudforge.settings.get("seconds", 1)
assert(type(seconds) == "number" and seconds == seconds and seconds >= 0.1 and seconds <= 3600,
    "Production Time seconds must be between 0.1 and 3600")
local duration = math.floor(seconds * 1000000000 + 0.5) -- keen::Time is nanoseconds.
local recipe_type = assert(game.types.get("keen::RecipeRegistryResource"),
    "API type unavailable: keen::RecipeRegistryResource")
local resources = game.assets.get_resources_by_type(recipe_type)
assert(#resources > 0, "Production Time found no recipe registries")

-- Validate every resource before queuing writes; assign an absolute duration.
-- This stays idempotent even when an existing backup already contains fast recipes.
local pending, changed = {}, 0
for _, resource in ipairs(resources) do
    local data = resource.data
    assert(data and data.recipes, "Recipe registry has no recipes")
    local dirty = false
    for _, recipe in ipairs(data.recipes) do
        local time = recipe.craftingDuration
        assert(time and type(time.value) == "number" and time.value == time.value and time.value >= 0
            and time.value < math.huge, "Invalid recipe craftingDuration")
        if time.value > 0 and time.value ~= duration then
            time.value = duration
            changed, dirty = changed + 1, true
        end
    end
    if dirty then pending[#pending + 1] = {resource = resource, data = data} end
end
for _, entry in ipairs(pending) do entry.resource.data = entry.data end
shroudforge.log.info("Production Time: updated " .. changed .. " timed recipes to " .. seconds .. " base seconds")
