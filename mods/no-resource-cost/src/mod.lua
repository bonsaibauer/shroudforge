runtime.require("game.assets.write")

local recipe_registry_type = game.types.get("keen::RecipeRegistryResource")
if recipe_registry_type == nil then
    error("API type unavailable: keen::RecipeRegistryResource")
end

local changed_stacks = 0
local changed_recipe_fields = 0
local registries = game.assets.get_resources_by_type(recipe_registry_type)
local remove_item_costs = shroudforge.settings.get("removeItemCosts")
local remove_energy_costs = shroudforge.settings.get("removeEnergyCosts")
local remove_water_costs = shroudforge.settings.get("removeWaterCosts")

local function set_zero(object, field)
    if object[field] ~= nil and object[field] ~= 0 then
        object[field] = 0
        changed_recipe_fields = changed_recipe_fields + 1
    end
end

for _, resource in ipairs(registries) do
    local data = resource.data
    if data and data.recipes then
        for _, recipe in ipairs(data.recipes) do
            if remove_energy_costs then
                set_zero(recipe, "requiredEnergy")
            end
            if remove_water_costs then
                set_zero(recipe, "waterAmount")
                set_zero(recipe, "waterCharges")
            end

            if remove_item_costs and recipe.input then
                for _, input in ipairs(recipe.input) do
                    local stack = input.itemStack
                    if stack and stack.count ~= 0 then
                        stack.count = 0
                        changed_stacks = changed_stacks + 1
                    end
                end
            end
        end
        resource.data = data
    end
end

shroudforge.log.info(
    "Removed costs from "
        .. changed_stacks
        .. " recipe input stacks and "
        .. changed_recipe_fields
        .. " recipe fields in "
        .. #registries
        .. " registries"
)
