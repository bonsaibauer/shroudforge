# Text and link buttons in the Modloader

This guide is for contributors who change the notices, translations, or link buttons shown in the Modloader. Players do not need to edit these files.

## Notices and news

ShroudForge shows short notices to explain a setting, describe a change, or share optional project support. A notice should only say that something happened when the loader can confirm it.

The notices are included in the loader when ShroudForge is built. They are not a separate file downloaded by players.

- **messages** contains notices that ship with ShroudForge.
- **templates** contains text for events, such as a mod update.
- **{name}**, **{version}**, and **{previousVersion}** are filled in when the notice appears.

The source and **news-schema.json** are in **modules/notifications/**. The schema is the rulebook for the shape of a valid news file.

Visible text is looked up in the Modloader language files:

- **modules/ui-resources/locales/en-GB.json** is the English (UK) source.
- **modules/ui-resources/locales/de.json** contains the German translation.

Add new text to the English (UK) file and provide its English (US) and German translations. **crowdin.yml** uses English (UK) as the source language.

## Saved notices and events

By default, the loader saves its state in **shroudforge/state.json**. The location can be changed in Settings → Storage locations:

- **events** contains notices from mods and mod-related actions.
- **news** remembers which notice IDs were read and when.
- **mods** stores the last observed mod versions so the loader can notice changes.

Runtime mods can publish their own local news notices with `shroudforge.notifications.publish`. A notice is written only when the mod calls the function. The API does not schedule, broadcast, throttle, or expire notices. Reusing the same mod-scoped ID replaces the notice in place. A new ID creates another local feed item. Read state is keyed by ID, so updating an already-read notice does not make it unread again. Mod authors should publish for meaningful events, never from per-frame callbacks, and should choose stable or versioned IDs based on whether a notice is an update or a separate release announcement. The [Lua API guide](../../modules/mod-runtime/README.md#publish-a-modloader-notice) has a full example and explains the fields, size limits, and ID behavior.

Under **Settings → General → Username**, the player can use automatic detection or choose a manual fallback. Automatic mode consumes the selected-character cache published by the existing runtime. The native main-menu callback reads the name on the first valid selection and when the selection or name descriptor changes. The cached value remains available while playing. If capture fails, the editable fallback defaults to `Flameborn`. No name from a previous game process is restored. Manual mode always uses the fallback. ShroudForge notices and mod-authored notices can use `{username}` in titles or messages. The Modloader replaces this token locally on the player's device.

A notice with **repeatEveryDays: 90** appears as unread again 90 days after it was last read. Without that setting, it appears once in the unread view. Read notices remain available in the full news list. The loader checks event data when it is saved and read. If a file is invalid, ShroudForge reports the problem rather than silently replacing it.

The Modloader also summarizes warning and error messages from the most recent 512 KiB of the loader log. Repeated copies are grouped together. A summary can open the Debug Console, and a known mod can be opened from its notice.

## Link buttons on a mod page

The Modloader can show buttons for a mod's project page, help page, or other links. Each button has a short name, an icon, and translated help text.

To add a button:

1. Add a definition file named after the new link ID.
2. Add that ID to **order.json** to choose where the button appears.
3. Add the matching SVG icon under **assets/**.
4. Add the translation text to the English and German language files.
5. Add a direct HTTPS link with the same ID to a mod's **extended.mod.json**.

For example, a mod with a **source** button can include:

~~~json
{
  "links": {
    "source": "https://github.com/example/my-mod"
  }
}
~~~

The definitions and icons are in **modules/modloader/ui/links/**. Their display order is in **modules/modloader/mod_link_order.json**. The translated button text is in the UI language files. Keep every link field name aligned with its ID in **order.json**.

## Updates

The updater uses the current update status and its event text. It installs the ShroudForge program after the game closes. News content is part of the program; the updater does not download a separate news file.
