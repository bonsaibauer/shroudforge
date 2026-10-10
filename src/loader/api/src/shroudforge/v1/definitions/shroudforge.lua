--- @meta

--- @class ShroudForgeSettings
--- Reads the current values declared by the package's `extended.mod.json` and player config.
--- Runtime mods should read values in lifecycle callbacks when changes must apply live.
shroudforge_settings = {}

--- @param key string
--- @param fallback? boolean|string|number|any[]
--- @return boolean|string|number|any[]|nil
function shroudforge_settings.get(key, fallback) end

--- @class ShroudForgeUi
--- Connects safe declarative UI actions to the current runtime mod.
shroudforge_ui = {}

--- @param action string Action identifier declared by a button in `extended.mod.json`.
--- @param callback fun()
function shroudforge_ui.on_action(action, callback) end

--- A message published to this player's local Modloader news feed.
--- Reusing the same ID from this mod updates the existing notice.
--- @class ShroudForgeNotification
--- @field id string Stable notice ID scoped to the publishing mod.
--- @field level? 'info'|'success'|'warning'|'error'|'update'
--- @field title string Title shown in the player's local news feed. May contain `{username}`.
--- @field message string Message shown in the player's local news feed. May contain `{username}`.
--- @field action_url? string HTTPS URL opened from the notice.

--- @class ShroudForgeNotifications
shroudforge_notifications = {}

--- Publish or update a notice in the player's Modloader news feed.
--- This does not distribute the notice to other players.
--- @param notice ShroudForgeNotification
function shroudforge_notifications.publish(notice) end

--- @class ShroudForgeInput
shroudforge_input = {}

--- Returns true while F1 through F8 is held and the game window is focused.
--- Poll from a runtime `on_update` callback and detect rising edges.
--- @param key 'F1'|'F2'|'F3'|'F4'|'F5'|'F6'|'F7'|'F8'
--- @return boolean
function shroudforge_input.is_key_down(key) end

--- @class ShroudForge
--- @field version string
--- @field mod_id string
--- @field mod_kind 'lua'
--- @field settings ShroudForgeSettings
--- @field ui ShroudForgeUi
--- @field input ShroudForgeInput
--- @field notifications ShroudForgeNotifications
shroudforge = {}
