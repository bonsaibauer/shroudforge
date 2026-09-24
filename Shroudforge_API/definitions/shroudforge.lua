--- @meta

--- @class ShroudForgeSettings
--- Reads the current values declared by the current package in `mod.json`.
--- Runtime mods should read values in lifecycle callbacks when changes must apply live.
shroudforge_settings = {}

--- @param key string
--- @param fallback? boolean|string|number|any[]
--- @return boolean|string|number|any[]|nil
function shroudforge_settings.get(key, fallback) end

--- @class ShroudForgeUi
--- Connects safe declarative UI actions to the current runtime mod.
shroudforge_ui = {}

--- @param action string Action identifier declared by a button in `mod.json`.
--- @param callback fun()
function shroudforge_ui.on_action(action, callback) end

--- @class ShroudForgeNotification
--- @field id string
--- @field level? 'info'|'success'|'warning'|'error'|'update'
--- @field title string
--- @field message string
--- @field action_url? string HTTPS URL.

--- @class ShroudForgeNotifications
shroudforge_notifications = {}

--- @param notice ShroudForgeNotification
function shroudforge_notifications.publish(notice) end

--- @class ShroudForge
--- @field version string
--- @field mod_id string
--- @field mod_kind 'lua'
--- @field settings ShroudForgeSettings
--- @field ui ShroudForgeUi
--- @field notifications ShroudForgeNotifications
shroudforge = {}
