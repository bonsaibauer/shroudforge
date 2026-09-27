runtime.require("game.assets.write")

local recipe_type = game.types.get("keen::RecipeRegistryResource")
if recipe_type == nil then
    error("API type unavailable: keen::RecipeRegistryResource")
end
local knowledge_id = 1715248921

local changed_recipes = 0
local changed_registries = 0
local registries = game.assets.get_resources_by_type(recipe_type)

for _, resource in ipairs(registries) do
    local data = resource.data
    if data and data.recipes then
        local registry_changed = false
        for _, recipe in ipairs(data.recipes) do
            local requirement = recipe.knowledgeRequirement
            if requirement and requirement.knowledgeOrQueryId then
                local changed = false
                if requirement.knowledgeOrQueryId.value ~= knowledge_id then
                    requirement.knowledgeOrQueryId.value = knowledge_id
                    changed = true
                end
                if requirement.compareValue ~= 1 then requirement.compareValue = 1; changed = true end
                if requirement.compareOperator ~= "Equals" then requirement.compareOperator = "Equals"; changed = true end
                if requirement.type ~= "SimpleBool" then requirement.type = "SimpleBool"; changed = true end
                if requirement.isExplicitPlayerKnowledgeQuery ~= false then
                    requirement.isExplicitPlayerKnowledgeQuery = false
                    changed = true
                end
                if changed then
                    changed_recipes = changed_recipes + 1
                    registry_changed = true
                end
            end
        end
        if registry_changed then
            resource.data = data
            changed_registries = changed_registries + 1
        end
    end
end

if #registries == 0 then
    shroudforge.log.warn("Unlock Blueprints found no RecipeRegistryResource assets")
elseif changed_recipes == 0 then
    shroudforge.log.info("Unlock Blueprints found no recipes requiring changes in " .. #registries .. " registries")
else
    shroudforge.log.info("Updated " .. changed_recipes .. " recipes in " .. changed_registries .. " of " .. #registries .. " registries")
end
