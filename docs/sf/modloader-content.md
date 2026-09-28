# Text and link buttons in the Modloader

This guide is for contributors who change the notices, translations, or link buttons shown in the Modloader. Players do not need to edit these files.

## Notices and news

ShroudForge shows short notices to explain a setting, describe a change, or share optional project support. A notice should only say that something happened when the loader can confirm it.

The notices are included in the loader when ShroudForge is built. They are not a separate file downloaded by players.

- **messages** contains notices that ship with ShroudForge.
- **templates** contains text for events, such as a mod update.
- **{name}**, **{version}**, and **{previousVersion}** are filled in when the notice appears.

The source and **news-schema.json** are in the loader package's news module. The schema is the rulebook for the shape of a valid news file.

Visible text is looked up in the Modloader language files:

- **src/loader/modules/modloader-ui/ui/src/locales/en.json** is the English source.
- **src/loader/modules/modloader-ui/ui/src/locales/de.json** contains the German translation.

Add new text to the English file and provide its German translation. **crowdin.yml** uses English as the source language.

## Saved notices and events

The loader saves its state in **shroudforge/state/state.json**:

- **events** contains notices from mods and mod-related actions.
- **news** remembers which notice IDs were read and when.
- **mods** stores the last observed mod versions so the loader can notice changes.

A notice with **repeatEveryDays: 90** appears as unread again 90 days after it was last read. Without that setting, it appears once. The loader checks event data when it is saved and read. If a file is invalid, ShroudForge reports the problem rather than silently replacing it.

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

The definitions, order, and icons are in **src/loader/modules/modloader-ui/ui/src/links/**. The translated button text is in the UI language files. Keep every link field name aligned with its ID in **order.json**.

## Updates

The updater uses the current update status and its event text. It installs the ShroudForge program after the game closes. News content is part of the program; the updater does not download a separate news file.
