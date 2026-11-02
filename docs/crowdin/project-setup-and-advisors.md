# Crowdin-Projekt: Einrichtung und Advisors

Diese Seite sammelt die Einrichtung des Crowdin-Projekts ShroudForge, die empfohlenen Branch-Einstellungen und die Maßnahmen zu den Context Advisors. Die Textbausteine sind zum Einfügen in Crowdin gedacht.

## Projektstand

Die Angaben stammen aus den Crowdin-Bildschirmtexten und Screenshots, die im Gespräch geteilt wurden, sowie aus der aktuellen `crowdin.yml` im Repository. Sie bestätigen nicht, dass jede Einstellung in Crowdin bereits gespeichert oder die GitHub-Synchronisierung erfolgreich abgeschlossen wurde.

| Feld | Stand beziehungsweise Empfehlung |
| --- | --- |
| Crowdin-Projekt | Shroudforge, Projekt-ID `938941`, [Projekt öffnen](https://crowdin.com/project/shroudforge) |
| GitHub-Repository | `bonsaibauer/shroudforge` |
| Projektart | File-based Project |
| Integrationsmodus | Source and translation files |
| Ausgangssprache | Englisch (UK) |
| Zielsprachen | Laut geteiltem Dashboard-Stand: Deutsch, Französisch und Spanisch |
| Übersetzungsdatei | `modules/ui-resources/locales/en-GB.json` |
| Zieldateimuster | `modules/ui-resources/locales/%two_letters_code%.json` |
| Crowdin-Konfiguration | `crowdin.yml` im Repository-Hauptordner und in jedem zu übersetzenden Branch |

Der geteilte Dashboard-Stand zeigte für die Sprachen noch `0 words`. Der Advisor erkannte zugleich eine Datei. Deshalb zuerst prüfen, ob die Integration den richtigen GitHub-Branch und dessen `crowdin.yml` synchronisiert hat. Erst danach lassen sich die Advisor-Zähler als vollständig interpretieren.

Das Repository bildet Englisch (US) in `crowdin.yml` auf `en-US.json` ab. Der geteilte Dashboard-Stand bestätigt nicht, ob Englisch (US) als Crowdin-Zielsprache aktiviert ist.

## Öffentliche Crowdin-Seite für Mitwirkende

Der direkte Projektlink ist [crowdin.com/project/shroudforge](https://crowdin.com/project/shroudforge). Damit andere das Projekt finden und beitreten können, muss die Projekt-Sichtbarkeit in Crowdin auf **Public** beziehungsweise **Open** stehen. **Private** bedeutet, dass nur eingeladene Personen Zugang erhalten. Falls Crowdin zusätzlich eine Sprachzugriffsrichtlinie anzeigt, legt diese fest, ob Mitwirkende alle Zielsprachen sehen oder den Zugang zu einzelnen Sprachen separat beantragen müssen.

Den öffentlichen Link können Mitwirkende direkt öffnen oder das Projekt über Crowdins Projektsuche finden. Nach einer Änderung der Sichtbarkeit den Link in einem abgemeldeten Browserfenster prüfen. [Crowdin: Projekte für Übersetzende finden und ihnen beitreten](https://support.crowdin.com/for-translators/)

Die derzeitige Repository-Konfiguration lautet:

```yaml
project_id_env: CROWDIN_PROJECT_ID
api_token_env: CROWDIN_PERSONAL_TOKEN
base_path: .
preserve_hierarchy: true

files:
  - source: /modules/ui-resources/locales/en-GB.json
    translation: /modules/ui-resources/locales/%two_letters_code%.json
    languages_mapping:
      two_letters_code:
        de-AT: de-AT
        de-CH: de-CH
        en-US: en-US
    update_option: update_as_unapproved
```

Die Zugangsdaten bleiben in Umgebungsvariablen und gehören nicht in die Datei.

## GitHub-Integration und Versionsbranches

Empfohlene Werte für die Integration:

| Einstellung | Auswahl |
| --- | --- |
| Repository | `bonsaibauer/shroudforge` |
| Branches for Translation | Den aktiven Release-Branch auswählen, zum Beispiel `1.7.0`, sofern dieser Branch auf GitHub liegt |
| Duplicate strings | **Show within a version branch (strict detection)**. Die JSON-Datei verwendet Schlüssel als String-IDs. |
| Service branch name | Standardpräfix `l10n_` beibehalten |
| Import bestehender Übersetzungen | **One-time translation import after the branch is connected** für den ersten Import auswählen |
| Always import new translations | Aus lassen, wenn Crowdin nach dem initialen Import die Quelle der Übersetzungen ist. Bei später neu im Repository angelegten Sprachdateien einmalig **Sync Translations to Crowdin** verwenden. |
| Push Sources | Aus lassen. Die englischen Quelltexte werden im Repository gepflegt. |
| Sync Schedule | Aktiviert lassen. Eine Stunde ist als Startwert in Ordnung. |
| Branches to Sync Automatically | `*.*.*` verwenden, wenn Releasebranches ausschließlich genau dem Schema `1.7.0`, `1.7.1` und `1.8.0` folgen. Bei weiteren Branch-Namensformen ein engeres Muster festlegen. |
| Configuration File Default Name | `crowdin.yml` |

Der Screenshot zeigte `1.7.0` in der Branch-Auswahl. Das ist ein gezeigter Stand, keine Bestätigung, dass die Integration anschließend gespeichert wurde. Die Branch-Regel `*.*.*` ist nur für Branches gedacht, deren Namen Versionsnummern sind. Crowdin benötigt den Branch zuerst auf GitHub und darin die Konfiguration im Repository-Hauptordner.

Crowdin legt für Übersetzungen einen Service-Branch an und erstellt daraus einen Pull Request zurück zum verbundenen Quellbranch. Übersetzungen aus `1.7.1` gehören dadurch in den Releasebranch `1.7.1`. Ein neuer Releasebranch wird nicht automatisch zum Hauptbranch. Historische Versionen müssen nur verbunden werden, wenn sie weiterhin separat übersetzt oder gepflegt werden.

Crowdin empfiehlt den einmaligen Import bestehender Repository-Übersetzungen. Danach soll Crowdin als Quelle für Übersetzungen dienen, damit nicht bestätigte Übersetzungen nicht durch neu importierte Repository-Dateien überschrieben werden. Die Repository-Integration synchronisiert Quelltextänderungen unabhängig vom stündlichen Übersetzungstakt fortlaufend. [Crowdin: GitHub-Integration](https://support.crowdin.com/github-integration/) · [Crowdin: Versionsverwaltung](https://support.crowdin.com/version-management/)

## Projektbeschreibung für Crowdin

Das Feld **General settings → Description** ist laut Advisor leer. Diesen englischen Text einfügen:

> ShroudForge is a Windows mod loader for Enshrouded. This project localizes the ShroudForge user interface into German, French, and Spanish. Translations should be clear, concise, and natural for players and mod creators. Preserve product names, mod names, technical identifiers, filenames, placeholders, and code as specified in the project glossary and style guide.

## Dateikontext für die Locale-Datei

Der Advisor meldet fehlenden Dateikontext für eine Datei. In **Sources → Files** die Beschreibung für `modules/ui-resources/locales/en-GB.json` ergänzen. Dieser englische Text kann direkt eingefügt werden:

> This file contains user-facing text for the ShroudForge application, including navigation, buttons, settings, mod management, compatibility information, update messages, in-app news, and modding guidance. Translate each string according to its UI context and keep labels concise. Preserve product and mod names, filenames, code, placeholders such as `{count}`, `{version}`, and `{username}`, and other technical identifiers.

Dateikontext erklärt den Übersetzenden, wo und wie die Strings verwendet werden. Er ersetzt weder Screenshots noch individuelle Kommentare zu mehrdeutigen Strings.

## Stilrichtlinie für Crowdin

Unter **Style Guides** eine Richtlinie für das Projekt anlegen und den Zielsprachen zuweisen. Folgenden Text verwenden:

> **Product and audience**
> ShroudForge is a technical Windows mod loader for Enshrouded. The audience includes players and mod creators. Write concise, calm, helpful UI text. Be technically precise without sounding formal or promotional. Do not add claims or information that is not present in the source.
>
> **Address and register**
> German: address the user as “du”. French: address the user as “vous”. Spanish: address the user as “tú”. Keep the chosen form consistent, including help, errors, confirmations, and news.
>
> **Names and technical content**
> Follow the project glossary for product names and established terms. Do not translate code, JSON keys, filenames, file paths, API names, identifiers, placeholders, or markup. Preserve spelling, capitalization, punctuation, and placeholder syntax. Never translate “Flameborn”. Keep “EML-Mod”, “SF-Mod”, “Asset-Mod”, “Client-Runtime”, “Client”, and “Server” exactly as written when they are product labels or category names.
>
> **Meaning and UI behavior**
> Translate for the actual control or message, not word-for-word. Keep “Save”, “Reload”, and “Refresh” distinct: saving persists changes, reloading loads content again, and refreshing updates the displayed state. Translate each action with the appropriate target-language verb. Preserve warnings, uncertainty, and emphasis from the source.
>
> **Locale conventions**
> Use natural spelling, accents, capitalization, and punctuation for each target language. Keep short buttons and labels concise, but do not omit meaning just to shorten them. For French, use French spacing and punctuation conventions. For German, retain umlauts and ß. For Spanish, retain accents and inverted question and exclamation marks where required.

Diese Richtlinie übernimmt die im Projekt sichtbare Anrede: Deutsch mit „du“, Französisch mit „vous“ und Spanisch mit „tú“. Sie beschreibt die vorhandene Terminologieprüfung, ist aber kein vollständiges Audit aller Locale-Strings.

## Glossar: unverändert lassen

Der Advisor zeigt ein zugewiesenes Glossar, aber null Glossarbegriffe. Das vorhandene Glossar prüfen und die folgenden Begriffe einzeln eintragen. In Crowdin je Eintrag die Zielsprachen und die passende Option **Do not translate** oder eine nicht übersetzbare Variante verwenden. Nur Begriffe, die im Quelltext vorkommen oder als feste Produktnamen gebraucht werden, aufnehmen.

### Produktnamen, Kategorien und feste technische Bezeichnungen

| Quellbegriff | Regel |
| --- | --- |
| `ShroudForge` | Produktname, exakt beibehalten |
| `ShroudEdit` | Produktname, exakt beibehalten |
| `Enshrouded` | Spielname, exakt beibehalten |
| `Flameborn` | Spielbezeichnung, exakt beibehalten |
| `EML` | Akronym, exakt beibehalten |
| `EML-Mod` | Kategoriename, exakt beibehalten |
| `SF-Mod` | Kategoriename, exakt beibehalten |
| `Asset-Mod` | Kategoriename, exakt beibehalten |
| `Client-Runtime` | Kategoriename, exakt beibehalten |
| `Client` | Feste technische Kategorie, exakt beibehalten |
| `Server` | Feste technische Kategorie, exakt beibehalten |
| `kfc-parser` | Modulname, exakt beibehalten |
| `Shroudtopia` | Eigenname, exakt beibehalten |
| `Brabb3l` | Eigenname, exakt beibehalten |
| `Lua` | Sprachname, exakt beibehalten |
| `MODLOADER` | Akronym beziehungsweise festes Label, Großschreibung beibehalten |
| `mod.json` | Dateiname, exakt beibehalten |
| `extended.mod.json` | Dateiname, exakt beibehalten |
| `SteamID64` | Technischer Bezeichner, exakt beibehalten |
| `API` | Akronym, exakt beibehalten |

### Plattformen und Modnamen

| Quellbegriff | Regel |
| --- | --- |
| `GitHub`, `GitLab`, `Codeberg` | Plattformnamen beibehalten |
| `Steam`, `Discord` | Plattformnamen beibehalten |
| `GitHub Sponsors`, `Open Collective`, `Patreon` | Dienstnamen beibehalten |
| `PayPal`, `Ko-fi` | Dienstnamen beibehalten |
| `Auto Stamina Refill` | Modname beibehalten |
| `Unlimited Flight` | Modname beibehalten |
| `Infinite Item Split` | Modname beibehalten |
| `Infinite Item Use` | Modname beibehalten |
| `No Fall Damage` | Modname beibehalten |
| `No Resource Cost` | Modname beibehalten |
| `Production Time` | Modname beibehalten |
| `Unlock Blueprints` | Modname beibehalten |
| `World Editor` | Modname beibehalten, sofern es den genannten Mod bezeichnet |
| `Hello Ember` | Fester Name im Produkt, beibehalten |

`mod` ist dagegen kein Markenname. Als etablierter Begriff bleibt die Wortform erhalten: Englisch `mod/mods`, Deutsch `Mod/Mods`, Französisch `mod/mods`, Spanisch `mod/mods`. Keine Ersetzung durch `modification` oder `modificación` vornehmen. Deutsche Großschreibung richtet sich nach der deutschen Rechtschreibung.

### Fachbegriffe mit Übersetzung

| Englisch | Deutsch | Französisch | Spanisch | Verwendung |
| --- | --- | --- | --- | --- |
| parser | Parser | parseur | analizador sintáctico | Parser-Komponente, nicht allgemein „Analysator“ |
| callback function | Callback-Funktion, in knappen technischen Labels auch Callback | fonction de rappel | función de devolución de llamada | Nie das mehrdeutige französische Einzelwort `rappel` für die Funktion verwenden |
| permission, wenn ein Mod eine Berechtigung anfordert | Berechtigung | autorisation | permiso, Plural `permisos` | Nicht mit allgemeinem Kontozugriff verwechseln |
| runtime, als technischer Laufzeitbegriff | Laufzeit oder Runtime je nach Satz | runtime | runtime | Im bestehenden Produktkontext verwenden, nicht pauschal ersetzen |
| Flame Altar | Flammenaltar | Autel de la Flamme | Altar de la Llama | Spielbegriff lokalisieren. Anders als `Flameborn` ist dies kein unveränderter Eigenname. |

Die UI-Aktionen **Save**, **Reload** und **Refresh** sind keine Glossarbegriffe mit fester englischer Form. Sie müssen lokalisiert und semantisch getrennt werden. Beispielsweise sind „neu laden“ und „aktualisieren“, `recharger` und `actualiser`, sowie `recargar` und `actualizar` nicht austauschbar. Bei zweifelhaften Strings den sichtbaren Bildschirm und die tatsächliche Aktion als Kontext verwenden.

## Produktnamen aus der Rechtschreibprüfung ausnehmen

In Crowdin wurden `Enshrouded` und `ShroudForge` in deutschen Übersetzungen als Rechtschreibfehler markiert. Das sind korrekte Eigennamen. Die Meldung ist ein Rechtschreibprüfungs-Hinweis, kein falscher Übersetzungstext.

Diese Wörter in Crowdin unter **Settings → QA Checks → Spellcheck Ignore List** eintragen. Die Ignorierliste ist getrennt vom Glossar. So bleibt die Rechtschreibprüfung aktiv und meldet weiterhin echte Tippfehler. Bei Meldungen in weiteren Zielsprachen dieselben unveränderten Namen auch dort zur jeweiligen Ignorierliste hinzufügen. [Crowdin: QA Checks und Spellchecker Ignore List](https://support.crowdin.com/project-settings/qa-checks/)

## Die sechs Context Advisors

Die Zahlen unten sind der beim Teilen angezeigte Dashboard-Stand. Crowdin-Berechnungen können sich nach der Synchronisierung ändern.

### 1. Missing file-level context

**Angezeigter Befund:** 0 Prozent Abdeckung, eine Datei ohne Kontext, eine Datei insgesamt.

**Was die Meldung bedeutet:** Crowdin hat für die Quelldatei noch keine Datei-Metadaten im Feld **Context**. Das ist keine fehlende Übersetzung und auch kein Fehler im JSON.

**So behebst du es:** In Crowdin **Sources → Files** öffnen, `modules/ui-resources/locales/en-GB.json` auswählen und die Dateieinstellungen beziehungsweise **File Settings → Context** öffnen. Den Text aus „Dateikontext für die Locale-Datei“ einfügen und speichern. Danach kontrollieren, ob der Advisor die Datei als beschrieben zählt. Crowdin verwendet diesen Kontext als Orientierung für alle Strings dieser Datei.

### 2. Missing character limits

**Was die Meldung bedeutet:** Crowdin hat 569 kurze Quellstrings ohne Feld **Max. length** gefunden. Das ist ein Coverage-Hinweis, kein Übersetzungsfehler. Das Limit ist Metadatum pro String. Crowdin zeigt es Übersetzenden an und kann Übersetzungen markieren, die länger als der eingetragene Wert sind. Die Advisor-Schwelle von 20 Prozent ist eine Empfehlung, keine technische Voraussetzung.

**So setzt du ein Limit:** In Crowdin **Sources → Strings** den betreffenden String öffnen, **Edit** wählen und bei **Max. length** entweder eine feste Zeichenzahl oder einen proportionalen Wert auswählen. Dasselbe Feld ist im Editor über **Edit String → Max. length** verfügbar.

**Welche Strings ein Limit bekommen sollten:** Nur Texte, für die das Layout tatsächlich einen festen Platz vorgibt, zum Beispiel bestimmte Buttons, schmale Labels oder feste Überschriften. News, Beschreibungen und andere frei umbrechende Texte brauchen normalerweise kein künstliches Limit. Erst die zugehörige Ansicht und das verfügbare Layout prüfen, dann einen Wert festlegen. Die 569 Einträge nicht pauschal mit derselben Zahl versehen. AI- oder Bulk-Vorschläge allenfalls als Kandidaten prüfen.

**Praktische Priorität:** Erst Dateikontext speichern. Danach bei den Zeichenlimits mit sichtbaren, platzkritischen Bedienelementen anfangen. Das Ziel ist, echte Überläufe in Deutsch, Englisch (US), Französisch und Spanisch zu verhindern, nicht bloß den Advisor-Zähler zu erhöhen.

### 3. Low screenshot coverage

**Angezeigter Befund:** 0 Prozent der UI-Strings sind Screenshots zugeordnet.

**Maßnahme:** Screenshots aus der tatsächlichen Anwendung hochladen und passende Strings den Screenshots zuordnen. Als erste Screens eignen sich Hauptnavigation, Modliste und Moddetails, Einstellungen mit Save/Reload, Kompatibilitätsansicht, Updateansicht und News. Keine Bildschirmbilder aus einer anderen Version verwenden, wenn sich Beschriftungen oder Bedienabläufe unterscheiden.

### 4. No style guide connected

**Angezeigter Befund:** Keine Stilrichtlinie mit dem Projekt verbunden.

**Maßnahme:** Die Stilrichtlinie aus dem gleichnamigen Abschnitt in Crowdin **Style Guides** anlegen und dem ShroudForge-Projekt sowie Deutsch, Englisch (US), Französisch und Spanisch zuweisen. Die Anredeformen `du`, `you` und `tú` sind dabei ausdrücklich festgelegt.

### 5. Insufficient terminology

**Angezeigter Befund:** Ein Glossar zugewiesen, null Einträge. Crowdin nennt 32 Begriffe als empfohlenen Mindestwert für dieses Projekt. Es werden keine weiteren Glossare zur Auswahl angezeigt.

**Maßnahme:** Das bereits zugewiesene Glossar bearbeiten und die tatsächlich vorkommenden Begriffe aus den beiden Glossar-Tabellen aufnehmen. Die Liste enthält mehr als 32 Kandidaten, aber sie sollen einzeln auf tatsächliche Verwendung geprüft werden. Produktnamen und Code-Bezeichner als nicht übersetzbar markieren. Allgemeine Fachbegriffe mit den genannten Zielsprachenformen eintragen, nicht als DNT markieren.

### 6. No communication channel connected

**Angezeigter Befund:** Auf Organisationsebene ist kein Chat- oder Benachrichtigungskanal verbunden.

**Maßnahme:** Nur verbinden, wenn für Crowdin tatsächlich ein gemeinsamer Teamkanal genutzt wird. Das ist eine optionale Organisationsintegration und keine Voraussetzung für die Übersetzungsdateien, das Glossar oder den GitHub-Ablauf. Keinen Kanal allein deshalb neu anlegen, um die Advisor-Meldung auszublenden.

## Separater Advisor: Projektbeschreibung fehlt

Die Meldung **Missing project description** ist ein zusätzlicher allgemeiner Advisor und gehört nicht zu den sechs oben genannten Context Advisors. Die Beschreibung aus dem Abschnitt „Projektbeschreibung für Crowdin“ unter **Settings → General settings → Description** eintragen.

## Reihenfolge zum Abarbeiten

1. GitHub-Integration auf den aktiven Releasebranch und `crowdin.yml` prüfen. Eine Synchronisierung starten und verifizieren, dass Quellstrings eingelesen wurden.
2. Bestehende Übersetzungen einmalig aus dem Repository importieren.
3. Projektbeschreibung und Dateikontext setzen.
4. Stilrichtlinie anlegen und zuweisen.
5. Das bereits zugewiesene Glossar mit geprüften, wirklich verwendeten Begriffen füllen.
6. Screenshots aus der aktuellen Anwendung hochladen und zuordnen.
7. Zeichenlimits gezielt für nachweislich platzbeschränkte UI-Strings setzen.
8. Einen Organisationskanal nur verbinden, wenn es dafür einen bestehenden Teamablauf gibt.

## Crowdin-Dokumentation

- [Projektseite für Mitwirkende](https://crowdin.com/project/shroudforge)
- [Projekte finden und ihnen beitreten](https://support.crowdin.com/for-translators/)
- [GitHub-Integration und Synchronisierung](https://support.crowdin.com/github-integration/)
- [Branches und Versionsverwaltung](https://support.crowdin.com/version-management/)
- [Crowdin-Glossar](https://support.crowdin.com/glossary/)
- [Crowdin-Stilrichtlinien](https://support.crowdin.com/project-settings/style-guides/)
- [QA Checks und Rechtschreibprüfung](https://support.crowdin.com/project-settings/qa-checks/)
- [Screenshots in Crowdin](https://support.crowdin.com/screenshots/)

## Crowdin-Stand pro Build in der News-Karte

Die Grafik aus Crowdin **Badges & Status Images** ist ein allgemeines Balkendiagramm. ShroudForge zeigt den Sprachfortschritt stattdessen mit eigenen horizontalen Balken für die vorhandenen Zielsprachen an. Englisch (UK) ist die Quellsprache und wird nicht als Zielsprache dargestellt. Die Repository-Konfiguration behandelt Englisch (US) als Zielvariante. Der Stand wird einmal beim Erstellen eines Builds aus dessen Crowdin-Versionsbranch abgefragt und in die UI eingebettet. Installierte Builds fragen Crowdin nicht im Hintergrund ab und aktualisieren den Stand nicht nachträglich.

Crowdin verlangt für den branchbezogenen Fortschritts-Endpunkt einen API-Token. Der Token bleibt in GitHub Actions oder in der lokalen Build-Umgebung und wird nicht in den ShroudForge-Client eingebaut. **dev/build/capture_crowdin_progress.mjs** liest die Zielsprachen dynamisch aus den JSON-Dateien unter **modules/ui-resources/locales/**, ruft den Crowdin-Branch aus **VERSION** ab und schreibt den Snapshot nach **build/ui/modloader-inputs/crowdin-progress.json**. Dabei werden nur Sprachen angezeigt, für die die UI bereits eine Locale-Datei besitzt. Namen und Flaggen kommen direkt aus der vorhandenen UI-Locale-Liste. Die Crowdin-Dateimaske **%two_letters_code%.json** verwendet nur den zweistelligen Sprachcode. Ohne Zuordnung würden `de-AT` und `de-CH` auf `de.json` und `en-US` auf `en.json` fallen. Die Zuordnung in **crowdin.yml** erhält stattdessen die vollständigen Dateinamen **de-AT.json**, **de-CH.json** und **en-US.json**. Die Werte entsprechen den Crowdin-Sprachcodes. Alternativ kann **%locale%** den vollständigen Locale-Code ausgeben. Dann müssen jedoch auch die bisherigen kurzen Dateinamen wie `de.json` und `fr.json` auf die von Crowdin gelieferten Locale-Codes umgestellt werden. [Crowdin dokumentiert beide Platzhalter und die Sprachzuordnung](https://support.crowdin.com/developer/configuration-file/). **de-AT** enthält Wienerisch und **de-CH** Zürcher Schweizerdeutsch. Die UI-Buildskripte **npm run build** und **npm run dev** erfassen den Fortschritt vor dem Start automatisch. Ohne Token erzeugen sie für jede vorhandene Zielsprache einen Eintrag ohne Fortschrittswerte. Der Snapshot wird in das fertige UI und damit in den Build eingebettet. Im installierten Modloader gibt es keine Crowdin-Abfrage.

### Einmalige Einrichtung

1. In Crowdin einen Personal Access Token mit den reinen Leserechten **project.source.file** und **project.status.progress** erstellen. Der erste Umfang wird zum Auflisten der Release-Branches benötigt. Der zweite liest deren Übersetzungs- und Freigabefortschritt.
2. Im GitHub-Repository **Settings → Secrets and variables → Actions → New repository secret** öffnen. Den Secret **CROWDIN_PERSONAL_TOKEN** nennen und dort den Token eintragen.
3. Die Änderungen an **dev/build/capture_crowdin_progress.mjs**, **modules/modloader/ui/package.json**, **modules/modloader/ui/vite.config.ts** und **modules/modloader/ui/tsconfig.json** übernehmen. Wenn **CROWDIN_PERSONAL_TOKEN** gesetzt ist, wird der Crowdin-Stand bei jedem UI-Build und beim Start des lokalen UI-Entwicklungsservers abgefragt. Ohne Token werden dynamische Einträge ohne Fortschrittswerte erzeugt. Für eine neue Sprache kommen die Crowdin-Zielsprache und ihre Locale-Datei mit **locale.name** und **locale.flag** hinzu. Locale-Auswahl, Fortschrittsabfrage, Flaggen-Generierung und Locale-Prüfung nehmen die neue Sprache danach automatisch auf. Ein Build mit **-SkipUiBuild** verwendet den bereits vorhandenen UI-Stand. Es gibt keinen Stundenplan und keine Crowdin-Abfrage in der installierten App.

Der Projekt-Identifier **938941** ist voreingestellt. Falls er als Repository-Variable **CROWDIN_PROJECT_ID** gesetzt ist, wird dieser Wert verwendet. Die News-Karte zeigt für die gebaute Version den abgefragten Crowdin-Branch, Übersetzungsprozente, Freigabeprozente, Wortzahlen und den Zeitpunkt der Abfrage an. Fehlt der Token, enthält der Build absichtlich keine Crowdin-Werte und zeigt einen entsprechenden Hinweis.

Crowdin dokumentiert für die API die Scopes und den Branch-Fortschritts-Endpunkt unter [Crowdin API v2](https://support.crowdin.com/developer/api/v2/). Die Grafik-App ist auf der [Crowdin Store-Seite](https://store.crowdin.com/bds) beschrieben.
