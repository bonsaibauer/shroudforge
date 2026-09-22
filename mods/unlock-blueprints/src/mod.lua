runtime.require("game.assets.write")

local recipe_type = game.types.get("keen::RecipeRegistryResource")
if recipe_type == nil then
    error("API type unavailable: keen::RecipeRegistryResource")
end
local knowledge_id = 1715248921

local changed_recipes = 0
local registries = game.assets.get_resources_by_type(recipe_type)

for _, resource in ipairs(registries) do
    local data = resource.data
    if data and data.recipes then
        for _, recipe in ipairs(data.recipes) do
            local requirement = recipe.knowledgeRequirement
            if requirement and requirement.knowledgeOrQueryId then
                requirement.knowledgeOrQueryId.value = knowledge_id
                requirement.compareValue = 1
                requirement.compareOperator = "Equals"
                requirement.type = "SimpleBool"
                requirement.isExplicitPlayerKnowledgeQuery = false
                changed_recipes = changed_recipes + 1
            end
        end
        resource.data = data
    end
end

shroudforge.log.info(
    "Updated " .. changed_recipes .. " recipes in " .. #registries .. " registries"
)
