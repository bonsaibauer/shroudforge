--- @meta

--- @class ShroudForgeSettings
--- Reads the current values declared by the package's `extended.mod.json` and player config.
--- Runtime mods should read values in lifecycle callbacks when changes must apply live.
shroudforge_settings = {}

--- Returns the current player-configured value for a setting declared in `extended.mod.json`.
--- Read it when a runtime callback or action runs so saved changes are picked up.
--- @param key string Setting key declared by the mod.
--- @param fallback? boolean|string|number|any[] Value to use when the key has no saved value.
--- @return boolean|string|number|any[]|nil
function shroudforge_settings.get(key, fallback) end

--- @class ShroudForgeUi
--- Connects safe declarative UI actions to the current runtime mod.
shroudforge_ui = {}

--- Registers a Lua callback for a button declared in `extended.mod.json`.
--- The callback runs when the player activates the matching button in the Modloader.
--- @param action string Action identifier declared by a button in `extended.mod.json`.
--- @param callback fun() Function to run after that button is activated.
function shroudforge_ui.on_action(action, callback) end

--- Requests the same Modloader UI toggle as the configured shortcut.
--- This signals the client UI process directly and does not synthesize a key press.
function shroudforge_ui.toggle_modloader() end

--- A message published to this player's local Modloader news feed.
--- Reusing the same ID from this mod updates the existing notice.
--- @class ShroudForgeNotification
--- @field id string Stable notice ID scoped to the publishing mod.
--- @field level? 'info'|'success'|'warning'|'error'|'update' Visual category used for the notice.
--- @field title string Title shown in the player's local news feed. May contain `{username}`.
--- @field message string Message shown in the player's local news feed. May contain `{username}`.
--- @field action_url? string HTTPS URL opened from the notice.

--- @class ShroudForgeNotifications
shroudforge_notifications = {}

--- Publish or update a notice in the player's Modloader news feed.
--- This does not distribute the notice to other players.
--- @param notice ShroudForgeNotification
function shroudforge_notifications.publish(notice) end

--- @class ShroudForgeI18n
--- Reads mod-authored text from the same `locales/<locale>.json` files that
--- localize the mod's Modloader name, settings, groups, actions and changelog.
shroudforge_i18n = {}

--- Returns this mod's translated string for the requested locale.
--- Regional catalogs take precedence over their base language. Missing keys
--- return `fallback`, or the key itself when no fallback is supplied.
--- Pass a value returned by `shroudforge.locale()` and cache it between
--- settings changes. The catalog is loaded with the mod, so lookup does not
--- read files or loader settings.
--- @param key string String key declared under `strings` in a mod locale file.
--- @param locale string Locale code, for example `"de"` or `"de-AT"`.
--- @param fallback? string Text to return when this mod has no translation.
--- @return string
function shroudforge_i18n.get(key, locale, fallback) end

--- @class ShroudForgeInput
shroudforge_input = {}

--- @deprecated Removed and unsupported in ShroudForge 1.7.6. Calling it logs a migration error and raises a Lua error.
--- Declare a `control: "keybind"` setting, assign it in Compatibility → Hotkeys, and use is_keybind_down or was_keybind_pressed.
--- @vararg any
function shroudforge_input.is_key_down(...) end

--- Returns true while either configured keyboard or mouse chord and its exact modifiers are held
--- and a window in the game process is focused. The setting must declare `control: "keybind"`
--- in `extended.mod.json`. Live saved changes reach the runtime on its one-second configuration refresh.
--- Other settings follow the existing mod-settings live/restart policy.
--- An empty binding or a false enabledWhen condition disables the shortcut.
--- Poll from `on_update` and detect rising edges.
--- @param setting_key string Key of this mod's declared keybind setting.
--- @return boolean
function shroudforge_input.is_keybind_down(setting_key) end

--- Returns true once when the configured shortcut was pressed since this mod last queried it.
--- Short taps between runtime updates are retained. The setting must declare control: "keybind".
--- An empty binding or a false enabledWhen condition disables the shortcut.
--- @param setting_key string Key of this mod's declared keybind setting.
--- @return boolean
function shroudforge_input.was_keybind_pressed(setting_key) end

--- Returns the current display label supplied by SDL, including modifiers and both alternatives.
--- One setting string stores a keyboard chord, a mouse chord, or keyboard|mouse,
--- for example `Ctrl+SDL:29|Mouse:4`. The editor shows separate columns for each device.
--- Returns nil when the shortcut is disabled.
--- @param setting_key string Key of this mod's declared keybind setting.
--- @return string|nil
function shroudforge_input.keybind_label(setting_key) end

--- Reports whether one of this mod's input contexts is active, such as "the hammer is in hand".
--- The contexts are declared in `extended.mod.json` under `inputReservations.contexts`. While a
--- context is active, the game does not receive the keybinds reserved for it
--- (`inputReservations.keys`, per device, padlocked by the player in the Modloader), nor the
--- mouse wheel of a reserved `inputReservations.wheels` setting. This mod still reads them:
--- keys with is_keybind_down and was_keybind_pressed, the wheel with wheel_steps.
--- An active context lasts one second: report it from every `on_update` while it holds, and
--- report `false` as soon as it ends. Escape, F9 and F10 always reach the game.
--- @param context string ID declared in `inputReservations.contexts`.
--- @param active boolean
function shroudforge_input.set_context(context, active) end

--- Returns the mouse wheel steps turned since this mod last asked, while the context of this
--- `inputReservations.wheels` setting was active (positive: away from the player), then zero.
--- The setting is a boolean: true keeps the wheel from the game meanwhile, false shares it.
--- @param setting_key string Key of this mod's boolean setting declared in `inputReservations.wheels`.
--- @return integer
function shroudforge_input.wheel_steps(setting_key) end

--- @class ShroudForge
--- @field version string Version of the ShroudForge Lua API.
--- @field mod_id string Stable ID of the currently running mod.
--- @field mod_kind 'lua' Runtime package kind for the currently running mod.
--- @field settings ShroudForgeSettings Read the current values from the mod's settings.
--- @field ui ShroudForgeUi Register Modloader button callbacks and request the UI toggle.
--- @field input ShroudForgeInput Read declared keyboard and mouse shortcuts using SDL.
--- @field notifications ShroudForgeNotifications Publish notices in the local Modloader feed.
--- @field i18n ShroudForgeI18n Read mod-authored text from the locale catalogs.
shroudforge = {}

--- Language the player chose for ShroudForge in the Modloader, such as `"en"`,
--- `"de"`, `"fr"` or a regional variant such as `"de-AT"`; `"en"` when the
--- setting cannot be read. Reads the settings file: call it when settings
--- change, not at every update.
--- @return string
function shroudforge.locale() end
