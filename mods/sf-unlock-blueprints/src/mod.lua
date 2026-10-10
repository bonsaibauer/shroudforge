-- This mod runs in the pregame asset pass, where EML exposes patch availability
-- through loader.features rather than requiring the in-game runtime namespace.
if not loader.features.patch then
    error("Unlock Blueprints requires the game.assets.write patch capability")
end

local recipe_type = game.types.get("keen::RecipeRegistryResource")
if recipe_type == nil then
    error("API type unavailable: keen::RecipeRegistryResource")
end
-- Use the engine's named query/action relation. The old numeric constant was
-- a knowledge ID for the first Flame base hint, not a universal unlock flag.
local query_type = assert(game.types.get("keen::GameKnowledgeQueryResourceDb"),
    "API type unavailable: keen::GameKnowledgeQueryResourceDb")
local knowledge_id
for _, resource in ipairs(game.assets.get_resources_by_type(query_type)) do
    for _, query in ipairs(resource.data.queries) do
        if query.name == "Unlock_Flame_Altar_PK" then
            for _, action in ipairs(query.actions) do
                if action.name == "NPC_Flame_Hint01" then
                    local id = action.query.knowledgeOrQueryId.value
                    if knowledge_id and knowledge_id ~= id then
                        error("Conflicting Flame hint knowledge IDs in this build")
                    end
                    knowledge_id = id
                end
            end
        end
    end
end
if not knowledge_id then
    error("Engine query Unlock_Flame_Altar_PK / NPC_Flame_Hint01 is missing")
end

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
    warn("Unlock Blueprints found no RecipeRegistryResource assets")
elseif changed_recipes == 0 then
    shroudforge.log.info("Unlock Blueprints found no recipes requiring changes in " .. #registries .. " registries")
else
    shroudforge.log.info("Unlock Blueprints updated " .. changed_recipes .. " recipes in " .. changed_registries .. " of " .. #registries .. " registries")
end
