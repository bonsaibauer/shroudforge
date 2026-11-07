# Main page setup notes

1. Upload the four PNG files from assets/ with the exact filenames shown there.
2. Create the article and category pages supplied in this proposal.
3. Replace the full contents of Module:Mainpage/settings.json with Mainpage-settings.json.
4. Keep the existing Template:Mainpage transclusion on Main_Page.

The imported layout has a few labels and sections controlled by Lua rather than settings.json. In Module:Mainpage/Event/Legacy, change the fixed EVENT kicker to MOD SPOTLIGHT. In Module:Mainpage/OnThisDay, replace the real-life and lore date content with a modding feature or a rotating guide card. The settings file cannot change those module templates.

The community card and social links are handled by the ShroudForge Module:Mainpage/Community page. Keep its ShroudForge links. Skin colours and hero overlay styling are separate CSS work, so this configuration preserves the existing page layout while changing its content and navigation.