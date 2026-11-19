# Online-Wikiartikel erstellen und übersetzen

Dieses Verzeichnis enthält ausschließlich die technischen Werkzeuge für das öffentliche Wiki unter `https://wiki.shroudforge.net`. Artikel werden direkt über die MediaWiki-API veröffentlicht. Dauerhafte Artikeltexte, Illustrationen und PO-Dateien liegen nicht im Repository.

## Temporärer Arbeitsbereich

Alle Entwürfe und Übersetzungsdateien liegen unter `target/wiki/<artikel-id>/`. Der gesamte Ordner `target/` ist bereits durch `.gitignore` ausgeschlossen. Die dort abgelegten Wikitext- und PO-Dateien werden nicht committet.

Eine Artikelmappe enthält üblicherweise:

```text
target/wiki/<artikel-id>/
  en.wikitext
  ShroudForge-Wiki-<artikel-id>.svg
  de.po
  de-at.po
  de-ch.po
  es.po
  fr.po
  hi.po
  it.po
  ja.po
  nl.po
  pl.po
  sv.po
  zh.po
```

Die Sprachdateien entstehen nur, wenn Übersetzungen gewünscht sind. Es gibt keine Artikeldatenbank und keinen lokalen Online-Wiki-Artikelbestand im Repository.

## Werkzeuge

| Datei | Zweck |
| --- | --- |
| `new_article_workspace.ps1` | Erstellt eine leere, temporäre englische Artikelmappe unter `target/wiki/`. |
| `publish_article.ps1` | Lädt fehlende Bilder und den englischen Wikitext über die API hoch, markiert die Quellseite für Translate und setzt die Prioritätssprachen. Bestehende Seitentexte bleiben standardmäßig unverändert. |
| `create_po_templates.ps1` | Liest die Nachrichtenschlüssel der veröffentlichten Translate-Gruppe live aus dem Wiki und erzeugt leere PO-Vorlagen im temporären Artikelordner. |
| `import_po.ps1` | Importiert eine fertig übersetzte PO-Datei getrennt über `Special:ImportTranslations` und prüft gespeicherte Übersetzungen sowie die gerenderte Sprachseite. |
| `configure_credentials.ps1` | Speichert API- und Importzugang verschlüsselt im Windows-Benutzerprofil. |

Alle Befehle werden aus dem Repository-Stamm ausgeführt. Die API-Anmeldung verwendet `%LOCALAPPDATA%\ShroudForge\wiki-api-credential.xml`. Für den PO-Import wird ein normales Wiki-Konto mit `translate-import` unter `%LOCALAPPDATA%\ShroudForge\wiki-import-credential.xml` verwendet. Ein BotPassword kann die Special-Page nicht öffnen.

## Ablauf 1: Englischen Artikel veröffentlichen und markieren

API-Zugang einmalig einrichten, falls er noch nicht gespeichert ist:

```powershell
pwsh -NoProfile -File dev/wiki/configure_credentials.ps1 -Kind Api
```

Temporäre Artikelmappe mit englischer Wikitext-Vorlage erstellen:

```powershell
pwsh -NoProfile -File dev/wiki/new_article_workspace.ps1 `
  -ArticleId monetization-guidelines `
  -Title 'Enshrouded Fan Project Guidelines' `
  -Description 'An overview of the fan project rules and how ShroudForge follows them.'
```

Den englischen Artikel in `target/wiki/monetization-guidelines/en.wikitext` bearbeiten. Die verwendete Bilddatei muss mit `picture:` übereinstimmen und im selben temporären Ordner liegen. Die Seite benötigt einen `<translate>`-Block und die Kategorie `[[Category:guide:Shroudforge]]`.

Zuerst ohne Änderungen am Wiki prüfen, danach veröffentlichen:

```powershell
pwsh -NoProfile -File dev/wiki/publish_article.ps1 `
  -WorkspacePath target/wiki/monetization-guidelines `
  -WhatIf

pwsh -NoProfile -File dev/wiki/publish_article.ps1 `
  -WorkspacePath target/wiki/monetization-guidelines
```

Das Skript erstellt eine fehlende Quellseite, lädt lokal vorhandene und im Artikel verwendete Bilder hoch und markiert die englische Seite per API für Translate. Es aktiviert Titelübersetzung und setzt die aktuell unterstützten Prioritätssprachen `de-at`, `de-ch`, `de`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` und `zh`. `en-GB` ist die Quellsprache.

Existiert die Seite bereits, lässt das Skript ihren Text unverändert und markiert die aktuelle Revision. `-UpdateExisting` überschreibt den Seitentext gezielt und verwendet dabei die aktuelle Revisions-ID als Konfliktschutz.

## Ablauf 2: PO-Dateien erzeugen und einzeln importieren

Nach dem Veröffentlichen die exakte Translate-Gruppen-ID `page-<englischer Seitentitel>` verwenden. Die Vorlagen werden anhand der live aus dem Wiki gelesenen Kontexte und Quelltexte erstellt:

```powershell
pwsh -NoProfile -File dev/wiki/create_po_templates.ps1 `
  -Group 'page-Enshrouded Fan Project Guidelines' `
  -WorkspacePath target/wiki/monetization-guidelines
```

Die leeren `msgstr`-Felder der gewünschten Sprachdateien ausfüllen. `msgctxt`, `msgid`, `X-Message-Group` und `X-Language-Code` unverändert lassen. Die Datei beginnt unmittelbar mit dem Gettext-Header `msgid ""`. Das Wiki lehnt vorangestellte Kommentare ab. Das Skript erzeugt standardmäßig alle Prioritätssprachen der Live-Gruppe. Vorhandene PO-Dateien bleiben erhalten. `-Force` ersetzt sie ausdrücklich.

Jede PO-Datei separat prüfen und importieren:

```powershell
pwsh -NoProfile -File dev/wiki/import_po.ps1 `
  -PoPath target/wiki/monetization-guidelines/de.po `
  -WhatIf

pwsh -NoProfile -File dev/wiki/import_po.ps1 `
  -PoPath target/wiki/monetization-guidelines/de.po
```

Beim ersten echten Import fragt das Skript nach dem normalen Wiki-Konto, falls noch kein Importzugang gespeichert ist. Alternativ den Zugang vorher einrichten:

```powershell
pwsh -NoProfile -File dev/wiki/configure_credentials.ps1 -Kind Import
```

Der Import läuft über das Formular von `Special:ImportTranslations`. Das Skript prüft vor dem Upload, dass die PO-Datei alle Live-Kontexte enthält und ihre englischen `msgid`-Quellen mit der Wiki-Gruppe übereinstimmen. Danach verifiziert es die gespeicherten Übersetzungen. MediaWiki rendert Sprach-Unterseiten über `TranslateRenderJob`. Fehlt eine Unterseite trotz gespeichertem Import, muss der Wiki-Host die Translate-Jobs abarbeiten.

## Medien und Grenzen

Derzeit akzeptiert die Wiki-API PNG, GIF, JPG, JPEG, WebP und SVG. PDF-Dateien werden abgewiesen. Für PDFs einen externen Original-Link angeben oder Seiten als Bilder im temporären Artikelordner bereitstellen.

`modules/wiki/articles/` gehört zum Offline-Hilfesystem der Modloader-Anwendung. Für öffentliche Online-Wikiartikel nicht dort schreiben. Dessen Artikelverfahren bleibt im Skill `add-modloader-wiki-article` dokumentiert.
