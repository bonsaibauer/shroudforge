import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const pagesRoot = path.join(root, "website", "pages", "modding");
const packageRoot = path.join(root, "src", "loader", "package", "src", "registry");
const manifestSchema = readJson(path.join(packageRoot, "manifest.schema.json"));
const extensionSchema = readJson(path.join(packageRoot, "extended.mod.schema.json"));
const loaderSchema = readJson(path.join(root, "src", "loader", "package", "src", "config", "loader.schema.json"));
const loaderDefaults = readJson(path.join(root, "src", "loader", "package", "src", "config", "loader.default.json"));
const overwriteSeededPages = process.argv.includes("--overwrite");

const pages = [];
const slug = value => String(value).normalize("NFKD").replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
const titleFromKey = key => key.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replaceAll("_", " ").replace(/^./, value => value.toUpperCase());
const code = value => `\x60${String(value)}\x60`;

function addPage({ id, folder, navGroup, navOrder = 100, order = 100, titleDe, titleEn, summaryDe, summaryEn, bodyDe, bodyEn, apiSymbols, apiPrefixes }) {
  const page = {
    id,
    title: { de: titleDe, en: titleEn },
    summary: { de: summaryDe, en: summaryEn },
    navGroup: { de: navGroup.de, en: navGroup.en },
    navOrder,
    order,
  };
  if (apiSymbols?.length) page.apiSymbols = apiSymbols;
  if (apiPrefixes?.length) page.apiPrefixes = apiPrefixes;
  const directory = path.join(pagesRoot, folder);
  fs.mkdirSync(directory, { recursive: true });
  writeSeedFile(path.join(directory, "page.json"), `${JSON.stringify(page, null, 2)}\n`);
  writeSeedFile(path.join(directory, "de.md"), bodyDe.trim() + "\n");
  writeSeedFile(path.join(directory, "en.md"), bodyEn.trim() + "\n");
  pages.push(page);
}

function writeSeedFile(file, content) {
  if (overwriteSeededPages || !fs.existsSync(file)) fs.writeFileSync(file, content);
}

function articleIntro(title, summary) {
  return `# ${title}\n\n${summary}\n`;
}

const core = [
  {
    id: "mod-json-guide", folder: "mod-json", navGroup: { de: "mod.json", en: "mod.json" }, navOrder: 10, order: 0,
    titleDe: "mod.json, der Steckbrief deines Mods", titleEn: "mod.json, your mod's information card",
    summaryDe: "Diese Datei nennt die Mod-ID, den Namen, die Version und die Zugriffsrechte. Sie gehört in den Hauptordner deines Mods.",
    summaryEn: "This file declares the mod ID, name, version, and requested capabilities. It belongs in the root folder of your mod.",
    bodyDe: `# mod.json, der Steckbrief deines Mods\n\nJeder Mod braucht eine \x60mod.json\x60. ShroudForge liest sie, bevor der Mod vorbereitet oder gestartet wird. Die Datei beschreibt den Mod und seine angeforderten Funktionen.\n\n## Pflichtfelder\n\n\x60id\x60, \x60name\x60 und \x60version\x60 sind erforderlich. Die anderen Felder sind freiwillig. Jede Eigenschaft hat eine eigene Seite in dieser Gruppe.\n\n## Welche Datei wird hier beschrieben?\n\nDas kanonische Schema liegt in [manifest.schema.json](../../../schemas/manifest.schema.json). Der Loader prüft zusätzlich Abhängigkeiten und Laufzeitbedingungen in [manifest_reader.rs](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest_reader.rs).\n\n## Kleines Beispiel\n\n\x60\x60\x60json\n{\n  \"id\": \"example.hello-ember\",\n  \"name\": \"Hello Ember\",\n  \"version\": \"1.0.0\",\n  \"authors\": [\"Your Name\"],\n  \"capabilities\": [\"runtime\"],\n  \"dependencies\": []\n}\n\x60\x60\x60\n\n## Zugehörige Themen\n\n- [Die optionale extended.mod.json](#doc-extended-mod-json-guide) enthält Einstellungen und Modloader-Aktionen.\n- [Einen Mod Schritt für Schritt bauen](#first) zeigt ein vollständiges kleines Paket.\n- [EML-Mods verstehen und migrieren](#doc-eml-migration) erklärt Herkunft und Unterschiede.`,
    bodyEn: `# mod.json, your mod's information card\n\nEvery mod needs a \x60mod.json\x60. ShroudForge reads it before preparing or starting a mod. The file describes the mod and the capabilities it requests.\n\n## Required fields\n\n\x60id\x60, \x60name\x60, and \x60version\x60 are required. Every other field is optional. Each property has its own page in this group.\n\n## The file described here\n\nThe canonical schema is [manifest.schema.json](../../../schemas/manifest.schema.json). The loader also checks dependencies and runtime conditions in [manifest_reader.rs](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest_reader.rs).\n\n## A small example\n\n\x60\x60\x60json\n{\n  \"id\": \"example.hello-ember\",\n  \"name\": \"Hello Ember\",\n  \"version\": \"1.0.0\",\n  \"authors\": [\"Your Name\"],\n  \"capabilities\": [\"runtime\"],\n  \"dependencies\": []\n}\n\x60\x60\x60\n\n## Related topics\n\n- [The optional extended.mod.json](#doc-extended-mod-json-guide) holds settings and Modloader actions.\n- [Build a mod step by step](#first) walks through a complete small package.\n- [Understand and migrate EML mods](#doc-eml-migration) explains provenance and differences.`,
  },
  {
    id: "extended-mod-json-guide", folder: "extended-mod-json", navGroup: { de: "extended.mod.json", en: "extended.mod.json" }, navOrder: 20, order: 0,
    titleDe: "extended.mod.json, Einstellungen und Modloader-Seite", titleEn: "extended.mod.json, settings and the Modloader page",
    summaryDe: "Diese optionale Datei steuert Modloader-Einstellungen, Gruppen, Aktionen, Links, Zielprozesse und den gespeicherten Aktivierungszustand.",
    summaryEn: "This optional file describes Modloader settings, groups, actions, links, target processes, and the saved enabled state.",
    bodyDe: `# extended.mod.json\n\n\x60extended.mod.json\x60 ergänzt den Mod-Steckbrief. Sie wird verwendet, wenn dein Mod eigene Werte im Modloader anzeigen oder seinen Aktivierungszustand speichern soll. Jedes Feld hat eine eigene Seite in dieser Navigation.\n\n## Das Wichtigste\n\n- \x60schemaVersion\x60 ist derzeit \x601\x60.\n- \x60enabled\x60 speichert, ob der Mod eingeschaltet ist.\n- \x60targets\x60 legt fest, in welchem Prozess der Mod geladen wird. Das ist keine Netzwerk-Replikation.\n- \x60settings\x60 enthält Mod-eigene Werte.\n- \x60groups\x60 ordnet Werte in der Modloader-Oberfläche. Nicht gruppierte Einstellungen erscheinen ebenfalls, sie stehen unter einer Standardgruppe.\n- \x60launcher\x60 kennzeichnet die Herkunft eines übernommenen EML-Mods.\n\n## Beispiel\n\n\x60\x60\x60json\n{\n  \"$schema\": \"https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json\",\n  \"schemaVersion\": 1,\n  \"enabled\": false,\n  \"targets\": [\"client\"],\n  \"settings\": {\n    \"greeting\": {\n      \"value\": \"Hello from my mod\",\n      \"label\": \"Greeting\",\n      \"control\": \"text\"\n    }\n  }\n}\n\x60\x60\x60\n\nDie Mod-Einstellung \x60greeting\x60 wird in der Lua-API mit genau diesem Schlüssel gelesen. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls).\n\n## Quelle der Regeln\n\n[extended.mod.schema.json](../../../schemas/extended.mod.schema.json) definiert Form und Datentypen. Der Paketleser prüft zusätzlich, ob Gruppen auf vorhandene Einstellungen zeigen und ob Auswahlwerte gültig sind.\n\n[Mod-Einstellungen live ausprobieren](#manifests).`,
    bodyEn: `# extended.mod.json\n\n\x60extended.mod.json\x60 extends the mod information card. Use it when your mod needs player-facing values in Modloader or needs to save its enabled state. Each field has its own page in this navigation.\n\n## The essentials\n\n- \x60schemaVersion\x60 is currently \x601\x60.\n- \x60enabled\x60 stores whether the mod is switched on.\n- \x60targets\x60 selects the process that loads the mod. It does not replicate changes over the network.\n- \x60settings\x60 stores mod-specific values.\n- \x60groups\x60 organizes values in the Modloader. Ungrouped settings also appear, under a default group.\n- \x60launcher\x60 preserves the origin of an imported EML mod.\n\n## Example\n\n\x60\x60\x60json\n{\n  \"$schema\": \"https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json\",\n  \"schemaVersion\": 1,\n  \"enabled\": false,\n  \"targets\": [\"client\"],\n  \"settings\": {\n    \"greeting\": {\n      \"value\": \"Hello from my mod\",\n      \"label\": \"Greeting\",\n      \"control\": \"text\"\n    }\n  }\n}\n\x60\x60\x60\n\nThe mod setting \x60greeting\x60 is read in Lua by using that exact key. See [Settings and controls](#doc-setting-controls).\n\n## Source of the rules\n\n[extended.mod.schema.json](../../../schemas/extended.mod.schema.json) defines the structure and data types. The package reader also checks that groups point to existing settings and choice values are valid.\n\n[Try the live mod settings builder](#manifests).`,
  },
  {
    id: "setting-controls", folder: "extended-mod-json/settings-controls", navGroup: { de: "Einstellungen", en: "Settings" }, navOrder: 30, order: 0,
    titleDe: "Einstellungen und Steuerelemente", titleEn: "Settings and controls",
    summaryDe: "Jede Einstellung hat einen Schlüssel und einen Startwert. Metadaten bestimmen Beschriftung, Hilfe, Eingabefeld, Grenzen und Auswahlwerte.",
    summaryEn: "Each setting has a key and starting value. Metadata controls its label, help text, input, limits, and choices.",
    bodyDe: `# Einstellungen und Steuerelemente\n\nEin Eintrag unter \x60settings\x60 hat einen mod-eigenen Schlüssel. Dieser Schlüssel ist die technische Verbindung zwischen \x60extended.mod.json\x60 und deinem Lua-Code.\n\n## Einfache und ausführliche Werte\n\nEin boolescher Wert, Text, Zahl oder eine Liste kann direkt als Wert stehen. Die Objektform ergänzt Metadaten:\n\n\x60\x60\x60json\n{\n  \"settings\": {\n    \"flightSpeed\": {\n      \"value\": 1.0,\n      \"label\": \"Fluggeschwindigkeit\",\n      \"description\": \"Wie schnell sich der Charakter bewegt.\",\n      \"control\": \"slider\",\n      \"min\": 0.2,\n      \"max\": 3.0,\n      \"step\": 0.1\n    }\n  }\n}\n\x60\x60\x60\n\n## Alle zwölf Steuerelemente\n\nDie einzelnen Seiten erklären wann ein Steuerelement passt und welche Werte es erwartet. Boolean zeigt ohne Auswahl meist einen Schalter, Text einen Textbereich und Zahlen ein Zahlenfeld. Mit \x60control\x60 kannst du die Darstellung ausdrücklich wählen.\n\n- Umschalten: [toggle](#doc-control-toggle) und [checkbox](#doc-control-checkbox).\n- Text: [text](#doc-control-text) und [textarea](#doc-control-textarea).\n- Zahlen: [number](#doc-control-number) und [slider](#doc-control-slider).\n- Auswahl: [select](#doc-control-select), [radio](#doc-control-radio), [segmented](#doc-control-segmented) und [multiselect](#doc-control-multiselect).\n- Spezialfelder: [keybind](#doc-control-keybind) und [color](#doc-control-color).\n\n## Werte in Lua verwenden\n\n\x60shroudforge.settings.get("flightSpeed", 1.0)\x60 liest den aktuellen gespeicherten Wert. Den genauen Schlüssel und einen Fallback desselben Typs angeben. Lies ihn in der Callback-Funktion, wenn Änderungen während der Sitzung berücksichtigt werden sollen. Siehe [Einstellungen in Lua lesen](#doc-api-settings).\n\n## Gruppen sind optional\n\nMit \x60groups[].settings\x60 legst du die Sortierung fest. Ein nicht aufgeführter Wert wird in der Standardgruppe angezeigt, nicht ausgeblendet. Der Modloader speichert Änderungen im selben \x60extended.mod.json\x60.`,
    bodyEn: `# Settings and controls\n\nAn entry under \x60settings\x60 has a mod-specific key. That key connects \x60extended.mod.json\x60 to your Lua code.\n\n## Simple and detailed values\n\nA boolean, string, number, or list can be written as a direct value. The object form adds metadata:\n\n\x60\x60\x60json\n{\n  \"settings\": {\n    \"flightSpeed\": {\n      \"value\": 1.0,\n      \"label\": \"Flight speed\",\n      \"description\": \"How quickly the character moves.\",\n      \"control\": \"slider\",\n      \"min\": 0.2,\n      \"max\": 3.0,\n      \"step\": 0.1\n    }\n  }\n}\n\x60\x60\x60\n\n## All twelve controls\n\nThe individual pages explain when a control fits and which values it expects. A boolean usually becomes a toggle, text becomes a text field, and numbers become a number field. Set \x60control\x60 to choose a specific presentation.\n\n- Toggles: [toggle](#doc-control-toggle) and [checkbox](#doc-control-checkbox).\n- Text: [text](#doc-control-text) and [textarea](#doc-control-textarea).\n- Numbers: [number](#doc-control-number) and [slider](#doc-control-slider).\n- Choices: [select](#doc-control-select), [radio](#doc-control-radio), [segmented](#doc-control-segmented), and [multiselect](#doc-control-multiselect).\n- Special inputs: [keybind](#doc-control-keybind) and [color](#doc-control-color).\n\n## Read values in Lua\n\n\x60shroudforge.settings.get("flightSpeed", 1.0)\x60 reads the current saved value. Use the exact key and a fallback with the same type. Read it inside a callback when the current value should reflect changes during the session. See [Read settings in Lua](#doc-api-settings).\n\n## Groups are optional\n\nUse \x60groups[].settings\x60 to choose the order. A setting not listed there appears in the default group, it is not hidden. Modloader saves changes into the same \x60extended.mod.json\x60.`,
  },
  {
    id: "setting-groups-actions", folder: "extended-mod-json/groups-actions", navGroup: { de: "Einstellungen", en: "Settings" }, navOrder: 30, order: 1,
    titleDe: "Gruppen und Aktionsknöpfe", titleEn: "Groups and action buttons",
    summaryDe: "Gruppen ordnen Mod-Einstellungen. Aktionsknöpfe rufen eine benannte Funktion deines Lua-Mods auf.",
    summaryEn: "Groups organize mod settings. Action buttons call a named function in your Lua mod.",
    bodyDe: `# Gruppen und Aktionsknöpfe\n\nEine Gruppe hat eine sichtbare Beschriftung und kann Einstellungsschlüssel sowie Aktionen enthalten. Gruppen ändern nicht den Zugriff des Mods.\n\n## Einstellungen zuordnen\n\n\x60groups[].settings\x60 enthält die Schlüssel aus \x60settings\x60. Jeder Schlüssel darf nur einmal gruppiert werden. Nicht gruppierte Werte zeigt der Modloader in einer Standardgruppe.\n\n## Aktion mit Lua verbinden\n\nDer Schlüssel \x60groups[].actions[].id\x60 muss genau dem Namen entsprechen, den dein Mod an \x60shroudforge.ui.on_action\x60 übergibt. Beispiel:\n\n\x60\x60\x60json\n{\n  \"groups\": [{\n    \"label\": \"Werkzeuge\",\n    \"settings\": [\"enabledFeature\"],\n    \"actions\": [{\"id\": \"resetFeature\", \"label\": \"Zurücksetzen\", \"style\": \"secondary\"}]\n  }]\n}\n\x60\x60\x60\n\n\x60\x60\x60lua\nshroudforge.ui.on_action("resetFeature", function()\n  shroudforge.log.info("Feature settings reset")\nend)\n\x60\x60\x60\n\nDie Mod benötigt \x60runtime\x60 in \x60mod.json\x60. \x60style\x60 ändert nur die Darstellung. \x60confirm\x60 zeigt vor dem Aufruf eine Rückfrage. Siehe [UI-Aktionen in Lua](#doc-api-ui-actions).`,
    bodyEn: `# Groups and action buttons\n\nA group has a visible label and can contain setting keys and actions. Groups do not change the mod's access permissions.\n\n## Assign settings\n\n\x60groups[].settings\x60 contains keys from \x60settings\x60. A key may only be grouped once. The Modloader shows ungrouped values in a default group.\n\n## Connect an action to Lua\n\nThe key \x60groups[].actions[].id\x60 must exactly match the name passed to \x60shroudforge.ui.on_action\x60. Example:\n\n\x60\x60\x60json\n{\n  \"groups\": [{\n    \"label\": \"Tools\",\n    \"settings\": [\"enabledFeature\"],\n    \"actions\": [{\"id\": \"resetFeature\", \"label\": \"Reset\", \"style\": \"secondary\"}]\n  }]\n}\n\x60\x60\x60\n\n\x60\x60\x60lua\nshroudforge.ui.on_action("resetFeature", function()\n  shroudforge.log.info("Feature settings reset")\nend)\n\x60\x60\x60\n\nThe mod needs \x60runtime\x60 in \x60mod.json\x60. \x60style\x60 only changes the appearance. \x60confirm\x60 asks the player before calling the action. See [UI actions in Lua](#doc-api-ui-actions).`,
  },
  {
    id: "eml-migration", folder: "eml-migration", navGroup: { de: "EML verstehen und migrieren", en: "Understand and migrate EML" }, navOrder: 40, order: 0,
    titleDe: "EML-Mods verstehen und migrieren", titleEn: "Understand and migrate EML mods",
    summaryDe: "ShroudForge kann EML-Pakete laden. Diese Seite trennt alte Manifestdaten von ShroudForge-Erweiterungen und erklärt die wichtigen Unterschiede.",
    summaryEn: "ShroudForge can load EML packages. This page separates legacy manifest data from ShroudForge extensions and explains the differences that matter.",
    bodyDe: `# EML-Mods verstehen und migrieren\n\n\x60mod.json\x60 basiert auf dem EML-Manifestformat. ShroudForge nutzt weiterhin dessen Mod-ID, Name, Version, Abhängigkeiten und Berechtigungsangaben. Zusätzliche ShroudForge-Werte stehen in \x60extended.mod.json\x60.\n\n## Herkunft und Launcher-Badge\n\nEin EML-Mod ohne Erweiterungsdatei wird als älteres EML-Paket behandelt. Wenn ShroudForge beim Speichern eine Erweiterung anlegt, setzt es \x60launcher: "EML"\x60, damit die Herkunft erhalten bleibt. Für ShroudForge-Mods kann \x60launcher\x60 fehlen.\n\n## Client und Server\n\nEML-Pakete ohne explizites \x60targets\x60 laufen standardmäßig auf Client und Server, auch wenn die Erweiterungsdatei \x60launcher: "EML"\x60 enthält. Für neue ShroudForge-Pakete ist der Standard Client. Schreibe \x60targets\x60 ausdrücklich, wenn ein anderer Prozess gebraucht wird. Zielprozesse garantieren keine Netzwerkübertragung. Siehe [targets](#doc-targets).\n\n## Berechtigungen\n\nPrüfe, welche Fähigkeiten der Mod tatsächlich verwendet. \x60runtime-register-dll\x60 bezieht sich auf eine DLL aus dem Mod-Paket und ist nicht die eingebaute ShroudForge \x60kfc-runtime.dll\x60. Die Berechtigung plant die EML-DLL-Registrierung für die Laufzeit ein.\n\n## Sichere Migration\n\n1. Sichere den Mod-Ordner.\n2. Lass \x60mod.json\x60 unverändert, sofern keine belegte Anpassung nötig ist.\n3. Ergänze die optionale \x60extended.mod.json\x60 für ShroudForge-Einstellungen.\n4. Setze \x60launcher: "EML"\x60 und das gewünschte \x60targets\x60 ausdrücklich.\n5. Prüfe den Mod im Modloader und im passenden Client- oder Serverlog.\n\nEine erfolgreiche Paketprüfung bedeutet nicht automatisch, dass jede EML-Funktion oder jede native Operation mit jedem Spielbuild kompatibel ist. Die [durchsuchbare API-Referenz](#api) unterscheidet EML- und ShroudForge-Funktionen.`,
    bodyEn: `# Understand and migrate EML mods\n\n\x60mod.json\x60 is based on the EML manifest format. ShroudForge continues to use its mod ID, name, version, dependencies, and capability declarations. ShroudForge-specific values belong in \x60extended.mod.json\x60.\n\n## Origin and launcher badge\n\nAn EML package without an extension file is treated as a legacy EML package. When ShroudForge creates an extension while saving it, it writes \x60launcher: "EML"\x60 to preserve that origin. ShroudForge mods may omit \x60launcher\x60.\n\n## Client and server\n\nEML packages without explicit \x60targets\x60 default to client and server, including packages whose extension declares \x60launcher: "EML"\x60. New ShroudForge packages default to client. Set \x60targets\x60 explicitly when another process is needed. Process targets do not guarantee network replication. See [targets](#doc-targets).\n\n## Capabilities\n\nCheck which capabilities the mod actually uses. \x60runtime-register-dll\x60 refers to a DLL included in the mod package, not ShroudForge's built-in \x60kfc-runtime.dll\x60. It schedules EML DLL registration for the runtime phase.\n\n## A cautious migration\n\n1. Back up the mod folder.\n2. Leave \x60mod.json\x60 unchanged unless a verified adjustment is needed.\n3. Add the optional \x60extended.mod.json\x60 for ShroudForge settings.\n4. Set \x60launcher: "EML"\x60 and the intended \x60targets\x60 explicitly.\n5. Check the mod in Modloader and in the matching client or server log.\n\nA valid package does not automatically mean every EML feature or native operation is compatible with every game build. The [searchable API reference](#api) distinguishes EML and ShroudForge functions.`,
  },
  {
    id: "api-guides", folder: "lua-api", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 0,
    titleDe: "Lua-API, Anleitungen nach Funktion", titleEn: "Lua API, guides by feature",
    summaryDe: "Die API-Referenz listet alle verfügbaren Symbole. Diese Anleitungen erklären häufige Aufgaben mit Ablauf, Voraussetzungen und vollständigen Beispielen.",
    summaryEn: "The API reference lists every available symbol. These guides explain common tasks with their flow, prerequisites, and complete examples.",
    bodyDe: `# Lua-API, Anleitungen nach Funktion\n\nDie [durchsuchbare API-Referenz](#api) ist das vollständige Verzeichnis der implementierten Funktionen und Typen. Diese Anleitungen zeigen typische Aufgaben mit Kontext und Beispielcode.\n\n## Häufige Aufgaben\n\n- [Modloader-Mitteilungen veröffentlichen](#doc-api-notifications) erklärt \x60shroudforge.notifications.publish\x60 und wie Nachrichten im Modloader erscheinen.\n- [Mod-Einstellungen lesen](#doc-api-settings) verbindet \x60extended.mod.json\x60 mit \x60shroudforge.settings.get\x60.\n- [Modloader-Aktionen verbinden](#doc-api-ui-actions) verbindet die Aktions-ID mit \x60shroudforge.ui.on_action\x60.\n- [Meldungen schreiben](#doc-api-logging) erklärt Log-Level und sinnvolle Meldungen.\n- [Dateien und Exporte](#doc-api-files) beschreibt Paketdateien und Exportberechtigungen.\n- [Laufzeit und Spielfunktionen](#doc-api-runtime) beschreibt Lifecycle, Client/Server und native Unterstützung.\n\n## So sind API-Artikel aufgebaut\n\nJede Anleitung nennt Zweck, passende Situation, erforderliche Fähigkeiten, erwartete Argumente und Ergebnisse. Für alle übrigen Funktionen nutze die Suchfunktion der API-Referenz, sie verlinkt auf den Lua-Quellvertrag. API-Funktionen können EML oder ShroudForge-spezifisch sein.`,
    bodyEn: `# Lua API, guides by feature\n\nThe [searchable API reference](#api) is the complete index of implemented functions and types. These guides explain common tasks with context and working examples.\n\n## Common tasks\n\n- [Publish Modloader notices](#doc-api-notifications) explains \x60shroudforge.notifications.publish\x60 and how notices appear in the Modloader.\n- [Read mod settings](#doc-api-settings) connects \x60extended.mod.json\x60 to \x60shroudforge.settings.get\x60.\n- [Connect Modloader actions](#doc-api-ui-actions) connects an action ID to \x60shroudforge.ui.on_action\x60.\n- [Write log messages](#doc-api-logging) explains levels and useful messages.\n- [Files and exports](#doc-api-files) explains package files and export capabilities.\n- [Runtime and game features](#doc-api-runtime) explains lifecycle, client/server targets, and native support.\n\n## What each API article covers\n\nEach guide explains purpose, when to use it, required capabilities, arguments, and results. Use the API search for the remaining functions, it links back to the Lua source contract. API functions may be EML-specific or ShroudForge-specific.`,
  },
  {
    id: "api-notifications", folder: "lua-api/modloader-notifications", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 1,
    titleDe: "Modloader-Mitteilungen veröffentlichen", titleEn: "Publish Modloader notices",
    summaryDe: "Mitteilungen zeigen Spielern wichtige Mod-Ereignisse. Sie werden lokal gespeichert und nur veröffentlicht, wenn dein Mod die Funktion aufruft.",
    summaryEn: "Notices tell players about meaningful mod events. They are stored locally and are published only when your mod calls the function.",
    bodyDe: `# Modloader-Mitteilungen veröffentlichen\n\n\x60shroudforge.notifications.publish\x60 fügt eine mod-eigene Mitteilung in den lokalen Modloader-Feed ein. Sie wird nicht an andere Spieler gesendet. Der Mod muss die Funktion gezielt aufrufen, zum Beispiel nach einer wichtigen Änderung oder einem abgeschlossenen Schritt.\n\n## Beispiel\n\n\x60\x60\x60lua\nshroudforge.notifications.publish({\n  id = "world-ready",\n  title = "World is ready",\n  message = "Your world data has been checked.",\n  level = "info"\n})\n\x60\x60\x60\n\nDie konkrete Nutzlast und erlaubten Level stehen in der [API-Referenz](#api). Der Mod benötigt die passende Laufzeitfunktion und \x60runtime\x60 in \x60mod.json\x60.\n\n## IDs und Wiederholung\n\nDie ID ist im jeweiligen Mod-Namensraum stabil. Dieselbe ID aktualisiert die vorhandene Mitteilung, eine neue ID erzeugt einen weiteren Eintrag. Der Feed wird lokal im konfigurierten State-Speicher abgelegt. Die Funktion terminiert, wiederholt oder verteilt Mitteilungen nicht automatisch.\n\n## Wann eine Mitteilung passt\n\nMelde bestätigte Ereignisse. Rufe die Funktion nicht pro Frame oder in schnellen Schleifen auf. Für technische Diagnosen nutze das [Mod-Logging](#doc-api-logging). Details zum gespeicherten Format stehen in [modloader-content.md](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/sf/modloader-content.md).`,
    bodyEn: `# Publish Modloader notices\n\n\x60shroudforge.notifications.publish\x60 adds a mod-authored notice to the local Modloader feed. It is not sent to other players. Call it deliberately, for example after a meaningful change or a completed step.\n\n## Example\n\n\x60\x60\x60lua\nshroudforge.notifications.publish({\n  id = "world-ready",\n  title = "World is ready",\n  message = "Your world data has been checked.",\n  level = "info"\n})\n\x60\x60\x60\n\nThe exact payload and accepted levels are in the [API reference](#api). The mod needs the matching runtime feature and \x60runtime\x60 in \x60mod.json\x60.\n\n## IDs and repeated notices\n\nThe ID is stable within the mod's namespace. Reusing it updates the existing notice, while a new ID creates another entry. The feed is stored locally in the configured state location. The function does not schedule, repeat, or distribute notices automatically.\n\n## When to publish\n\nReport confirmed events. Do not call it per frame or in a tight loop. Use [mod logging](#doc-api-logging) for technical diagnostics. The stored format is described in [modloader-content.md](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/sf/modloader-content.md).`,
  },
  {
    id: "api-settings", folder: "lua-api/settings", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 2,
    titleDe: "Mod-Einstellungen in Lua lesen", titleEn: "Read mod settings in Lua",
    summaryDe: "Der Schlüssel in extended.mod.json ist derselbe Schlüssel, den dein Lua-Code an shroudforge.settings.get übergibt.",
    summaryEn: "The key in extended.mod.json is the same key your Lua code passes to shroudforge.settings.get.",
    bodyDe: `# Mod-Einstellungen in Lua lesen\n\nDie Beschreibung und der Startwert stehen in \x60extended.mod.json\x60. Der Lua-Code liest den aktuellen Wert mit \x60shroudforge.settings.get(key, fallback)\x60.\n\n\x60\x60\x60lua\nlocal speed = shroudforge.settings.get("flightSpeed", 1.0)\n\x60\x60\x60\n\nDer Schlüssel muss exakt übereinstimmen. Der Fallback sollte denselben Datentyp haben wie \x60value\x60. Rufe \x60get\x60 im Callback auf, wenn eine Spielerauswahl ohne Neustart berücksichtigt werden soll. Ein beim Laden einmal gelesener lokaler Wert wird nicht von selbst aktualisiert.\n\nEin Mod, der diese Laufzeitfunktion verwendet, braucht \x60runtime\x60 in \x60mod.json\x60. Die Funktion selbst speichert keine Einstellung, das erledigt der Modloader. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls).`,
    bodyEn: `# Read mod settings in Lua\n\nThe description and starting value live in \x60extended.mod.json\x60. Lua reads the current value with \x60shroudforge.settings.get(key, fallback)\x60.\n\n\x60\x60\x60lua\nlocal speed = shroudforge.settings.get("flightSpeed", 1.0)\n\x60\x60\x60\n\nThe key must match exactly. The fallback should have the same type as \x60value\x60. Call \x60get\x60 inside a callback when a player's change should take effect without restarting. A local value read once during loading does not update by itself.\n\nA mod using this runtime function needs \x60runtime\x60 in \x60mod.json\x60. The function does not save settings, Modloader does. See [Settings and controls](#doc-setting-controls).`,
  },
  {
    id: "api-ui-actions", folder: "lua-api/ui-actions", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 3,
    titleDe: "Modloader-Aktionsknöpfe mit Lua verbinden", titleEn: "Connect Modloader buttons to Lua",
    summaryDe: "Eine Aktions-ID in der Moddatei verbindet den sichtbaren Knopf mit einer Callback-Funktion deines Mods.",
    summaryEn: "An action ID in the mod file connects a visible button to a callback in your mod.",
    bodyDe: `# Modloader-Aktionsknöpfe mit Lua verbinden\n\nEine Aktion steht unter \x60groups[].actions[]\x60. Die \x60id\x60 wird an \x60shroudforge.ui.on_action\x60 übergeben. Beide IDs müssen gleich geschrieben sein.\n\n\x60\x60\x60lua\nshroudforge.ui.on_action("resetFeature", function()\n  shroudforge.log.info("Reset action requested")\nend)\n\x60\x60\x60\n\nDer Mod braucht \x60runtime\x60. Die Funktion muss registriert sein, während die Mod-Laufzeit geladen wird. \x60label\x60 und \x60style\x60 beschreiben die sichtbare Schaltfläche, \x60confirm\x60 ist eine optionale Rückfrage.\n\nDie Aktion selbst führt nur deinen Callback aus. Sie garantiert keine erfolgreiche Spieländerung. Prüfe Rückgabewerte und logge das Ergebnis. Details der API stehen in der [API-Referenz](#api).`,
    bodyEn: `# Connect Modloader buttons to Lua\n\nAn action is declared under \x60groups[].actions[]\x60. Its \x60id\x60 is passed to \x60shroudforge.ui.on_action\x60. The two IDs must match exactly.\n\n\x60\x60\x60lua\nshroudforge.ui.on_action("resetFeature", function()\n  shroudforge.log.info("Reset action requested")\nend)\n\x60\x60\x60\n\nThe mod needs \x60runtime\x60. Register the callback while the mod runtime loads. \x60label\x60 and \x60style\x60 describe the visible button, while \x60confirm\x60 adds an optional prompt.\n\nThe action only calls your callback. It does not guarantee a game change succeeded. Check return values and log the outcome. See the [API reference](#api) for function details.`,
  },
  {
    id: "api-logging", folder: "lua-api/logging", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 4,
    titleDe: "Verständliche Mod-Meldungen schreiben", titleEn: "Write useful mod log messages",
    summaryDe: "Logs sollen zeigen, was der Mod tut, in welcher Phase er ist und ob die Aktion bestätigt wurde oder scheiterte.",
    summaryEn: "Logs should show what the mod is doing, which phase it reached, and whether the action was confirmed or failed.",
    bodyDe: `# Verständliche Mod-Meldungen schreiben\n\nNutze \x60shroudforge.log\x60 für technische Abläufe des Mods. Eine gute Meldung nennt Aktion und Ergebnis, ein Fehler nennt zusätzlich den Grund. Die genaue gemeinsame Richtlinie steht in [Logging für Mods und Loader](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/sf/logging.md).\n\n\x60\x60\x60lua\nshroudforge.log.info("Blueprint export started")\nshroudforge.log.warn("Blueprint export paused, destination is unavailable")\nshroudforge.log.error("Blueprint export failed, write verification did not match")\n\x60\x60\x60\n\nNutze \x60INFO\x60 für wichtige Vorgänge und bestätigte Ergebnisse, \x60WARN\x60 für blockierte oder unklare Abläufe, \x60ERROR\x60 für echte Fehlschläge. \x60DEBUG\x60 und \x60TRACE\x60 sind für Diagnosewerte und Detailverläufe. Stelle identische Fehler nicht still ab, erkläre stattdessen die Rolle jeder Meldung, wenn mehrere Ebenen beteiligt sind.\n\nIm Client und Dedicated Server werden getrennte ShroudForge-Logs geschrieben. Die effektive Mindeststufe steuert, was gespeichert wird. Der Debug-Console-Filter blendet nur gespeicherte Zeilen aus oder ein.`,
    bodyEn: `# Write useful mod log messages\n\nUse \x60shroudforge.log\x60 for technical mod activity. A useful message names the action and result, while an error also explains why it failed. The shared policy is in [Logging for mods and the loader](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/sf/logging.md).\n\n\x60\x60\x60lua\nshroudforge.log.info("Blueprint export started")\nshroudforge.log.warn("Blueprint export paused, destination is unavailable")\nshroudforge.log.error("Blueprint export failed, write verification did not match")\n\x60\x60\x60\n\nUse \x60INFO\x60 for important operations and confirmed results, \x60WARN\x60 for blocked or uncertain flows, and \x60ERROR\x60 for actual failures. Use \x60DEBUG\x60 and \x60TRACE\x60 for diagnostic values and detailed traces. Do not silently suppress a duplicate failure, explain the role of each message when more than one layer is involved.\n\nClient and Dedicated Server write separate ShroudForge logs. The effective minimum level controls what gets saved. The Debug Console filter only hides or shows lines that are already stored.`,
  },
  {
    id: "api-files", folder: "lua-api/files-and-exports", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 5,
    apiPrefixes: ["io.", "buffer.", "Buffer:", "game.assets."],
    titleDe: "Dateien lesen und Exporte speichern", titleEn: "Read files and save exports",
    summaryDe: "Paketdateien und Exportdateien haben unterschiedliche Orte und Berechtigungen. Verwende den passenden API-Bereich.",
    summaryEn: "Package files and exported files use different locations and permissions. Choose the matching API surface.",
    bodyDe: `# Dateien lesen und Exporte speichern\n\nDie globale \x60io\x60-API trennt mitgelieferte Paketdateien von exportierten Spielerdaten. Ein Mod sollte nicht annehmen, dass beide Bereiche denselben Pfad oder dieselbe Schreibberechtigung haben.\n\n## Berechtigung\n\nFür Exportfunktionen muss \x60export\x60 in \x60mod.json\x60 stehen. Prüfe den API-Rückgabewert, bevor du einen Erfolg meldest. Nutze die [API-Referenz](#api), um die genaue Funktion und Argumente nachzuschlagen.\n\n## Typischer Ablauf\n\n1. Bestimme einen relativen Exportpfad.\n2. Erzeuge die Bytes mit der passenden Buffer- oder Serialisierungsfunktion.\n3. Rufe die Exportfunktion auf.\n4. Prüfe den Rückgabewert und schreibe eine verständliche Ergebnis- oder Fehlermeldung.\n\nDer Loader begrenzt Pfade auf den jeweiligen Modbereich. Die Exportfunktion ist kein allgemeiner Zugriff auf beliebige Dateien des Computers.`,
    bodyEn: `# Read files and save exports\n\nThe global \x60io\x60 API separates bundled package files from player exports. A mod should not assume that these areas use the same path or write permission.\n\n## Capability\n\nExport functions require \x60export\x60 in \x60mod.json\x60. Check the API result before reporting success. Use the [API reference](#api) to look up exact functions and arguments.\n\n## Typical flow\n\n1. Choose a relative export path.\n2. Create bytes with the matching buffer or serialization function.\n3. Call the export function.\n4. Check its result and log a useful success or failure message.\n\nThe loader keeps paths inside the mod's permitted area. Export is not unrestricted access to arbitrary files on the computer.`,
  },
  {
    id: "api-runtime", folder: "lua-api/runtime-and-game", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 6,
    titleDe: "Laufzeit, Client und Server", titleEn: "Runtime, client, and server",
    summaryDe: "Laufzeit-Mods starten in einem bestimmten Spielprozess. Native Spielfunktionen hängen zusätzlich vom Spielbuild und freigegebenem Profil ab.",
    summaryEn: "Runtime mods start in a selected game process. Native game operations also depend on the game build and a supported profile.",
    bodyDe: `# Laufzeit, Client und Server\n\nDie Lua-Laufzeit führt \x60src/mod.lua\x60 in dem Prozess aus, der zu \x60targets\x60 passt. Client heißt Spielprozess des Spielers, Server heißt Dedicated-Server-Prozess. Siehe [targets](#doc-targets).\n\n## Prozessziel ist keine Multiplayer-Synchronisierung\n\n\x60targets\x60 steuert, wo der Code läuft. Es überträgt keine Werte an andere Clients und repliziert keine Weltänderung. Für Netzwerkfunktionen müssen passende APIs und eine explizite serverseitige Verarbeitung verwendet werden.\n\n## Laufzeit-Callbacks und native Unterstützung\n\nDie Mod muss benötigte Laufzeitfähigkeiten in \x60mod.json\x60 deklarieren. Manche Spielzugriffe verwenden KFC Runtime, das eingebaute native Modul. Diese Aufrufe benötigen einen passenden Client- oder Serverbuild und ein unterstütztes Profil. Ein erfolgreicher Lua-Start bestätigt noch nicht jede native Aktion.\n\nBeginne mit [Lua und KFC Runtime](#runtime) und suche konkrete Symbole in der [API-Referenz](#api).`,
    bodyEn: `# Runtime, client, and server\n\nThe Lua runtime runs \x60src/mod.lua\x60 in the process selected by \x60targets\x60. Client means the player's game process, server means the Dedicated Server process. See [targets](#doc-targets).\n\n## A process target is not multiplayer synchronization\n\n\x60targets\x60 selects where code runs. It does not send values to other clients or replicate a world edit. Network behavior needs suitable APIs and explicit server-side handling.\n\n## Runtime callbacks and native support\n\nA mod declares required runtime capabilities in \x60mod.json\x60. Some game operations use KFC Runtime, ShroudForge's built-in native module. These calls need a matching client or server build and a supported profile. A successful Lua start does not confirm that every native action is available.\n\nStart with [Lua and KFC Runtime](#runtime), then search for specific symbols in the [API reference](#api).`,
  },
];

for (const page of core) addPage(page);

addPage({
  id: "api-network", folder: "lua-api/network-p2p", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 7,
  titleDe: "Steam-P2P-Nachrichten zwischen Mod-Prozessen", titleEn: "Steam P2P messages between mod processes",
  summaryDe: "runtime.network stellt einen eigenen Steam-Nachrichtenkanal bereit. Er versendet keine normalen Enshrouded-Spielpakete.",
  summaryEn: "runtime.network provides a separate Steam messaging channel. It does not send normal Enshrouded game packets.",
  bodyDe: `# Steam-P2P-Nachrichten zwischen Mod-Prozessen\n\n\x60runtime.network\x60 ermöglicht Nachrichten zwischen Mod-Laufzeiten auf einem Client und einem Dedicated Server. Es ist ein eigener Steam Networking Messages Transport, keine Enshrouded-Spielnachricht. Siehe auch [targets](#doc-targets).\n\n## Nachrichten für einen bestimmten Mod\n\n\x60send_mod\x60 sendet eine UTF-8-Nachricht an eine konkrete SteamID64 und Mod-ID. Der Empfänger liest sie mit \x60receive_mod\x60. Das ist praktisch, wenn beide Seiten denselben Mod installiert haben. Es gibt keinen Broadcast.\n\n## Eigene Kanäle\n\n\x60send\x60 und \x60receive\x60 arbeiten mit binärsicheren Lua-Strings. Öffentliche Kanäle reichen von 0 bis 65534. Kanal 65535 ist für die interne Health-Prüfung reserviert. Der Kanal muss auf beiden Seiten übereinstimmen.\n\n## Serverfreigabe und SteamID64\n\nDer Dedicated Server akzeptiert nur aktuell von Enshrouded authentifizierte, verbundene Spieler. Eine optionale Freigabeliste unter Modloader-Einstellungen → Netzwerk kann diesen Kreis weiter einschränken. Eine leere Liste bedeutet alle aktuell authentifizierten Spieler. Der Client erkennt die Server-SteamID64 aus dem Live-Kontext und der Health-Prüfung. Die Einstellung \x60serverSteamId\x60 ist ein Fallback für entfernte Server.\n\n## Ergebnis prüfen\n\nEin erfolgreicher \x60send\x60-Aufruf bedeutet, dass Steam den Sendeauftrag angenommen hat. Er bestätigt nicht, dass der andere Prozess die Nachricht empfangen oder verarbeitet hat. Prüfe Protokoll, Sender, Mod-ID und Berechtigung im Empfänger. Das API-Verzeichnis dokumentiert Payload-Grenzen und Rückgabewerte.`,
  bodyEn: `# Steam P2P messages between mod processes\n\n\x60runtime.network\x60 sends messages between mod runtimes on a client and a Dedicated Server. It is a separate Steam Networking Messages transport, not an Enshrouded game packet. See [targets](#doc-targets) as well.\n\n## Messages for a specific mod\n\n\x60send_mod\x60 sends UTF-8 text to one SteamID64 and mod ID. The receiving mod reads it with \x60receive_mod\x60. Use this when both sides install the same mod. There is no broadcast.\n\n## Raw channels\n\n\x60send\x60 and \x60receive\x60 use binary-safe Lua strings. Public channels are 0 through 65534. Channel 65535 is reserved for the internal health probe. Both sides must use the same channel.\n\n## Server authorization and SteamID64\n\nThe Dedicated Server accepts only players Enshrouded has currently authenticated and connected. An optional allowlist under Modloader Settings → Network can further restrict this set. An empty list means all currently authenticated players. The client learns the server SteamID64 from live world context and the health probe. \x60serverSteamId\x60 is a fallback for remote servers.\n\n## Check the result\n\nA successful \x60send\x60 call means Steam accepted the send request. It does not confirm that the other process received or handled the message. Validate the protocol, sender, mod ID, and permissions on the receiving side. The API index documents payload limits and return values.`,
});

addPage({
  id: "api-game-data", folder: "lua-api/game-data", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 8,
  titleDe: "Spielressourcen, ECS und reflektierte Datentypen", titleEn: "Game assets, ECS, and reflected data types",
  summaryDe: "Asset- und Runtime-Funktionen arbeiten in unterschiedlichen Phasen und brauchen passende Fähigkeiten oder ein unterstütztes Spielprofil.",
  summaryEn: "Asset and runtime functions operate in different phases and need the right capabilities or a supported game profile.",
  bodyDe: `# Spielressourcen, ECS und reflektierte Datentypen\n\nDie Lua-API enthält mehrere Bereiche, die sich nach dem Zeitpunkt und der Art des Zugriffs unterscheiden. Die [API-Referenz](#api) listet alle Funktionen und Typen.\n\n## Assets vorbereiten\n\n\x60game.assets\x60 liest und verändert unterstützte Spielressourcen in der Vorbereitungsphase. Änderungen werden für einen späteren Spielstart vorbereitet. Sie werden nicht sofort in eine bereits laufende Welt übernommen. Deklariere die passende \x60patch\x60- oder \x60export\x60-Fähigkeit und prüfe die verfügbaren Ressourcentypen des aktiven Profils.\n\n## Laufzeit und ECS\n\n\x60runtime.ecs\x60 arbeitet während des Spiels mit unterstützten Komponenten. Der Loader prüft Profil, Typen und Ausführungsphase. Verwende nur dokumentierte Abfragen und Schreibfunktionen und prüfe ihren Rückgabestatus. Eine bestätigte Speicheroperation ist kein unabhängiger Nachweis der sichtbaren Spielwirkung.\n\n## Typen und Datenwerte\n\n\x60game.types\x60 und \x60runtime.types\x60 stellen die reflektierten Typinformationen des aktiven Spielprofils bereit. \x60runtime.values\x60 kann eigene Bytefolgen gemäß einem bekannten Typ lesen oder erzeugen. Das Dereferenzieren beliebiger Engine-Zeiger ist nicht Teil dieses sicheren Wertezugriffs.\n\n## Warum ein Spielupdate wichtig ist\n\nDie verfügbaren Typen und Operationen hängen vom installierten Enshrouded-Build und Profil ab. Ein Mod kann vorhanden und geladen sein, obwohl ein bestimmter nativer Zugriff nicht unterstützt wird. Prüfe \x60runtime.has(feature)\x60 und den Status der konkreten Funktion.`,
  bodyEn: `# Game assets, ECS, and reflected data types\n\nThe Lua API has several areas with different timing and access behavior. The [API reference](#api) lists all functions and types.\n\n## Prepare assets\n\n\x60game.assets\x60 reads and changes supported game resources during preparation. Changes are prepared for a later game start. They do not update an already running world immediately. Declare the matching \x60patch\x60 or \x60export\x60 capability and check resource types available in the active profile.\n\n## Runtime and ECS\n\n\x60runtime.ecs\x60 works with supported components while the game is running. The loader checks the profile, types, and execution phase. Use documented query and write functions and inspect their result status. A confirmed memory operation is not independent proof of the visible gameplay effect.\n\n## Types and values\n\n\x60game.types\x60 and \x60runtime.types\x60 expose reflected type information from the active game profile. \x60runtime.values\x60 can read or create owned byte strings according to a known type. Dereferencing arbitrary engine pointers is not part of this safe value access.\n\n## Why game updates matter\n\nAvailable types and operations depend on the installed Enshrouded build and profile. A mod can be present and loaded while one native access is unsupported. Check \x60runtime.has(feature)\x60 and the status of the specific function.`,
});

addPage({
  id: "api-lifecycle", folder: "lua-api/lifecycle", navGroup: { de: "Lua-API-Anleitungen", en: "Lua API guides" }, navOrder: 50, order: 9,
  titleDe: "Laufzeit-Callbacks", titleEn: "Runtime callbacks",
  summaryDe: "on_load, on_update und on_unload steuern, wann dein Lua-Mod während einer Spielsitzung arbeitet.",
  summaryEn: "on_load, on_update, and on_unload control when your Lua mod runs during a game session.",
  bodyDe: `# Laufzeit-Callbacks\n\nEin Laufzeit-Mod gibt eine Tabelle mit Callback-Funktionen zurück. Der Loader ruft sie zum passenden Zeitpunkt auf. Deklariere \x60runtime\x60 in \x60mod.json\x60.\n\n\x60\x60\x60lua\nruntime.require("runtime.lifecycle")\n\nreturn {\n  on_load = function()\n    shroudforge.log.info("Mod runtime started")\n  end,\n  on_update = function(delta_seconds)\n    -- Read current settings or process queued work here\n  end,\n  on_unload = function()\n    shroudforge.log.info("Mod runtime stopped")\n  end,\n  update_interval_ms = 100\n}\n\x60\x60\x60\n\n\x60on_load\x60 läuft beim Start der Mod-Laufzeit. \x60on_update\x60 erhält die verstrichene Zeit in Sekunden. \x60on_unload\x60 läuft beim Beenden oder Entladen. Das Updateintervall ist standardmäßig 50 ms und erlaubt 8 bis 1000 ms. Verpasste Intervalle werden nicht nachgeholt.\n\nNutze \x60on_update\x60 nicht für unnötige Arbeit pro Frame. Lies live änderbare Werte in der Callback-Funktion erneut. Prüfe bei Netzwerk- und nativen Aufrufen Rückgabewerte und Status.`,
  bodyEn: `# Runtime callbacks\n\nA runtime mod returns a table of callback functions. The loader calls them at the matching time. Declare \x60runtime\x60 in \x60mod.json\x60.\n\n\x60\x60\x60lua\nruntime.require("runtime.lifecycle")\n\nreturn {\n  on_load = function()\n    shroudforge.log.info("Mod runtime started")\n  end,\n  on_update = function(delta_seconds)\n    -- Read current settings or process queued work here\n  end,\n  on_unload = function()\n    shroudforge.log.info("Mod runtime stopped")\n  end,\n  update_interval_ms = 100\n}\n\x60\x60\x60\n\n\x60on_load\x60 runs when the mod runtime starts. \x60on_update\x60 receives elapsed time in seconds. \x60on_unload\x60 runs when the mod stops or unloads. The default update interval is 50 ms and the accepted range is 8 to 1000 ms. Missed intervals are not replayed.\n\nDo not use \x60on_update\x60 for unnecessary per-frame work. Read live settings again inside the callback. Check return values and status for network and native operations.`,
});

const fieldPurpose = {
  id: ["Stabile Kennung, auf die andere Mods verweisen können.", "Stable identifier that other mods can depend on."],
  name: ["Name, den Spieler im Modloader sehen.", "Name players see in the Modloader."],
  version: ["Version dieser Mod-Veröffentlichung im SemVer-Grundformat.", "Version of this mod release using the basic SemVer format."],
  description: ["Kurzer Text für die Modseite.", "Short text shown on the mod page."],
  authors: ["Namen der Personen, die den Mod erstellt haben.", "Names of the people who created the mod."],
  license: ["Lizenzkennung, falls der Mod eine Lizenz erklärt.", "License identifier when the mod declares a license."],
  icon: ["Icon-Datei im Paketstamm oder unter assets/.", "Icon file in the package root or under assets/."],
  dependencies: ["Andere Mods, deren Verfügbarkeit oder Version der Loader vor dem Start prüfen muss.", "Other mods whose availability or version the loader checks before startup."],
  dependencyId: ["Exakte Mod-ID aus der mod.json des benötigten Mods.", "Exact mod ID from the required mod's mod.json."],
  dependencyVersion: ["SemVer-Bereich, der für die Version des benötigten Mods akzeptiert wird.", "SemVer range accepted for the required mod's version."],
  actionId: ["Eindeutiger Aktionsname, den der Mod in Lua registriert.", "Unique action name that the mod registers in Lua."],
  capabilities: ["Zugriffsarten, die der Mod anfordert.", "Access categories requested by the mod."],
  enabled: ["Gespeicherter Aktivierungszustand für diesen Mod.", "Saved enabled state for this mod."],
  schemaVersion: ["Version der extended.mod.json-Struktur.", "Version of the extended.mod.json structure."],
  targets: ["Wählt den Client-Prozess, den Dedicated Server oder beide als Ausführungsort des Mods.", "Selects the client process, the Dedicated Server, or both as mod execution targets."],
  launcher: ["Herkunft des Mod-Pakets für die Modloader-Anzeige.", "Package origin used in the Modloader display."],
  links: ["Bekannte Link-IDs und HTTPS-Adressen für Modloader-Schaltflächen.", "Recognized link IDs and HTTPS addresses for Modloader buttons."],
  changelog: ["Kurze Versionshinweise auf der Modseite.", "Short version notes on the mod page."],
  settings: ["Mod-eigene Einstellungswerte und ihre Anzeigenmetadaten.", "Mod-specific setting values and their display metadata."],
  groups: ["Gruppen für Einstellungen und Modloader-Aktionen.", "Groups for settings and Modloader actions."],
  value: ["Startwert und erwarteter Lua-Datentyp der Einstellung.", "Starting value and expected Lua data type of the setting."],
  label: ["Spielerfreundlicher Name, der im Modloader angezeigt wird.", "Player-facing name displayed in the Modloader."],
  control: ["Art des Eingabefelds, das der Modloader zeigt.", "Input control displayed by the Modloader."],
  min: ["Kleinster zulässiger Eingabewert.", "Smallest accepted input value."],
  max: ["Größter zulässiger Eingabewert.", "Largest accepted input value."],
  step: ["Schrittweite für Zahlen- und Reglerfelder.", "Increment for number and slider inputs."],
  minLength: ["Mindestlänge eines Textwerts.", "Minimum length of a text value."],
  maxLength: ["Höchstlänge eines Textwerts.", "Maximum length of a text value."],
  options: ["Erlaubte Auswahlwerte mit den Namen, die Spieler sehen.", "Allowed choices and the labels players see."],
  actions: ["Aktionsknöpfe, deren IDs Lua-Callbacks aufrufen.", "Action buttons whose IDs call Lua callbacks."],
  groupSettings: ["Schlüssel vorhandener Mod-Einstellungen, die in dieser Gruppe angezeigt werden.", "Keys of existing mod settings displayed in this group."],
  style: ["Darstellung des Aktionsknopfs, ohne die Funktion zu ändern.", "Appearance of the action button, without changing its behavior."],
  confirm: ["Optionale Rückfrage vor dem Ausführen einer Aktion.", "Optional confirmation prompt before running an action."],
  optional: ["Ob eine fehlende oder inkompatible Abhängigkeit den Start blockiert.", "Whether a missing or incompatible dependency blocks startup."],
};

function constraints(schema, locale) {
  if (!schema || typeof schema !== "object") return "";
  const values = [];
  if (schema.const !== undefined) values.push(`${locale === "de" ? "fester Wert" : "fixed value"}: ${code(JSON.stringify(schema.const))}`);
  if (schema.enum) values.push(`${locale === "de" ? "Werte" : "values"}: ${schema.enum.map(value => code(JSON.stringify(value))).join(", ")}`);
  if (schema.items?.enum) values.push(`${locale === "de" ? "Listenelemente" : "list items"}: ${schema.items.enum.map(value => code(JSON.stringify(value))).join(", ")}`);
  for (const [key, label] of [["minLength", locale === "de" ? "Mindestlänge" : "minimum length"], ["maxLength", locale === "de" ? "Höchstlänge" : "maximum length"], ["minItems", locale === "de" ? "Mindesteinträge" : "minimum items"], ["maxItems", locale === "de" ? "Höchsteinträge" : "maximum items"], ["minimum", locale === "de" ? "Minimum" : "minimum"], ["maximum", locale === "de" ? "Maximum" : "maximum"], ["exclusiveMinimum", locale === "de" ? "größer als" : "greater than"]]) {
    if (schema[key] !== undefined) values.push(`${label}: ${code(schema[key])}`);
  }
  if (schema.pattern) values.push(`${locale === "de" ? "Muster" : "pattern"}: ${code(schema.pattern)}`);
  if (schema.uniqueItems) values.push(locale === "de" ? "Einträge müssen eindeutig sein." : "Items must be unique.");
  if (schema.description) {
    const translations = {
      "Optional license identifier (for example, MIT). Use null when no license is declared.": [
        "Optionale Lizenzkennung, zum Beispiel MIT. Verwende null, wenn keine Lizenz angegeben wird.",
        schema.description,
      ],
      "Optional mod icon filename in the package root or path under assets/. Use null when the mod has no icon.": [
        "Optionaler Dateiname des Mod-Icons im Paketordner oder ein Pfad unter assets/. Verwende null, wenn der Mod kein Icon hat.",
        schema.description,
      ],
      "Mods that must be available before this mod starts. Each id is another mod's manifest id. Version is a SemVer version requirement. If optional is true, the dependency does not block startup when missing, disabled, or outside the version range.": [
        "Mods, die vor dem Start dieses Mods verfügbar sein müssen. id muss der Mod-ID aus der anderen mod.json entsprechen. version ist ein SemVer-Bereich. Bei optional: true blockiert eine fehlende, deaktivierte oder unpassende Abhängigkeit den Start nicht.",
        schema.description,
      ],
      "Exact id from the dependency mod's mod.json.": [
        "Exakte ID aus der mod.json des benötigten Mods.",
        schema.description,
      ],
      "SemVer version requirement, for example ^1.2.0 or >=1.2.0, <2.0.0.": [
        "SemVer-Versionsbereich, zum Beispiel ^1.2.0 oder >=1.2.0, <2.0.0.",
        schema.description,
      ],
      "When true, this mod may start if the dependency is unavailable. Defaults to false.": [
        "Wenn true, darf dieser Mod auch starten, wenn die Abhängigkeit fehlt. Standard ist false.",
        schema.description,
      ],
      "Processes this mod runs in. Legacy EML packages without targets default to client and server. ShroudForge packages default to client.": [
        "Legt fest, in welchen Prozessen der Mod läuft. EML-Pakete ohne targets laufen standardmäßig auf Client und Server. ShroudForge-Pakete laufen standardmäßig auf dem Client.",
        schema.description,
      ],
      "Launcher that originally identified this mod. Omit for ShroudForge mods. Use EML to preserve EML provenance.": [
        "Herkunft des Mods. Für ShroudForge-Mods weglassen. EML eintragen, um die Herkunft eines migrierten EML-Mods zu erhalten.",
        schema.description,
      ],
    };
    values.push(translations[schema.description]?.[locale === "de" ? 0 : 1] || schema.description);
  }
  return values.length ? values.map(value => `- ${value}`).join("\n") : (locale === "de" ? "Keine zusätzlichen Werte oder Grenzen im Schema angegeben." : "No additional values or limits are declared by the schema.");
}

function sampleForSchema(schema) {
  if (!schema || typeof schema !== "object") return null;
  if (schema.const !== undefined) return schema.const;
  if (schema.enum?.length) return schema.enum[0];
  const types = Array.isArray(schema.type) ? schema.type.filter(type => type !== "null") : [schema.type];
  const type = types[0];
  if (type === "array") return [sampleForSchema(schema.items)];
  if (type === "object") {
    const sample = {};
    for (const key of schema.required || []) sample[key] = sampleForSchema(schema.properties?.[key]);
    return sample;
  }
  if (type === "boolean") return true;
  if (type === "integer" || type === "number") return schema.minimum ?? 1;
  return "example";
}

function exampleForField(pathLabel, property, value) {
  if (pathLabel.startsWith("dependencies[].")) {
    const dependency = { id: "helper.mod", version: "^1.0.0" };
    dependency[property] = value;
    return { dependencies: [dependency] };
  }
  if (pathLabel.startsWith("settings.<key>.")) {
    const setting = { value: true };
    if (property === "control") setting.control = value;
    if (property === "control" && value === "slider") setting.value = 1;
    if (["min", "max", "step"].includes(property)) {
      setting.value = 1;
      setting.control = "number";
    }
    if (["minLength", "maxLength"].includes(property)) {
      setting.value = "hello";
      setting.control = "text";
    }
    if (property === "options") {
      setting.value = "easy";
      setting.control = "select";
    }
    if (["select", "radio", "segmented"].includes(value)) setting.value = "easy";
    if (value === "multiselect") setting.value = ["easy"];
    if (["select", "radio", "segmented", "multiselect"].includes(value) || property === "options") {
      setting.options = { easy: "Easy", hard: "Hard" };
    }
    setting[property] = value;
    return { settings: { example: setting } };
  }
  if (pathLabel.startsWith("groups[].actions[].")) {
    const action = { id: "reset", label: "Reset" };
    action[property] = value;
    return { groups: [{ label: "Example", actions: [action] }] };
  }
  if (pathLabel.startsWith("groups[].")) {
    const group = { label: "Example", settings: [] };
    group[property] = value;
    return { groups: [group] };
  }
  return { [property]: value };
}

function schemaField({ schemaName, schema, property, pathLabel, file, section, group, groupOrder, fieldOrder, purposeName = property, example, requiredOverride }) {
  const localized = fieldPurpose[purposeName] || fieldPurpose[property] || [
    `Dieses Feld konfiguriert ${code(pathLabel)}. Der genaue technische Vertrag steht im Schema.`,
    `This field configures ${code(pathLabel)}. The exact technical contract is defined by the schema.`,
  ];
  const required = requiredOverride ?? (schemaName === "mod.json" ? manifestSchema.required?.includes(property) : extensionSchema.required?.includes(property));
  const type = Array.isArray(schema?.type) ? schema.type.join(" | ") : schema?.type || (schema?.enum ? "string" : "object");
  const typeTextDe = `${code(type)}${required ? " · erforderlich" : " · optional"}`;
  const typeTextEn = `${code(type)}${required ? " · required" : " · optional"}`;
  const exampleText = example ?? sampleForSchema(schema);
  const exampleJson = JSON.stringify(exampleForField(pathLabel, property, exampleText), null, 2);
  const baseId = `field-${slug(pathLabel)}`;
  const id = pages.some(page => page.id === baseId) ? `${baseId}-${slug(schemaName)}` : baseId;
  const folder = `${section}/${slug(pathLabel)}`;
  const titleDe = `Feld ${code(pathLabel)}`;
  const titleEn = `The ${code(pathLabel)} field`;
  const summaryDe = localized[0];
  const summaryEn = localized[1];
  addPage({
    id, folder, navGroup: group, navOrder: groupOrder, order: fieldOrder,
    titleDe, titleEn, summaryDe, summaryEn,
    bodyDe: `${articleIntro(titleDe, summaryDe)}\n## Datentyp und Pflicht\n\n${typeTextDe}\n\n## Erlaubte Werte und Grenzen\n\n${constraints(schema, "de")}\n\n## Beispiel\n\n\x60\x60\x60json\n${exampleJson}\n\x60\x60\x60\n\n## Quelle\n\nSchema: [${schemaName}](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/${file}). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.`,
    bodyEn: `${articleIntro(titleEn, summaryEn)}\n## Type and requirement\n\n${typeTextEn}\n\n## Allowed values and limits\n\n${constraints(schema, "en")}\n\n## Example\n\n\x60\x60\x60json\n${exampleJson}\n\x60\x60\x60\n\n## Source\n\nSchema: [${schemaName}](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/${file}). The package reader checks additional relationships between fields.`,
  });
}

const modFieldOrder = ["id", "name", "version", "description", "authors", "license", "icon", "dependencies", "capabilities", "$schema"];
for (const [property, schema] of Object.entries(manifestSchema.properties)) {
  schemaField({ schemaName: "mod.json", schema, property, pathLabel: property, file: "manifest.schema.json", section: "mod-json/fields", group: { de: "mod.json", en: "mod.json" }, groupOrder: 10, fieldOrder: modFieldOrder.indexOf(property) < 0 ? 99 : modFieldOrder.indexOf(property), example: property === "id" ? "author.example-mod" : property === "name" ? "Example Mod" : property === "version" ? "1.0.0" : property === "authors" ? ["Your Name"] : property === "capabilities" ? ["runtime"] : undefined });
}
const depSchema = manifestSchema.properties.dependencies.items.properties;
for (const property of ["id", "version", "optional"]) schemaField({ schemaName: "mod.json", schema: depSchema[property], property, purposeName: property === "id" ? "dependencyId" : property === "version" ? "dependencyVersion" : property, pathLabel: `dependencies[].${property}`, file: "manifest.schema.json", section: "mod-json/dependencies", group: { de: "mod.json, Abhängigkeiten", en: "mod.json, dependencies" }, groupOrder: 11, fieldOrder: ["id", "version", "optional"].indexOf(property), requiredOverride: property === "id" || property === "version", example: property === "id" ? "author.shared-library" : property === "version" ? "^1.2.0" : true });

for (const [property, schema] of Object.entries(extensionSchema.properties)) {
  if (property === "settings" || property === "groups") {
    const purpose = property === "settings"
      ? ["Dynamische Schlüssel und Werte, die der Modloader als Mod-Einstellungen darstellt und Lua lesen kann.", "Dynamic keys and values that Modloader presents as mod settings and Lua can read."]
      : ["Optionale Gruppen für Einstellungsfelder und Lua-Aktionen.", "Optional groups for setting fields and Lua actions."];
    addPage({
      id: `extension-${property}`, folder: `extended-mod-json/${property}`, navGroup: { de: "extended.mod.json", en: "extended.mod.json" }, navOrder: 20, order: property === "settings" ? 10 : 11,
      titleDe: `Feld ${code(property)}`, titleEn: `The ${code(property)} field`, summaryDe: purpose[0], summaryEn: purpose[1],
      bodyDe: articleIntro("Feld " + code(property), purpose[0]) + "\n\n" + (property === "settings" ? "Jeder Schlüssel ist mod-eigen und muss dem Schlüssel entsprechen, den Lua an `shroudforge.settings.get(key, fallback)` übergibt. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls), [settings.<key>](#doc-extension-setting-key) und [alle Metadatenfelder](#doc-field-setting-key-value)." : "Eine Gruppe hat `label` und kann `description`, `settings` sowie `actions` enthalten. Nicht gruppierte Einstellungen erscheinen in der Standardgruppe. Siehe [Gruppen und Aktionsknöpfe](#doc-setting-groups-actions)."),
      bodyEn: articleIntro("The " + code(property) + " field", purpose[1]) + "\n\n" + (property === "settings" ? "Every key is specific to the mod and must match the key passed to `shroudforge.settings.get(key, fallback)` in Lua. See [Settings and controls](#doc-setting-controls), [settings.<key>](#doc-extension-setting-key), and [all metadata fields](#doc-field-setting-key-value)." : "A group has `label` and may include `description`, `settings`, and `actions`. Ungrouped settings appear in the default group. See [Groups and action buttons](#doc-setting-groups-actions)."),
    });
  }
  if (property === "settings" || property === "groups") continue;
  schemaField({ schemaName: "extended.mod.json", schema, property, pathLabel: property, file: "extended.mod.schema.json", section: "extended-mod-json/fields", group: { de: "extended.mod.json", en: "extended.mod.json" }, groupOrder: 20, fieldOrder: Object.keys(extensionSchema.properties).indexOf(property), example: property === "targets" ? ["client"] : property === "launcher" ? "EML" : property === "schemaVersion" ? 1 : property === "enabled" ? false : undefined });
}
const settingSchema = extensionSchema.$defs.setting.properties;
for (const property of Object.keys(settingSchema)) schemaField({ schemaName: "extended.mod.json", schema: settingSchema[property], property, pathLabel: `settings.<key>.${property}`, file: "extended.mod.schema.json", section: "extended-mod-json/setting-fields", group: { de: "Einstellungen", en: "Settings" }, groupOrder: 30, fieldOrder: Object.keys(settingSchema).indexOf(property), requiredOverride: property === "value", example: property === "value" ? true : property === "control" ? "slider" : property === "options" ? { easy: "Easy", hard: "Hard" } : property === "step" ? 0.1 : property === "minLength" ? 3 : property === "maxLength" ? 5 : undefined });
for (const property of ["label", "description", "settings", "actions"]) schemaField({ schemaName: "extended.mod.json", schema: extensionSchema.$defs.group.properties[property], property, pathLabel: `groups[].${property}`, file: "extended.mod.schema.json", section: "extended-mod-json/group-fields", group: { de: "Gruppen und Aktionen", en: "Groups and actions" }, groupOrder: 31, fieldOrder: ["label", "description", "settings", "actions"].indexOf(property), purposeName: property === "settings" ? "groupSettings" : property, requiredOverride: property === "label", example: property === "label" ? "Gameplay" : property === "settings" ? ["enabledFeature"] : property === "actions" ? [{ id: "reset", label: "Reset" }] : undefined });
const actionSchema = extensionSchema.$defs.action.properties;
for (const property of Object.keys(actionSchema)) schemaField({ schemaName: "extended.mod.json", schema: actionSchema[property], property, pathLabel: `groups[].actions[].${property}`, file: "extended-mod.schema.json", section: "extended-mod-json/action-fields", group: { de: "Gruppen und Aktionen", en: "Groups and actions" }, groupOrder: 31, fieldOrder: Object.keys(actionSchema).indexOf(property), purposeName: property === "id" ? "actionId" : property, requiredOverride: property === "id" || property === "label", example: property === "id" ? "resetFeature" : property === "label" ? "Reset" : property === "style" ? "secondary" : undefined });
addPage({
  id: "extension-setting-key", folder: "extended-mod-json/settings/key", navGroup: { de: "Einstellungen", en: "Settings" }, navOrder: 30, order: 1,
  titleDe: "Schlüssel `settings.<key>`", titleEn: "The `settings.<key>` key",
  summaryDe: "Der Schlüssel benennt eine Einstellung und ist zugleich der Name, den Lua zum Lesen ihres aktuellen Werts verwendet.",
  summaryEn: "The key names a setting and is also the name Lua uses to read its current value.",
  bodyDe: `# Schlüssel \x60settings.<key>\x60\n\nDer Platzhalter \x60<key>\x60 wird durch einen Namen ersetzt, den der Mod-Autor wählt. Zum Beispiel heißt \x60settings.flightSpeed\x60 eine Einstellung \x60flightSpeed\x60.\n\nErlaubt sind 1 bis 80 Buchstaben, Zahlen, Punkte, Bindestriche und Unterstriche. Groß- und Kleinschreibung zählt. Lua verwendet exakt denselben Namen: \x60shroudforge.settings.get("flightSpeed", 1.0)\x60.\n\nDieser Schlüssel ist kein globaler ShroudForge-Schlüssel. Jeder Mod hat seine eigenen Einstellungswerte.`,
  bodyEn: `# The \x60settings.<key>\x60 key\n\nThe placeholder \x60<key>\x60 is replaced by a name chosen by the mod author. For example, \x60settings.flightSpeed\x60 names a setting \x60flightSpeed\x60.\n\nThe key accepts 1 to 80 letters, numbers, dots, hyphens, and underscores. Letter case matters. Lua uses the exact same name: \x60shroudforge.settings.get("flightSpeed", 1.0)\x60.\n\nThis is not a global ShroudForge setting. Each mod has its own setting values.`,
});
addPage({
  id: "extension-link-id", folder: "extended-mod-json/links/id", navGroup: { de: "Gruppen und Aktionen", en: "Groups and actions" }, navOrder: 31, order: 10,
  titleDe: "Schlüssel `links.<id>`", titleEn: "The `links.<id>` key",
  summaryDe: "Die Link-ID entscheidet, welche bekannte Schaltfläche der Modloader anzeigt.",
  summaryEn: "The link ID selects which recognized button the Modloader displays.",
  bodyDe: `# Schlüssel \x60links.<id>\x60\n\nDie ID ist der Schlüssel innerhalb von \x60links\x60. Der Modloader zeigt nur IDs an, die in seiner Liste registriert sind. Die IDs und Reihenfolge stehen in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/modules/modloader-ui/ui/src/links/order.json).\n\nDie aktuell unterstützten IDs umfassen \x60source\x60, \x60source-github\x60, \x60source-gitlab\x60, \x60source-codeberg\x60, \x60issues\x60, \x60wiki\x60, \x60website\x60, \x60store\x60 und die \x60support-*\x60-IDs. Ein unbekannter Schlüssel kann das Schema bestehen, wird aber nicht als sichtbarer Button registriert. Die Adresse muss HTTPS verwenden.`,
  bodyEn: `# The \x60links.<id>\x60 key\n\nThe ID is the key inside \x60links\x60. Modloader displays only IDs registered by the UI. The IDs and their order are in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/modules/modloader-ui/ui/src/links/order.json).\n\nCurrently supported IDs include \x60source\x60, \x60source-github\x60, \x60source-gitlab\x60, \x60source-codeberg\x60, \x60issues\x60, \x60wiki\x60, \x60website\x60, \x60store\x60, and the \x60support-*\x60 IDs. An unknown key can pass the schema but is not registered as a visible button. The URL must use HTTPS.`,
});

const linkIds = ["source", "source-github", "source-gitlab", "source-codeberg", "issues", "wiki", "website", "store", "support-bmac", "support-patreon", "support-paypal", "support-github", "support-ko-fi", "support-open-collective", "support-other"];
for (const [index, id] of linkIds.entries()) addPage({
  id: `link-${slug(id)}`, folder: `extended-mod-json/links/${slug(id)}`, navGroup: { de: "Link-IDs", en: "Link IDs" }, navOrder: 33, order: index,
  titleDe: `Link-ID ${code(id)}`, titleEn: `Link ID ${code(id)}`,
  summaryDe: `Bekannte Modloader-Schaltfläche für ${id}.`, summaryEn: `Recognized Modloader button for ${id}.`,
  bodyDe: `# Link-ID ${code(id)}\n\nDiese ID wird in ${code("extended.mod.json → links." + id)} verwendet. Trage darunter eine HTTPS-Adresse ein. Der Modloader zeigt den Link mit dem zugehörigen Symbol und übersetzten Namen.\n\n\x60\x60\x60json\n{\n  \"links\": {\n    \"${id}\": \"https://example.com/project\"\n  }\n}\n\x60\x60\x60\n\nDie IDs und ihre Reihenfolge sind in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/modules/modloader-ui/ui/src/links/order.json) definiert.`,
  bodyEn: `# Link ID ${code(id)}\n\nUse this ID in ${code("extended.mod.json → links." + id)} and provide an HTTPS address. The Modloader displays the link with its registered icon and translated label.\n\n\x60\x60\x60json\n{\n  \"links\": {\n    \"${id}\": \"https://example.com/project\"\n  }\n}\n\x60\x60\x60\n\nThe IDs and their order are defined in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/modules/modloader-ui/ui/src/links/order.json).`,
});

const capabilities = [
  ["patch", "Game file preparation", "Vorbereitung unterstützter Spieldateien vor dem Start", "Prepare supported game files before launch"],
  ["export", "File export", "Exportfunktionen für Mod-Ausgaben", "Export functions for mod output"],
  ["runtime", "Lua runtime", "Laufzeit-Callbacks und ShroudForge-Laufzeitfunktionen", "Runtime callbacks and ShroudForge runtime functions"],
  ["runtime-register-dll", "EML DLL registration", "Registrierung einer DLL aus dem Mod-Paket im Runtime-Schritt", "Register a package-local DLL during the runtime phase"],
];
for (const [value, label, de, en] of capabilities) addPage({
  id: `capability-${value}`, folder: `mod-json/capabilities/${value}`, navGroup: { de: "Berechtigungen in mod.json", en: "mod.json capabilities" }, navOrder: 12, order: capabilities.findIndex(row => row[0] === value),
  titleDe: `Berechtigung ${code(value)}`, titleEn: `The ${code(value)} capability`, summaryDe: de, summaryEn: en,
  bodyDe: `# Berechtigung \x60${value}\x60\n\n${de}. Trage den Wert in das Feld \x60capabilities\x60 in \x60mod.json\x60 ein, wenn der Mod die Funktion wirklich nutzt.\n\n\x60\x60\x60json\n{\n  \"capabilities\": [\"${value}\"]\n}\n\x60\x60\x60\n\nDie Deklaration ist keine Erfolgsmeldung. Der Loader prüft den Mod und die jeweilige Funktion zusätzlich. Siehe [Berechtigungen in mod.json](#doc-field-capabilities).`,
  bodyEn: `# The \x60${value}\x60 capability\n\n${en}. Add the value to \x60capabilities\x60 in \x60mod.json\x60 only when the mod uses that feature.\n\n\x60\x60\x60json\n{\n  \"capabilities\": [\"${value}\"]\n}\n\x60\x60\x60\n\nDeclaring a capability does not report success. The loader checks the mod and the feature separately. See [the mod.json capabilities field](#doc-field-capabilities).`,
});

const controls = [
  ["toggle", "Schalter", "Toggle", "Zeigt einen Ein/Aus-Schalter für einen booleschen Wert.", "Displays an on/off switch for a boolean value.", true],
  ["checkbox", "Kontrollkästchen", "Checkbox", "Zeigt einen booleschen Wert als Kontrollkästchen.", "Displays a boolean value as a checkbox.", false],
  ["text", "Einzeiliges Textfeld", "Single-line text field", "Für kurze Texte und Namen.", "For short text and names.", "text"],
  ["textarea", "Mehrzeiliges Textfeld", "Multi-line text field", "Für längere Texte, die über mehrere Zeilen bearbeitet werden.", "For longer text edited across multiple lines.", "Longer text"],
  ["number", "Zahlenfeld", "Number field", "Für numerische Werte. min, max und step können den Eingabebereich begrenzen.", "For numeric values. min, max, and step can constrain input.", 1],
  ["slider", "Regler", "Slider", "Für Zahlen, die Spieler innerhalb eines Bereichs einstellen.", "For numbers players adjust within a range.", 1],
  ["select", "Auswahlliste", "Select list", "Für genau eine Auswahl aus options.", "For exactly one choice from options.", "easy"],
  ["radio", "Optionsfelder", "Radio buttons", "Zeigt alle Werte aus options, von denen genau einer aktiv ist.", "Shows every option with exactly one active choice.", "easy"],
  ["segmented", "Segmentierte Auswahl", "Segmented choices", "Zeigt Auswahlwerte als eine Reihe von Schaltflächen.", "Shows choices as a row of buttons.", "easy"],
  ["multiselect", "Mehrfachauswahl", "Multi-select", "Erlaubt mehrere Werte. value ist eine Liste und options muss die möglichen Elemente enthalten.", "Allows several values. value is a list and options defines the available items.", ["easy"]],
  ["keybind", "Tastenkürzel", "Key binding", "Für eine Tastenkombination, die der Modloader als Schlüsselwert speichert.", "For a keyboard shortcut stored by Modloader as a key value.", "F7"],
  ["color", "Farbauswahl", "Color picker", "Für einen Farbwert, der im Modloader als Farbfeld bearbeitet wird.", "For a color value edited through a color input in Modloader.", "#d0ae6d"],
];
for (const [id, titleDe, titleEn, purposeDe, purposeEn, value] of controls) addPage({
  id: `control-${id}`, folder: `extended-mod-json/controls/${id}`, navGroup: { de: "Steuerelemente", en: "Controls" }, navOrder: 32, order: controls.findIndex(row => row[0] === id),
  titleDe: `Steuerelement ${code(id)}`, titleEn: `The ${code(id)} control`, summaryDe: purposeDe, summaryEn: purposeEn,
  bodyDe: `# Steuerelement \x60${id}\x60\n\n${purposeDe} Lege es bei einer Einstellung als \x60control\x60 fest.\n\n\x60\x60\x60json\n{\n  \"settings\": {\n    \"example\": {\"value\": ${JSON.stringify(value)}, \"label\": \"${titleDe}\", \"control\": \"${id}\"${["select", "radio", "segmented", "multiselect"].includes(id) ? ", \"options\": {\"easy\": \"Einfach\", \"hard\": \"Schwer\"}" : ""}}\n  }\n}\n\x60\x60\x60\n\n${["select", "radio", "segmented", "multiselect"].includes(id) ? "Die Optionsschlüssel sind gespeicherte Werte. Bei \x60multiselect\x60 muss \x60value\x60 eine Liste sein." : "Der Wert in \x60value\x60 sollte zum Steuerelement passen."} Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).`,
  bodyEn: `# The \x60${id}\x60 control\n\n${purposeEn} Set it as \x60control\x60 on a setting.\n\n\x60\x60\x60json\n{\n  \"settings\": {\n    \"example\": {\"value\": ${JSON.stringify(value)}, \"label\": \"${titleEn}\", \"control\": \"${id}\"${["select", "radio", "segmented", "multiselect"].includes(id) ? ", \"options\": {\"easy\": \"Easy\", \"hard\": \"Hard\"}" : ""}}\n  }\n}\n\x60\x60\x60\n\n${["select", "radio", "segmented", "multiselect"].includes(id) ? "Option keys are the stored values. For \x60multiselect\x60, \x60value\x60 must be a list." : "The value in \x60value\x60 should match the selected control."} See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.`,
});

addPage({
  id: "targets", folder: "extended-mod-json/targets", navGroup: { de: "extended.mod.json", en: "extended.mod.json" }, navOrder: 20, order: 1,
  titleDe: "targets, Client und Server auswählen", titleEn: "targets, choose client and server",
  summaryDe: "targets legt fest, in welchem Spielprozess ein Mod ausgeführt wird. Client, Server oder beide sind möglich.",
  summaryEn: "targets selects the game process in which a mod runs. Client, server, or both are available.",
  bodyDe: `# \x60targets\x60, Client und Server auswählen\n\n\x60targets\x60 ist eine Liste in \x60extended.mod.json\x60. Sie steuert, in welchem Prozess der Mod geladen wird.\n\n| Wert | Wo der Mod läuft |\n| --- | --- |\n| \x60["client"]\x60 | Im lokalen Enshrouded-Spielprozess des Spielers. |\n| \x60["server"]\x60 | Im Dedicated-Server-Prozess. |\n| \x60["client", "server"]\x60 | Im Client und im Dedicated Server. |\n\n## Beispiele\n\nNur Client:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"client\"]\n}\n\x60\x60\x60\n\nNur Server:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"server\"]\n}\n\x60\x60\x60\n\nBeide Prozesse:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"client\", \"server\"]\n}\n\x60\x60\x60\n\n## Das bewirkt es nicht\n\nEin Client-Mod wird dadurch nicht automatisch mit dem Server synchronisiert. \x60targets\x60 repliziert weder Lua-Zustand noch Weltänderungen und sendet keine Steam-P2P-Anfrage. Dafür braucht der Mod eine passende Netzwerk- und Serverlogik.\n\n## Standardwerte und EML\n\nFür neue ShroudForge-Pakete sollte \x60["client"]\x60 ausdrücklich gesetzt werden. Ein EML-Paket ohne explizites \x60targets\x60 läuft auf Client und Server. Das gilt auch, wenn seine Erweiterungsdatei \x60launcher: "EML"\x60 enthält. Setze \x60targets\x60 explizit, wenn du migrierst oder ein anderes Ziel benötigst. Siehe [EML-Migration](#doc-eml-migration).\n\nFür den Multiplayer muss der Mod dort installiert sein, wo sein Code laufen soll. Siehe den [Multiplayer-Quickstart](#server).`,
  bodyEn: `# \x60targets\x60, choose client and server\n\n\x60targets\x60 is a list in \x60extended.mod.json\x60. It selects the process in which the mod is loaded.\n\n| Value | Where the mod runs |\n| --- | --- |\n| \x60["client"]\x60 | In the player's local Enshrouded game process. |\n| \x60["server"]\x60 | In the Dedicated Server process. |\n| \x60["client", "server"]\x60 | In both the client and Dedicated Server. |\n\n## Examples\n\nClient only:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"client\"]\n}\n\x60\x60\x60\n\nServer only:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"server\"]\n}\n\x60\x60\x60\n\nBoth processes:\n\n\x60\x60\x60json\n{\n  \"targets\": [\"client\", \"server\"]\n}\n\x60\x60\x60\n\n## What it does not do\n\nA client mod is not automatically synchronized with the server. \x60targets\x60 does not replicate Lua state or world edits, and it does not send a Steam P2P request. Those need suitable network and server-side logic.\n\n## Defaults and EML\n\nNew ShroudForge packages should set \x60["client"]\x60 explicitly. An EML package without explicit \x60targets\x60 runs on client and server, including an extension file with \x60launcher: "EML"\x60. Set \x60targets\x60 explicitly when migrating or when you need another target. See [EML migration](#doc-eml-migration).\n\nFor multiplayer, install the mod wherever its code needs to run. See the [multiplayer quickstart](#server).`,
});

// User-facing Modloader preferences are separate from settings packaged by a mod.
const general = loaderSchema.properties.general.properties;
const modules = loaderSchema.properties.modules.properties;
const preferencePurposes = {
  locale: ["Sprache der Modloader-Oberfläche, als gültiger Sprach-Tag.", "Language of the Modloader interface, as a valid language tag."],
  compactMode: ["Verdichtet die Modloader-Oberfläche.", "Uses a denser Modloader layout."],
  reducedMotion: ["Reduziert Bewegungs- und Übergangseffekte.", "Reduces motion and transition effects."],
  usernameMode: ["Legt fest, ob der Spielername automatisch erkannt oder manuell festgelegt wird.", "Chooses automatic player-name detection or a manual name."],
  messageNameFallback: ["Name, den Nachrichten verwenden, wenn die automatische Erkennung keinen eindeutigen Spielernamen findet.", "Name used by notices when automatic detection cannot find one player name."],
  minimumLevel: ["Niedrigste Log-Stufe, die in ShroudForge-Dateien gespeichert wird.", "Lowest log level written to ShroudForge log files."],
  enabled: ["Schaltet dieses eingebaute Loader-Modul ein oder aus.", "Enables or disables this built-in loader module."],
  serverSteamId: ["Fallback-SteamID64 eines Dedicated Servers, wenn der Client sie nicht automatisch aus dem Live-Kontext erkennt.", "Fallback SteamID64 for a Dedicated Server when the client cannot detect it from the live world context."],
  allowedClientSteamIds: ["Optionale Liste erlaubter Client-SteamID64-Werte für Server-P2P-Anfragen.", "Optional list of client SteamID64 values allowed to send server P2P requests."],
  startPage: ["Startseite, die die Modloader-Oberfläche nach dem Öffnen zeigt.", "Page shown when the Modloader interface opens."],
  toggleKey: ["Tastencode zum Öffnen und Schließen dieses Fensters.", "Key code used to open and close this window."],
  refreshMilliseconds: ["Intervall, in dem das UI den aktuellen Zustand erneut abfragt.", "Interval used by the UI to refresh its current state."],
  defaultSource: ["Logquelle, die in der Debug Console zuerst ausgewählt wird.", "Log source selected first in the Debug Console."],
  levelFilter: ["Anzeigefilter für Log-Stufen in der Debug Console.", "Display filter for log levels in the Debug Console."],
  autoScroll: ["Scrollt bei neuen Logzeilen automatisch zum Ende.", "Automatically scrolls to the newest log lines."],
  tailBytes: ["Maximale Dateimenge, die die Debug Console beim Lesen vom Log-Ende berücksichtigt.", "Maximum amount read from the end of a log file by the Debug Console."],
  continuous: ["Lässt die Laufzeitdiagnose über einzelne Schnappschüsse hinaus regelmäßig weiterlaufen.", "Keeps runtime diagnostics sampling beyond a one-time snapshot."],
  intervalMilliseconds: ["Abstand zwischen Diagnose-Schnappschüssen.", "Delay between diagnostic snapshots."],
  maximumDurationSeconds: ["Höchstdauer einer Diagnosesitzung.", "Maximum duration of a diagnostics capture."],
  slowCallbackMilliseconds: ["Schwelle, ab der ein Callback als langsam markiert wird.", "Threshold at which a callback is marked slow."],
  onlyChanges: ["Veröffentlicht wiederholte Diagnosen nur, wenn sich der Zustand ändert.", "Reports repeated diagnostics only when the state changes."],
  areas: ["Diagnosebereiche, die ein Schnappschuss aufnehmen darf.", "Diagnostic areas included in a snapshot."],
  checkMinutes: ["Wartezeit zwischen Update-Prüfungen.", "Delay between update checks."],
  channel: ["Release-Kanal, der auf Updates geprüft wird.", "Release channel used for update checks."],
  includePrereleases: ["Bezieht Vorabversionen in Update-Prüfungen ein.", "Includes pre-release versions in update checks."],
};
const pathPurposes = {
  mods: ["Ordner, in dem ShroudForge installierte Mod-Pakete sucht.", "Folder where ShroudForge searches for installed mod packages."],
  state: ["Ordner für den persistenten Loader-Zustand, darunter Aktivierungs- und Laufzeitstatus.", "Folder for persistent loader state, including enablement and runtime status."],
  logs: ["Ordner für ShroudForge- und Mod-Logdateien. Client und Dedicated Server schreiben getrennte Dateien.", "Folder for ShroudForge and mod log files. The client and Dedicated Server write separate files."],
  cache: ["Ordner für wiederverwendbare Zwischendaten.", "Folder for reusable cached data."],
  exports: ["Zielordner für Dateien, die Mods exportieren.", "Destination folder for files exported by mods."],
  updates: ["Ordner für heruntergeladene oder vorbereitete Updates.", "Folder for downloaded or staged updates."],
  ui: ["Ordner für persistente Daten und das WebView-Profil der Modloader-Oberfläche.", "Folder for persistent data and the Modloader interface WebView profile."],
  runtime: ["Ordner für Lua- und native Laufzeitdaten sowie erkannte Spielprofile.", "Folder for Lua and native runtime data and detected game profiles."],
};
const preferencePathPurposes = {
  "exports.enabled": ["Schaltet die Exportfunktionen für Mod-Ausgaben ein oder aus.", "Enables or disables mod export functions."],
  "logging.minimumLevel": ["Niedrigste Stufe, die ShroudForge in Logdateien speichert. Die Debug Console hat zusätzlich einen eigenen Anzeigefilter.", "Lowest level ShroudForge writes to log files. The Debug Console also has its own display filter."],
  "modules.debugConsole.toggleKey": ["Taste zum Öffnen oder Schließen der Debug Console. Der Standardwert 121 entspricht F10.", "Key used to open or close the Debug Console. The default value 121 is F10."],
  "modules.debugConsole.defaultSource": ["Logquelle, die beim Öffnen der Debug Console ausgewählt ist.", "Log source selected when the Debug Console opens."],
  "modules.debugConsole.levelFilter": ["Filtert sichtbare Zeilen in der Debug Console. Das ändert nicht, welche Zeilen in der Logdatei gespeichert werden.", "Filters visible rows in the Debug Console. It does not change which rows are written to the log file."],
  "modules.modloaderUi.toggleKey": ["Taste zum Öffnen oder Schließen des Modloader-Fensters. Der Standardwert 120 entspricht F9.", "Key used to open or close the Modloader window. The default value 120 is F9."],
  "modules.worldEditor.toggleKey": ["Taste zum Öffnen oder Schließen des World-Editor-Fensters. Der Standardwert 113 entspricht F2.", "Key used to open or close the World Editor window. The default value 113 is F2."],
  "modules.network.serverSteamId": ["Optionale SteamID64 als Fallback, wenn der Client die Dedicated-Server-ID nicht aus dem aktuellen Weltkontext erkennt.", "Optional SteamID64 fallback when the client cannot detect the Dedicated Server ID from the current world context."],
  "modules.network.allowedClientSteamIds": ["Optionale SteamID64-Liste für Clients, die Server-P2P-Anfragen senden dürfen. Ohne Einträge gelten authentifizierte Spieler.", "Optional SteamID64 list for clients allowed to send server P2P requests. Authenticated players are accepted when the list is empty."],
};

addPage({
  id: "modloader-settings-guide", folder: "modloader-settings", navGroup: { de: "Modloader-Einstellungen", en: "Modloader settings" }, navOrder: 60, order: 0,
  titleDe: "Modloader-Einstellungen", titleEn: "Modloader settings",
  summaryDe: "Diese Werte konfigurieren ShroudForge selbst. Sie sind von den Einstellungen eines einzelnen Mods getrennt.",
  summaryEn: "These values configure ShroudForge itself. They are separate from settings packaged by an individual mod.",
  bodyDe: `# Modloader-Einstellungen\n\nDiese Einstellungen betreffen die Modloader-Oberfläche, integrierte Module und den Speicherort ihrer Daten. Sie liegen in \x60shroudforge/config/modloader-config.json\x60. Sie sind nicht die Mod-Einstellungen aus \x60extended.mod.json\x60.\n\nDie Liste der einzelnen Felder wird aus \x60loader.schema.json\x60 und den Standardwerten generiert. Jede Einstellung hat hier eine eigene Markdown-Seite. Nicht aufgeführt sind interne Aktions- und Sitzungswerte wie \x60requestId\x60 oder reine Fensterpositionen.\n\n## Bereiche\n\n- **Allgemein**: Sprache, Benutzername und Anzeigeverhalten.\n- **Protokollierung**: Mindeststufe für gespeicherte Logs.\n- **Debug Console**: Quelle, Filter, Tastenkürzel und Leselimit.\n- **Modloader- und World-Editor-UI**: Aktivierung und Aktualisierungsverhalten.\n- **Netzwerk**: Dedicated-Server-Fallback und optionale SteamID64-Freigabeliste.\n- **Runtime-Diagnose**: Bereiche, Intervall, Dauer und Langsamkeitsschwelle.\n- **Updates**: Release-Kanal und Prüfintervall.\n- **Speicherorte**: Mods, State, Logs, Cache, Exporte und Laufzeitdaten.\n\nÄnderungen an \x60minimumLevel\x60 bestimmen, was gespeichert wird. Ein Filter in der Debug Console ändert nicht den Inhalt der Logdatei.`,
  bodyEn: `# Modloader settings\n\nThese settings configure the Modloader interface, built-in modules, and their data locations. They live in \x60shroudforge/config/modloader-config.json\x60. They are separate from mod settings in \x60extended.mod.json\x60.\n\nThe field pages are generated from \x60loader.schema.json\x60 and the defaults file. Each setting has its own Markdown page. Internal action and session values such as \x60requestId\x60 and window-position state are not presented as user preferences.\n\n## Areas\n\n- **General**: language, username, and display behavior.\n- **Logging**: minimum level for saved log entries.\n- **Debug Console**: source, filter, shortcut, and read limit.\n- **Modloader and World Editor UI**: enablement and refresh behavior.\n- **Network**: Dedicated Server fallback and optional SteamID64 allowlist.\n- **Runtime diagnostics**: areas, interval, duration, and slow-callback threshold.\n- **Updates**: release channel and check interval.\n- **Storage locations**: mods, state, logs, cache, exports, and runtime data.\n\nChanging \x60minimumLevel\x60 controls what gets saved. A Debug Console filter does not change the log file contents.`,
});

function getAt(value, keys) {
  return keys.reduce((current, key) => current && typeof current === "object" ? current[key] : undefined, value);
}
function schemaLeaves(node, prefix = [], output = []) {
  if (!node || typeof node !== "object" || !node.properties) return output;
  for (const [key, schema] of Object.entries(node.properties)) {
    const next = [...prefix, key];
    if (schema.properties) schemaLeaves(schema, next, output);
    else output.push({ path: next, schema });
  }
  return output;
}
const preferenceRoots = [
  ...schemaLeaves({ properties: general }, ["general"]),
  ...schemaLeaves(loaderSchema.properties.logging, ["logging"]),
  ...schemaLeaves(loaderSchema.properties.exports, ["exports"]),
  ...schemaLeaves(loaderSchema.properties.paths, ["paths"]),
  ...schemaLeaves({ properties: { modules } }, ["modules"]),
];
const excludedPreference = new Set(["modules.runtimeDiagnostics.requestId", "modules.debugConsole.window.position", "modules.modloaderUi.window.position"]);
for (const [index, entry] of preferenceRoots.entries()) {
  const pathText = entry.path.join(".");
  if (excludedPreference.has(pathText) || pathText.startsWith("modules.debugConsole.window.position.") || pathText.startsWith("modules.modloaderUi.window.position.")) continue;
  const key = entry.path.at(-1);
  const purpose = entry.path[0] === "paths"
    ? pathPurposes[key]
    : preferencePathPurposes[pathText] || preferencePurposes[key] || [`Konfiguriert ${code(pathText)}.`, `Configures ${code(pathText)}.`];
  let groupKey = entry.path[0];
  if (groupKey === "modules") groupKey = entry.path[1] === "updates" ? "updates" : entry.path[1];
  let groupNames = {
    general: { de: "Allgemein", en: "General" },
    logging: { de: "Protokollierung", en: "Logging" },
    exports: { de: "Exporte", en: "Exports" },
    debugConsole: { de: "Debug Console", en: "Debug Console" },
    worldEditor: { de: "World Editor UI", en: "World Editor UI" },
    network: { de: "Netzwerk", en: "Network" },
    modloaderUi: { de: "Modloader UI", en: "Modloader UI" },
    runtimeDiagnostics: { de: "Runtime-Diagnose", en: "Runtime diagnostics" },
    updates: { de: "Updates", en: "Updates" },
    paths: { de: "Speicherorte", en: "Storage paths" },
  }[groupKey] || { de: "Modloader", en: "Modloader" };
  const navOrder = ["general", "logging", "exports", "debugConsole", "modloaderUi", "worldEditor", "network", "runtimeDiagnostics", "updates", "paths"].indexOf(groupKey);
  const defaultValue = getAt(loaderDefaults, entry.path);
  const displayDefault = defaultValue === undefined ? (entry.schema.default === undefined ? (entry.schema.const ?? "nicht festgelegt") : entry.schema.default) : defaultValue;
  const defaultTextDe = entry.path[0] === "paths" && defaultValue === null
    ? "Automatisch aus dem Spielordner ermittelt"
    : `\x60${JSON.stringify(displayDefault)}\x60`;
  const defaultTextEn = entry.path[0] === "paths" && defaultValue === null
    ? "Resolved automatically from the game folder"
    : `\x60${JSON.stringify(displayDefault)}\x60`;
  const exampleValue = entry.path[0] === "paths" ? `./custom-${key}`
    : pathText === "modules.network.serverSteamId" ? "76561198000000000"
      : pathText === "modules.network.allowedClientSteamIds" ? "76561198000000000,76561198000000001"
        : defaultValue;
  const exampleObject = entry.path.reduceRight((child, pathPart) => ({ [pathPart]: child }), exampleValue);
  const exampleJson = JSON.stringify(exampleObject, null, 2);
  const networkHelpDe = pathText === "modules.network.serverSteamId"
    ? "Die ID muss aus 16 bis 20 Dezimalziffern bestehen. Leer lassen, wenn kein manueller Fallback gebraucht wird."
    : pathText === "modules.network.allowedClientSteamIds"
      ? "Trenne mehrere IDs durch Kommas oder Semikolons. Jede ID muss aus 16 bis 20 Dezimalziffern bestehen. Ein leerer Wert erlaubt authentifizierte Spieler."
      : "";
  const networkHelpEn = pathText === "modules.network.serverSteamId"
    ? "Use 16 to 20 decimal digits. Leave it empty when no manual fallback is needed."
    : pathText === "modules.network.allowedClientSteamIds"
      ? "Separate multiple IDs with commas or semicolons. Each ID must contain 16 to 20 decimal digits. An empty value allows authenticated players."
      : "";
  const type = Array.isArray(entry.schema.type) ? entry.schema.type.join(" | ") : entry.schema.type || (entry.schema.enum ? "string" : "array");
  const allowedDe = constraints(entry.schema, "de");
  const allowedEn = constraints(entry.schema, "en");
  const pageId = `preference-${slug(pathText)}`;
  const pageFolder = `modloader-settings/${slug(pathText)}`;
  const titleDe = `Einstellung ${code(pathText)}`;
  const titleEn = `Setting ${code(pathText)}`;
  addPage({
    id: pageId, folder: pageFolder, navGroup: groupNames, navOrder: 60 + Math.max(navOrder, 0), order: index + 1,
    titleDe, titleEn, summaryDe: purpose[0], summaryEn: purpose[1],
    bodyDe: `${articleIntro(titleDe, purpose[0])}\n## Wirkung\n\n${purpose[0]}\n\n## Standardwert\n\n${defaultTextDe}\n\n${entry.path[0] === "paths" ? "Ein nicht gesetzter Wert wird beim Start relativ zum Spielordner aufgelöst. Ein relativer eigener Pfad bezieht sich ebenfalls auf den Spielordner und wird danach als absoluter Pfad gespeichert. Ein absoluter Pfad bleibt erhalten." : ""}\n\n${networkHelpDe}\n\n## Datentyp und zulässige Werte\n\nTyp: \x60${type}\x60.\n\n${allowedDe}\n\n## Beispiel\n\nDies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.\n\n\x60\x60\x60json\n${exampleJson}\n\x60\x60\x60\n\n## Ort in der Konfiguration\n\n\x60shroudforge/config/modloader-config.json\x60, JSON-Pfad \x60/${entry.path.join("/")}\x60. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.\n\nQuelle: \x60src/loader/package/src/config/loader.schema.json\x60 und \x60loader.default.json\x60.`,
    bodyEn: `${articleIntro(titleEn, purpose[1])}\n## What it changes\n\n${purpose[1]}\n\n## Default\n\n${defaultTextEn}\n\n${entry.path[0] === "paths" ? "An unset value is resolved relative to the game folder at startup. A custom relative path also starts from the game folder and is then saved as an absolute path. Absolute paths are preserved." : ""}\n\n${networkHelpEn}\n\n## Type and allowed values\n\nType: \x60${type}\x60.\n\n${allowedEn}\n\n## Example\n\nThis is a partial config snippet. Keep the other entries in your file.\n\n\x60\x60\x60json\n${exampleJson}\n\x60\x60\x60\n\n## Configuration location\n\n\x60shroudforge/config/modloader-config.json\x60, JSON path \x60/${entry.path.join("/")}\x60. This configures the loader and is not copied into a mod package.\n\nSource: \x60src/loader/package/src/config/loader.schema.json\x60 and \x60loader.default.json\x60.`,
  });
}

const builtinSettingTitlesDe = {
  "sf-auto-stamina-refill": { autoRefill: "Ausdauer automatisch auffüllen" },
  "sf-production-time": { seconds: "Basis-Produktionszeit in Sekunden" },
  "world-editor": {
    panelWidth: "Breite des Blueprint-Fensters",
    panelHeight: "Höhe des Blueprint-Fensters",
    panelPositionX: "Fensterposition X, minus eins zentriert",
    panelPositionY: "Fensterposition Y ab oberem Rand",
    maximumCopyableProps: "Maximale Props pro Blueprint",
    sourceX: "Quellzelle X als manueller Ersatzwert",
    sourceY: "Quellzelle Y als manueller Ersatzwert",
    sourceZ: "Quellzelle Z als manueller Ersatzwert",
    sizeX: "Auswahlgröße X als manueller Ersatzwert",
    sizeY: "Auswahlgröße Y als manueller Ersatzwert",
    sizeZ: "Auswahlgröße Z als manueller Ersatzwert",
    blueprintName: "Dauerhafter Blueprint-Name",
    blueprintNewName: "Neuer Name beim Umbenennen oder Duplizieren",
    pasteVoxelMode: "Voxel-Modus beim Einfügen",
    targetPropMode: "Props am Einfügeziel",
    targetX: "Zielzelle X",
    targetY: "Zielzelle Y",
    targetZ: "Zielzelle Z",
    targetWorldX: "Weltposition X für Props-only-Blueprints",
    targetWorldY: "Weltposition Y für Props-only-Blueprints",
    targetWorldZ: "Weltposition Z für Props-only-Blueprints",
    rotationQuarterTurns: "Drehung vor dem Einfügen",
    rotationAxis: "Obere Blueprint-Achse",
    entityHandle: "Interner Entity-Handle",
    templateUuidHigh: "Obere Hälfte der Template-UUID in Hexadezimal",
    templateUuidLow: "Untere Hälfte der Template-UUID in Hexadezimal",
    trackingId: "Tracking-ID",
    feedbackId: "Material-Feedback-ID nur für Platzierung",
    entityX: "Entity-Position X",
    entityY: "Entity-Position Y",
    entityZ: "Entity-Position Z",
    boundsMinX: "Untere Begrenzung X",
    boundsMinY: "Untere Begrenzung Y",
    boundsMinZ: "Untere Begrenzung Z",
    boundsMaxX: "Obere Begrenzung X",
    boundsMaxY: "Obere Begrenzung Y",
    boundsMaxZ: "Obere Begrenzung Z",
  },
};
const builtinSettingPurposes = {
  "sf-auto-stamina-refill": {
    autoRefill: ["Schaltet die wiederholte Auffüllung der Ausdauer bis zum Maximum ein oder aus.", "Turns repeated stamina refill to maximum on or off."],
  },
  "sf-production-time": {
    seconds: ["Legt die Basisdauer in Sekunden für alle zeitgesteuerten Produktionsrezepte fest. Stelle denselben Wert auf Client und Host oder Server ein, bevor das Spiel startet.", "Sets the base duration in seconds for timed production recipes. Set the same value on the client and host or server before starting the game."],
  },
  "world-editor": {
    panelWidth: ["Breite des World-Editor-Fensters in logischen Pixeln. Änderungen gelten während des laufenden Spiels.", "Width of the World Editor window in logical pixels. Changes apply while the game is running."],
    panelHeight: ["Höhe des World-Editor-Fensters in logischen Pixeln. Änderungen gelten während des laufenden Spiels.", "Height of the World Editor window in logical pixels. Changes apply while the game is running."],
    panelPositionX: ["Horizontale Position relativ zum Spielfenster. Der Wert −1 zentriert das Fenster, nichtnegative Werte legen den linken Abstand fest.", "Horizontal position relative to the game window. A value of −1 centers the window, nonnegative values set its left offset."],
    panelPositionY: ["Vertikaler Abstand des World-Editor-Fensters vom oberen Rand des Spielfensters.", "Vertical offset of the World Editor window from the top of the game window."],
    maximumCopyableProps: ["Maximale Prop-Anzahl pro Blueprint. Aufnahme, Speichern, Laden und Einfügen lehnen größere Blueprints ab, statt Props still zu entfernen.", "Maximum props per blueprint. Capture, save, load, and paste reject larger blueprints instead of silently dropping props."],
    sourceX: ["Manueller X-Koordinatenwert der Quellzelle für eine Aufnahme ohne vollständig markierte Cursor-Ecken. Normalerweise die Auswahl mit F5 markieren.", "Manual X coordinate of the source cell when capturing without both cursor corners marked. Normally mark the selection with F5."],
    sourceY: ["Manueller Y-Koordinatenwert der Quellzelle für eine Aufnahme ohne vollständig markierte Cursor-Ecken. Normalerweise die Auswahl mit F5 markieren.", "Manual Y coordinate of the source cell when capturing without both cursor corners marked. Normally mark the selection with F5."],
    sourceZ: ["Manueller Z-Koordinatenwert der Quellzelle für eine Aufnahme ohne vollständig markierte Cursor-Ecken. Normalerweise die Auswahl mit F5 markieren.", "Manual Z coordinate of the source cell when capturing without both cursor corners marked. Normally mark the selection with F5."],
    sizeX: ["Manuelle Ausdehnung der Aufnahme entlang X in Voxel-Zellen. Wird nur verwendet, wenn keine vollständige Auswahl über Cursor-Ecken vorliegt.", "Manual capture extent along X in voxel cells. Used only when there is no complete selection marked by cursor corners."],
    sizeY: ["Manuelle Ausdehnung der Aufnahme entlang Y in Voxel-Zellen. Wird nur verwendet, wenn keine vollständige Auswahl über Cursor-Ecken vorliegt.", "Manual capture extent along Y in voxel cells. Used only when there is no complete selection marked by cursor corners."],
    sizeZ: ["Manuelle Ausdehnung der Aufnahme entlang Z in Voxel-Zellen. Wird nur verwendet, wenn keine vollständige Auswahl über Cursor-Ecken vorliegt.", "Manual capture extent along Z in voxel cells. Used only when there is no complete selection marked by cursor corners."],
    blueprintName: ["Standardname für das Speichern eines Blueprints und den ersten Namen in den Mod-Aktionen zum Umbenennen oder Duplizieren.", "Default name for saving a blueprint and the first name used by rename or duplicate actions."],
    blueprintNewName: ["Zielname für die Mod-Aktionen zum Umbenennen oder Duplizieren eines Blueprints.", "Destination name for the mod actions that rename or duplicate a blueprint."],
    pasteVoxelMode: ["Bestimmt, wie Voxel beim Einfügen zusammengeführt werden. `replace` ersetzt die Zielzellen, `add` fügt nur belegte Quellzellen hinzu.", "Controls how voxels are combined during paste. `replace` replaces target cells, while `add` adds only occupied source cells."],
    targetPropMode: ["Bestimmt, ob Props am Ziel erhalten bleiben oder sicher überprüfte, sich überschneidende Props ersetzt werden.", "Controls whether props at the target are kept or safely verified intersecting props are replaced."],
    targetX: ["Manuelle X-Zielkoordinate in Voxel-Zellen für ein Blueprint mit Voxel-Daten, wenn weder Cursor noch markiertes Ziel verwendet werden.", "Manual X target coordinate in voxel cells for a blueprint with voxels when neither the cursor nor a marked target is used."],
    targetY: ["Manuelle Y-Zielkoordinate in Voxel-Zellen für ein Blueprint mit Voxel-Daten, wenn weder Cursor noch markiertes Ziel verwendet werden.", "Manual Y target coordinate in voxel cells for a blueprint with voxels when neither the cursor nor a marked target is used."],
    targetZ: ["Manuelle Z-Zielkoordinate in Voxel-Zellen für ein Blueprint mit Voxel-Daten, wenn weder Cursor noch markiertes Ziel verwendet werden.", "Manual Z target coordinate in voxel cells for a blueprint with voxels when neither the cursor nor a marked target is used."],
    targetWorldX: ["Weltkoordinate X als manuelles Einfügeziel für Props-only-Blueprints, wenn weder Cursor noch markiertes Ziel verwendet werden.", "World coordinate X as the manual paste target for props-only blueprints when neither the cursor nor a marked target is used."],
    targetWorldY: ["Weltkoordinate Y als manuelles Einfügeziel für Props-only-Blueprints, wenn weder Cursor noch markiertes Ziel verwendet werden.", "World coordinate Y as the manual paste target for props-only blueprints when neither the cursor nor a marked target is used."],
    targetWorldZ: ["Weltkoordinate Z als manuelles Einfügeziel für Props-only-Blueprints, wenn weder Cursor noch markiertes Ziel verwendet werden.", "World coordinate Z as the manual paste target for props-only blueprints when neither the cursor nor a marked target is used."],
    rotationQuarterTurns: ["Anfangsdrehung des aktiven Blueprints in Vierteldrehungen. 0, 1, 2 und 3 entsprechen 0°, 90°, 180° und 270°. F3 erhöht den Wert im Spiel.", "Initial rotation of the active blueprint in quarter turns. 0, 1, 2, and 3 mean 0°, 90°, 180°, and 270°. F3 advances it in game."],
    rotationAxis: ["Obere Achse neuer Aufnahmen. Sie wird im Blueprint gespeichert, bereits gespeicherte Blueprints behalten ihre eigene Achse.", "Up axis for new captures. It is stored in the blueprint, and existing blueprints keep their own axis."],
    entityHandle: ["Laufzeit-Handle eines Props aus der Aktion List props. Es gilt nur für den aktuellen Prozess und ist für die erweiterten nativen Aktionen gedacht.", "Runtime handle for a prop returned by the List props action. It is valid only in the current process and is intended for advanced native actions."],
    templateUuidHigh: ["Obere hexadezimale Hälfte der nativen Template-UUID für die erweiterte Aktion Queue prop spawn.", "Upper hexadecimal half of the native template UUID for the advanced Queue prop spawn action."],
    templateUuidLow: ["Untere hexadezimale Hälfte der nativen Template-UUID für die erweiterte Aktion Queue prop spawn.", "Lower hexadecimal half of the native template UUID for the advanced Queue prop spawn action."],
    trackingId: ["Nichtnuller Tracking-Wert aus der ItemInfo-Platzierungsdefinition. Die erweiterten nativen Spawn- und Platzierungsaktionen benötigen ihn.", "Nonzero tracking value from the ItemInfo placement definition. The advanced native spawn and placement actions require it."],
    feedbackId: ["Material-Feedback-ID aus der ItemInfo-Platzierungsdefinition. Die erweiterte Aktion Place at coordinates benötigt sie, Destroy at coordinates nicht.", "Material feedback ID from the ItemInfo placement definition. The advanced Place at coordinates action requires it, Destroy at coordinates does not."],
    entityX: ["Weltkoordinate X für die erweiterten nativen Props-Aktionen zum Erstellen, Platzieren oder Entfernen.", "World coordinate X for the advanced native prop spawn, placement, or removal actions."],
    entityY: ["Weltkoordinate Y für die erweiterten nativen Props-Aktionen zum Erstellen, Platzieren oder Entfernen.", "World coordinate Y for the advanced native prop spawn, placement, or removal actions."],
    entityZ: ["Weltkoordinate Z für die erweiterten nativen Props-Aktionen zum Erstellen, Platzieren oder Entfernen.", "World coordinate Z for the advanced native prop spawn, placement, or removal actions."],
    boundsMinX: ["X-Minimum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "X minimum of the bounds passed to the advanced native prop placement and removal actions."],
    boundsMinY: ["Y-Minimum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "Y minimum of the bounds passed to the advanced native prop placement and removal actions."],
    boundsMinZ: ["Z-Minimum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "Z minimum of the bounds passed to the advanced native prop placement and removal actions."],
    boundsMaxX: ["X-Maximum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "X maximum of the bounds passed to the advanced native prop placement and removal actions."],
    boundsMaxY: ["Y-Maximum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "Y maximum of the bounds passed to the advanced native prop placement and removal actions."],
    boundsMaxZ: ["Z-Maximum der Begrenzung, die an die erweiterten nativen Props-Platzierungs- und Entfernungsaktionen übergeben wird.", "Z maximum of the bounds passed to the advanced native prop placement and removal actions."],
  },
};
const builtinGermanText = new Map([
  ["Repeatedly refills stamina to maximum.", "Füllt die Ausdauer wiederholt bis zum Maximum auf."],
  ["Apply the same value to client and host/server before starting the game. World speed settings multiply this base duration. Instant recipes and inventory quantities are preserved.", "Lege vor dem Spielstart denselben Wert auf Client und Host oder Server fest. Weltgeschwindigkeitseinstellungen multiplizieren diese Basisdauer. Sofortrezepte und Inventarmengen bleiben unverändert."],
  ["Copy, rotate, save, and paste parts of the world.", "Weltbereiche kopieren, drehen, speichern und einfügen."],
  ["Keeps the original stack when splitting items. Also affects other inventory operations.", "Behält beim Teilen von Gegenständen den ursprünglichen Stapel. Das wirkt sich auch auf weitere Inventaraktionen aus."],
  ["Prevents items from being consumed when used.", "Verhindert, dass Gegenstände bei der Verwendung verbraucht werden."],
  ["Prevents health loss from falling.", "Verhindert Lebenspunkteverlust durch Stürze."],
  ["Removes resource consumption in supported building, crafting, and item-use actions.", "Entfernt den Ressourcenverbrauch bei unterstützten Bau-, Herstellungs- und Verwendungsaktionen."],
  ["Sets timed production recipes to a chosen base duration. World speed settings still apply.", "Legt eine Basisdauer für zeitgesteuerte Produktionsrezepte fest. Weltgeschwindigkeitseinstellungen gelten weiterhin."],
  ["Lets you keep flying without a time limit.", "Ermöglicht unbegrenztes Fliegen."],
  ["Changes recipe unlock requirements to the first Flame Altar hint.", "Ändert die Freischaltbedingungen von Rezepten auf den ersten Hinweis am Flammenaltar."],
  ["Spawn and placement use native game operations.", "Erstellen und Platzieren verwenden native Spieloperationen."],
  ["Position is relative to the game window in logical pixels. X = −1 centers the panel. Y is measured down from the top. Changes apply while the game is running.", "Die Position bezieht sich auf das Spielfenster und wird in logischen Pixeln gemessen. X = −1 zentriert das Fenster. Y wird vom oberen Rand nach unten gemessen. Änderungen gelten während des laufenden Spiels."],
  ["Capture voxel-and-prop or props-only blueprints, preview, paste, and undo through native world APIs.", "Voxel- und Prop-Blueprints oder reine Prop-Blueprints aufnehmen, prüfen, einfügen und über native Welt-APIs rückgängig machen."],
  ["Maximum props per blueprint", "Maximale Anzahl an Props pro Blueprint"],
  ["Replace target voxels", "Voxel am Ziel ersetzen"],
  ["Add occupied source voxels", "Belegte Voxel aus der Quelle hinzufügen"],
  ["Keep existing props", "Vorhandene Props behalten"],
  ["Replace intersecting props", "Props am Ziel ersetzen, wenn sie sich überschneiden"],
  ["X axis", "X-Achse"],
  ["Y axis", "Y-Achse"],
  ["Z axis", "Z-Achse"],
]);
const builtinGroupTitlesDe = new Map([
  ["In-game panel", "Fenster im Spiel"],
  ["World Editor limits", "Grenzen des World Editors"],
  ["World Editor", "World Editor"],
  ["Native entity operations", "Native Entity-Operationen"],
  ["Stamina behavior", "Ausdauerverhalten"],
  ["Production time", "Produktionszeit"],
]);

// Each shipped mod setting gets its own page based on the checked-in package definition.
const modsRoot = path.join(root, "mods");
const modDirectories = fs.readdirSync(modsRoot, { withFileTypes: true }).filter(entry => entry.isDirectory()).sort((a, b) => a.name.localeCompare(b.name));
let builtinOrder = 0;
for (const directory of modDirectories) {
  const extensionPath = path.join(modsRoot, directory.name, "extended.mod.json");
  if (!fs.existsSync(extensionPath)) continue;
  const extension = readJson(extensionPath);
  if (!extension.settings || typeof extension.settings !== "object") continue;
  const modManifest = readJson(path.join(modsRoot, directory.name, "mod.json"));
  const modTitle = modManifest.name || titleFromKey(directory.name.replace(/^sf-/, "").replaceAll("-", " "));
  const modDescription = modManifest.description || "";
  const modDescriptionDe = builtinGermanText.get(modDescription) || modDescription;
  for (const [key, raw] of Object.entries(extension.settings)) {
    const metadata = raw && typeof raw === "object" && !Array.isArray(raw) ? raw : { value: raw };
    const settingTitle = metadata.label || titleFromKey(key);
    const settingTitleDe = builtinSettingTitlesDe[directory.name]?.[key] || settingTitle;
    const group = extension.groups?.find(entry => entry.settings?.includes(key));
    const groupDescriptionDe = group?.description ? builtinGermanText.get(group.description) || group.description : "";
    const groupPurposeDe = groupDescriptionDe ? ` ${groupDescriptionDe}` : group ? ` Sie gehört zur Gruppe ${code(builtinGroupTitlesDe.get(group.label) || group.label)}.` : "";
    const groupPurposeEn = group?.description ? ` ${group.description}` : group ? ` It belongs to the ${code(group.label)} group.` : "";
    const specificPurpose = builtinSettingPurposes[directory.name]?.[key];
    const purposeDe = metadata.description || specificPurpose?.[0] || `Legt den Wert für ${code(settingTitleDe)} im Mod ${code(modTitle)} fest.${groupPurposeDe}`;
    const purposeEn = metadata.description || specificPurpose?.[1] || `Sets the value for ${code(settingTitle)} in the ${code(modTitle)} mod.${groupPurposeEn}`;
    const id = `builtin-${slug(directory.name)}-${slug(key)}`;
    const folder = `built-in-mods/${directory.name}/${slug(key)}`;
    const groupIndex = group ? extension.groups.indexOf(group) : 0;
    const groupLabelDe = group ? builtinGermanText.get(group.label) || builtinGroupTitlesDe.get(group.label) || group.label : "Einstellungen";
    const navGroup = { de: `Eingebaute Mods · ${modTitle} · ${groupLabelDe}`, en: `Built-in mods · ${modTitle} · ${group?.label || "Settings"}` };
    const value = Object.hasOwn(metadata, "value") ? metadata.value : null;
    const valueType = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
    const acceptedDe = metadata.options
      ? `Erlaubte Werte: ${Object.entries(metadata.options).map(([option, label]) => `${code(option)} für ${builtinGermanText.get(label) || label}`).join(", ")}.`
      : typeof value === "boolean"
        ? "Erlaubte Werte: `true` oder `false`."
        : typeof value === "number" && (metadata.min !== undefined || metadata.max !== undefined)
          ? `Zahlenbereich: ${metadata.min ?? "kein Minimum"} bis ${metadata.max ?? "kein Maximum"}.`
          : typeof value === "number"
            ? "Zahlenwert. Das Mod-Paket legt keine weitere Grenze fest."
            : typeof value === "string" && (metadata.minLength !== undefined || metadata.maxLength !== undefined)
              ? `Textlänge: ${metadata.minLength ?? 0} bis ${metadata.maxLength ?? "unbegrenzt"} Zeichen.`
              : typeof value === "string"
                ? "Textwert. Das Mod-Paket legt keine weitere Längenbegrenzung fest."
                : "Der Werttyp und die erlaubten Inhalte richten sich nach diesem gespeicherten Standardwert.";
    const acceptedEn = metadata.options
      ? `Allowed values: ${Object.entries(metadata.options).map(([option, label]) => `${code(option)} for ${label}`).join(", ")}.`
      : typeof value === "boolean"
        ? "Allowed values: `true` or `false`."
        : typeof value === "number" && (metadata.min !== undefined || metadata.max !== undefined)
          ? `Number range: ${metadata.min ?? "no minimum"} to ${metadata.max ?? "no maximum"}.`
          : typeof value === "number"
            ? "Numeric value. The mod package declares no additional limit."
            : typeof value === "string" && (metadata.minLength !== undefined || metadata.maxLength !== undefined)
              ? `Text length: ${metadata.minLength ?? 0} to ${metadata.maxLength ?? "unlimited"} characters.`
              : typeof value === "string"
                ? "Text value. The mod package declares no additional length limit."
                : "The value type and accepted content follow the saved default value.";
    const readmePath = path.join(modsRoot, directory.name, "README.md");
    const readmeLinkDe = fs.existsSync(readmePath)
      ? `\n\n[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/README.md)`
      : "";
    const readmeLinkEn = fs.existsSync(readmePath)
      ? `\n\n[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/README.md)`
      : "";
    const detailsDe = [
      `Mod: ${modTitle} (${code(directory.name)}).`,
      `Schlüssel für ${code(`shroudforge.settings.get("${key}", fallback)`)}: ${code(key)}.`,
      `Datentyp: ${code(valueType)}.`,
      `Startwert: ${code(JSON.stringify(value))}.`,
      acceptedDe,
      metadata.control ? `Steuerelement: ${code(metadata.control)}.` : "Das Steuerelement wird aus dem Werttyp abgeleitet.",
      metadata.min !== undefined ? `Minimum: ${code(metadata.min)}.` : "",
      metadata.max !== undefined ? `Maximum: ${code(metadata.max)}.` : "",
      metadata.step !== undefined ? `Schritt: ${code(metadata.step)}.` : "",
      metadata.options ? `Optionen: ${Object.entries(metadata.options).map(([option, label]) => `${code(option)} (${builtinGermanText.get(label) || label})`).join(", ")}.` : "",
    ].filter(Boolean).map(item => `- ${item}`).join("\n");
    const detailsEn = [
      `Mod: ${modTitle} (${code(directory.name)}).`,
      `Key read with ${code(`shroudforge.settings.get("${key}", fallback)`)}: ${code(key)}.`,
      `Type: ${code(valueType)}.`,
      `Starting value: ${code(JSON.stringify(value))}.`,
      acceptedEn,
      metadata.control ? `Control: ${code(metadata.control)}.` : "The control is inferred from the value type.",
      metadata.min !== undefined ? `Minimum: ${code(metadata.min)}.` : "",
      metadata.max !== undefined ? `Maximum: ${code(metadata.max)}.` : "",
      metadata.step !== undefined ? `Step: ${code(metadata.step)}.` : "",
      metadata.options ? `Options: ${Object.entries(metadata.options).map(([option, label]) => `${code(option)} (${label})`).join(", ")}.` : "",
    ].filter(Boolean).map(item => `- ${item}`).join("\n");
    addPage({
      id, folder, navGroup, navOrder: 80 + modDirectories.findIndex(item => item.name === directory.name) * 10 + Math.max(groupIndex, 0), order: builtinOrder++,
      titleDe: `${settingTitleDe} · ${key}`, titleEn: `${settingTitle} · ${key}`,
      summaryDe: purposeDe, summaryEn: purposeEn,
      bodyDe: `# ${settingTitleDe}\n\n${purposeDe}\n\n## Bedeutung und Standard\n\n${modDescriptionDe ? `${modDescriptionDe}\n\n` : ""}${detailsDe}\n\n## Woher die Angaben kommen\n\nDiese Seite basiert auf \x60mods/${directory.name}/extended.mod.json\x60. Lua liest den aktuellen Wert mit \x60shroudforge.settings.get("${key}", fallback)\x60. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/src/mod.lua).${readmeLinkDe}`,
      bodyEn: `# ${settingTitle}\n\n${purposeEn}\n\n## Meaning and default\n\n${modDescription ? `${modDescription}\n\n` : ""}${detailsEn}\n\n## Source of this information\n\nThis page is based on \x60mods/${directory.name}/extended.mod.json\x60. Lua reads the current value with \x60shroudforge.settings.get("${key}", fallback)\x60. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/src/mod.lua).${readmeLinkEn}`,
    });
  }
}

const modIndexRowsDe = [];
const modIndexRowsEn = [];
for (const [modIndex, directory] of modDirectories.entries()) {
  const manifestPath = path.join(modsRoot, directory.name, "mod.json");
  if (!fs.existsSync(manifestPath)) continue;
  const manifest = readJson(manifestPath);
  const title = manifest.name || directory.name;
  const description = manifest.description || "";
  const descriptionDe = builtinGermanText.get(description) || description;
  const extensionPath = path.join(modsRoot, directory.name, "extended.mod.json");
  const extension = fs.existsSync(extensionPath) ? readJson(extensionPath) : {};
  const settingEntries = Object.entries(extension.settings || {});
  const modGuideId = `builtin-mod-${slug(directory.name)}`;
  const modGuideFolder = `built-in-mods/${directory.name}`;
  const settingLink = (key, raw, locale) => {
    const metadata = raw && typeof raw === "object" && !Array.isArray(raw) ? raw : {};
    const label = locale === "de"
      ? builtinSettingTitlesDe[directory.name]?.[key] || metadata.label || titleFromKey(key)
      : metadata.label || titleFromKey(key);
    return `[${label}](#doc-builtin-${slug(directory.name)}-${slug(key)})`;
  };
  const groupedKeys = new Set();
  const settingSectionsDe = [];
  const settingSectionsEn = [];
  for (const group of extension.groups || []) {
    const entries = (group.settings || []).filter(key => Object.hasOwn(extension.settings || {}, key));
    if (!entries.length) continue;
    entries.forEach(key => groupedKeys.add(key));
    const groupLabelDe = builtinGermanText.get(group.label) || builtinGroupTitlesDe.get(group.label) || group.label;
    settingSectionsDe.push(`## ${groupLabelDe}\n\n${(group.description ? `${builtinGermanText.get(group.description) || group.description}\n\n` : "")}${entries.map(key => `- ${settingLink(key, extension.settings[key], "de")}`).join("\n")}`);
    settingSectionsEn.push(`## ${group.label}\n\n${group.description ? `${group.description}\n\n` : ""}${entries.map(key => `- ${settingLink(key, extension.settings[key], "en")}`).join("\n")}`);
  }
  const ungrouped = settingEntries.map(([key]) => key).filter(key => !groupedKeys.has(key));
  if (ungrouped.length) {
    settingSectionsDe.push(`## Weitere Einstellungen\n\n${ungrouped.map(key => `- ${settingLink(key, extension.settings[key], "de")}`).join("\n")}`);
    settingSectionsEn.push(`## Other settings\n\n${ungrouped.map(key => `- ${settingLink(key, extension.settings[key], "en")}`).join("\n")}`);
  }
  const targetValues = Array.isArray(extension.targets) ? extension.targets : extension.launcher === "EML" ? ["client", "server"] : ["client"];
  const targetTextDe = targetValues.map(value => value === "client" ? "Client" : "Dedicated Server").join(" und ");
  const targetTextEn = targetValues.map(value => value === "client" ? "client" : "Dedicated Server").join(" and ");
  addPage({
    id: modGuideId, folder: modGuideFolder,
    navGroup: { de: `Eingebaute Mods · ${title}`, en: `Built-in mods · ${title}` },
    navOrder: 80 + modIndex * 10, order: 0,
    titleDe: title, titleEn: title,
    summaryDe: descriptionDe || `Übersicht des Mods ${title} und seiner Einstellungen.`,
    summaryEn: description || `Overview of ${title} and its settings.`,
    bodyDe: `# ${title}\n\n${descriptionDe}\n\n## Paketangaben\n\n- Mod-ID: \x60${manifest.id}\x60\n- Zielprozesse: ${targetTextDe}\n- Laufzeitberechtigung: ${manifest.capabilities?.includes("runtime") ? "ja" : "nein"}\n- Deklarierte spielerseitige Einstellungen: ${settingEntries.length}\n\n## Einstellungen\n\n${settingSectionsDe.join("\n\n") || "Dieser Mod deklariert keine spielerseitigen Einstellungen."}\n\nDie Einstellungen werden in \x60extended.mod.json\x60 des Mod-Pakets gespeichert. Der Mod liest sie über \x60shroudforge.settings.get(key, fallback)\x60. Weitere Details stehen in der [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/src/mod.lua).`,
    bodyEn: `# ${title}\n\n${description}\n\n## Package details\n\n- Mod ID: \x60${manifest.id}\x60\n- Target processes: ${targetTextEn}\n- Runtime capability: ${manifest.capabilities?.includes("runtime") ? "yes" : "no"}\n- Declared player settings: ${settingEntries.length}\n\n## Settings\n\n${settingSectionsEn.join("\n\n") || "This mod does not declare player-facing settings."}\n\nSettings are stored in the mod package's \x60extended.mod.json\x60. The mod reads them with \x60shroudforge.settings.get(key, fallback)\x60. See the [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/${directory.name}/src/mod.lua) for implementation details.`,
  });
  const settingCountDe = settingEntries.length
    ? `${settingEntries.length} ${settingEntries.length === 1 ? "Einstellungsseite" : "Einstellungsseiten"}`
    : "keine spielerseitigen Einstellungen";
  const settingCountEn = settingEntries.length
    ? `${settingEntries.length} ${settingEntries.length === 1 ? "setting page" : "setting pages"}`
    : "no player settings";
  modIndexRowsDe.push(`| [${title}](#doc-${modGuideId}) | ${descriptionDe} | ${settingCountDe} |`);
  modIndexRowsEn.push(`| [${title}](#doc-${modGuideId}) | ${description} | ${settingCountEn} |`);
}
addPage({
  id: "builtin-mods-guide", folder: "built-in-mods", navGroup: { de: "Eingebaute Mods", en: "Built-in mods" }, navOrder: 79, order: 0,
  titleDe: "Eingebaute Mods und ihre Einstellungen", titleEn: "Built-in mods and their settings",
  summaryDe: "Übersicht der mit ShroudForge ausgelieferten Mods und ihrer einzeln erklärten Einstellungen.",
  summaryEn: "Overview of the mods shipped with ShroudForge and their individually documented settings.",
  bodyDe: `# Eingebaute Mods und ihre Einstellungen\n\nDiese Mods liegen im Repository unter \x60mods/\x60. Öffne eine Mod-Seite, um Zielprozesse, Zweck und Einstellungsgruppen zu sehen. Jede Einstellung besitzt eine eigene Seite mit Datentyp, Standardwert und erlaubten Werten. Mod-Einstellungen liegen in \x60extended.mod.json\x60, die allgemeinen Modloader-Einstellungen sind davon getrennt.\n\n| Mod | Wirkung | Einstellungen |\n| --- | --- | --- |\n${modIndexRowsDe.join("\n")}`,
  bodyEn: `# Built-in mods and their settings\n\nThese mods live in the repository's \x60mods/\x60 folder. Open a mod page to see its purpose, target processes, and setting groups. Every setting has its own page with type, default, and allowed values. Mod settings live in \x60extended.mod.json\x60 and are separate from general Modloader preferences.\n\n| Mod | What it does | Settings |\n| --- | --- | --- |\n${modIndexRowsEn.join("\n")}`,
});

console.log(`Created ${pages.length} Markdown page folders from the checked-in schemas and settings.`);

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}
