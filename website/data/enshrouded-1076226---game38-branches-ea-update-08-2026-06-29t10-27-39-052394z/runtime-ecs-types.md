# Runtime ECS type reference

This is a generated filtered view of the public ShroudForge type API, not a second API.

Generated from `enshrouded-1076226---game38-branches-ea-update-08-2026-06-29t10-27-39-052394z`.

Game version: `1076226|^/game38/branches/ea_update_08|2026-06-29T10:27:39.052394Z`

| Kind | Count |
| --- | ---: |
| component | 644 |
| event | 127 |
| runtime-struct | 422 |
| runtime-type | 249 |
| total | 1442 |

The types below are already reachable through `game.types.get("keen::ecs::...")`.
Live runtime access still depends on a verified provider for `runtime.ecs.query/read/write`.

## Important runtime/gameplay types

| Type | Kind | Size | Fields |
| --- | --- | ---: | --- |
| `keen::ecs::EntityId` | runtime-struct | 4 | `id` |
| `keen::ecs::GameObjectId` | runtime-struct | 16 | `type`, `value` |
| `keen::ecs::CurrentTransform` | component | 56 | `transform` |
| `keen::ecs::ClientCursor` | component | 4328 | `blueprintHoverVfx`, `buildingOffsetDirection`, `displayCursor`, `effectivePlacementVolume`, `hasInsufficientEnergy`, `hasMissingEnergyEntity`, `highlightColorInvalid`, `highlightColorMuted`, `highlightColorValid`, `hoveredEntityHightlightPhase`, `hoveredVoxelMaterialId`, `isCurrentlySnappingToBoxes`, `lastBuildingActionTimeStamp`, `lastCombatActionTimeStamp`, `maxPlacementVolume`, `previousSelectedEntityId`, `previousSelectedEntityTintColor`, `primaryFlags`, `primaryTransform`, `queries`, `rotationGizmoFxHandle`, `secondaryFlags`, `secondaryTransform`, `selectedObjectDismantleHoverDelay`, `snapToPlaneDelay`, `translationGizmoFxHandle` |
| `keen::ecs::NetworkCursor` | component | 56 | `currentBuildingItemId`, `currentBuildingItemPIDE`, `cursorEntityId`, `hoveredObjectId`, `isInPropVariationSequence`, `randomYawAngleOffset`, `selectedObjectDismantleMethod`, `selectedObjectId`, `serverFlags` |
| `keen::ecs::BuildingPlaceEvent` | event | 72 | `material`, `orientation`, `ownerId`, `position`, `trackingItemId`, `volumeMax`, `volumeMin` |
| `keen::ecs::BuildingTearDownEvent` | event | 72 | `material`, `orientation`, `ownerId`, `position`, `volumeMax`, `volumeMin` |
| `keen::ecs::Flying` | component | 176 | `fallOnHit`, `fallOnHitCooldown`, `fallOnParryStun`, `flapSetup`, `flappingSequence`, `flyAfterSpawn`, `flyingSequence`, `hitInAirSequence`, `hoverSequence`, `maxAcceleration`, `startFlyingSequence`, `stopFlyingSequence`, `stuckOnGroundSequence`, `useFlyAnimationHandling` |
| `keen::ecs::DynamicFlying` | runtime-struct | 32 | `fallOnDeath`, `isFlappingAllowed`, `isSequenceHandlingPaused`, `nextCheckFlappingTime`, `nextFallOnHitTime`, `state`, `wasSequenceHandlingPaused` |
| `keen::ecs::EnterFlyingStateEvent` | event | 16 | `targetId` |
| `keen::ecs::StartFlyingEvent` | event | 16 | `targetId` |
| `keen::ecs::StopFlyingEvent` | event | 16 | `targetId` |
| `keen::ecs::Stamina` | component | 68 | `dataStorage`, `definition` |
| `keen::ecs::StaminaDepletion` | component | 4 | `accumulatedValue` |
| `keen::ecs::StaminaRecharge` | component | 16 |  |
| `keen::ecs::FallDamage` | component | 24 | `fallDamageLethalDistance`, `fallDamageSequence`, `fallDamageStartDistance` |
| `keen::ecs::DynamicFallDamage` | runtime-struct | 16 | `detectedFallDamagePercentage`, `detectedFallDistance`, `fallStartAltitude`, `resetFallAltitudeOnApex`, `wasFalling` |
| `keen::ecs::ItemUsed` | event | 16 | `itemId`, `playerEntityId` |
| `keen::ecs::Inventory` | component | 96 | `slots` |
| `keen::ecs::Crafting` | component | 20 | `workshop`, `workshopId` |
| `keen::ecs::PlayerCraftingAction` | runtime-struct | 68 | `amount`, `craftingStationId`, `inputCategorySelections`, `itemId`, `recipeId`, `type`, `useAllAvailableIngredientOptions`, `versionData`, `waterAmount`, `waterSourceId` |
| `keen::ecs::Durability` | component | 44 | `dataStorage`, `definition` |
