# MediaWiki-Translate-Ablauf

Dieses Verzeichnis enthält die PO-Dateien der verfügbaren Artikelübersetzungen und die wiederholbare Veröffentlichung der Modloader-Wikiartikel auf MediaWiki. Das Artikelschema folgt `Dependencies`: Metadaten und Bild stehen außerhalb von `<translate>`, die englische Zusammenfassung und der Inhalt stehen innerhalb davon. Die Guide-Kategorie steht immer zuerst, außer ein Artikel ist einer eigenen Themenkategorie zugeordnet. `Startup stops because asset backups are incompatible` erhält zusätzlich `Problems:Shroudforge`. `Search and organize large mod settings` verwendet `Modding:Shroudforge` als erste Kategorie und erhält zusätzlich `Mod-extended-json`.

## Ein-Befehl-Ablauf

Alle Schritte laufen mit diesem Befehl. Beim ersten echten Lauf fragt PowerShell einmal nach dem normalen Wiki-Konto für den PO-Import. Das Passwort wird verschlüsselt im Windows-Benutzerprofil gespeichert. Danach baut das Skript die Seiten, veröffentlicht fehlende Inhalte und Bilder, setzt alle im Modloader verfügbaren und von MediaWiki akzeptierten Prioritätssprachen und importiert vorhandene PO-Dateien:

```powershell
pwsh -NoProfile -File docs/mediawiki-translate/run.ps1
```

Ein Trockenlauf prüft die Verbindung, Seitengruppen und PO-Dateien, ohne Wiki-Inhalte zu verändern oder nach dem normalen Importkonto zu fragen:

```powershell
pwsh -NoProfile -File docs/mediawiki-translate/run.ps1 -WhatIf
```

Du kannst den Ablauf auf einen Artikel begrenzen:

```powershell
pwsh -NoProfile -File docs/mediawiki-translate/run.ps1 -ArticleId dll-manager
```

## Einzelne Schritte

1. Englische und verfügbare übersetzte Artikel liegen unter `modules/wiki/articles/<id>/`.
2. PO-Dateien aus vorhandenen Deutsch-, Spanisch- und Französischinhalten bereitest du mit folgendem Befehl vor:

   ```powershell
   python -S docs/mediawiki-translate/prepare-localized-po.py
   ```

   Die deutsche PO wird außerdem für `de-at` und `de-ch` übernommen. Für `de-ch` wird `ß` zu `ss`. Vorhandene spanische und französische Artikeltexte werden übernommen.

   Die lokal erstellten Übersetzungen für `hi`, `it`, `ja`, `nl`, `pl`, `sv` und `zh` stehen in `offline-translations.json`. `populate-translated-po.py` setzt diese Texte in die passenden PO-Nachrichten ein. Für `game-profile-discovery` und `translations` enthält die Datei auch die fehlenden spanischen und französischen Übersetzungen.

3. Englische MediaWiki-Seiten und das Veröffentlichungsmanifest erzeugst du mit:

   ```powershell
   python -S docs/mediawiki-translate/build-pages.py
   ```

4. Der API-Dry-Run prüft Artikel und Translate-Gruppen, ohne Änderungen zu speichern:

   ```powershell
   pwsh -NoProfile -File docs/mediawiki-translate/publish.ps1 -WhatIf
   ```

5. Die Veröffentlichung legt fehlende Seiten und Bilder an, markiert die Seite für Translate, erlaubt die Übersetzung des Titels und setzt alle zwölf von MediaWiki akzeptierten Modloader-Locales als Prioritätssprachen. Sie überschreibt keine bestehenden Artikeltexte:

   ```powershell
   pwsh -NoProfile -File docs/mediawiki-translate/publish.ps1
   ```

6. Der PO-Import läuft über das Formular von [Special:ImportTranslations](https://wiki.shroudforge.net/index.php/Special:ImportTranslations). Beim ersten Lauf fragt PowerShell nach dem normalen Wiki-Konto mit `translate-import`-Recht. Die Anmeldung wird verschlüsselt im Windows-Benutzerprofil abgelegt. Danach lädt das Skript jede vollständige PO-Datei hoch, übernimmt die Standardaktion „Importieren“ für geänderte Einträge und prüft die Übersetzungen per Translate-API. Zum Schluss prüft es, ob MediaWiki die Sprach-Unterseiten erzeugt hat.

   ```powershell
   pwsh -NoProfile -File docs/mediawiki-translate/import-po.ps1
   ```

   Die Anmeldung kann auch separat eingerichtet werden. Dieses Kommando speichert den Zugang und beendet sich danach:

   ```powershell
   pwsh -NoProfile -File docs/mediawiki-translate/import-po.ps1 -ConfigureCredential
   ```

   Für einen einzelnen Artikel kannst du `-ArticleId dll-manager` ergänzen. Zum Beispiel:

   ```powershell
   pwsh -NoProfile -File docs/mediawiki-translate/import-po.ps1 -ArticleId dll-manager
   ```

## Aktueller Locale-Stand

MediaWiki akzeptiert `de-at`, `de-ch`, `de`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` und `zh` als Prioritätssprachen. `en-GB` bleibt die Quellsprache. `en-US` weist die Wiki-API als ungültige Prioritätssprache zurück und wird nicht als zusätzliche Zielseite angelegt.

Für alle elf Artikel liegen vollständige PO-Dateien in den zwölf von MediaWiki akzeptierten Zielsprachen vor. Das sind insgesamt 132 Dateien. Die Texte für Sprachen ohne Modloader-Wikiartikel werden lokal in `offline-translations.json` gepflegt. Der Ablauf erzeugt daraus PO-Dateien mit den echten Nachrichten-IDs der jeweiligen MediaWiki-Gruppe. Das Importskript prüft alle Einträge und stoppt bei leeren oder unvollständigen PO-Dateien.

## Zugang und Importweg

Der Bot-Zugang liegt verschlüsselt unter `%LOCALAPPDATA%\ShroudForge\wiki-api-credential.xml`. Er kann Seiten erstellen, Bilder hochladen und Übersetzungen markieren. `publish.ps1` nutzt ihn für die Action API.

MediaWiki Translate stellt keinen Action-API-Endpunkt für den PO-Import bereit. `import-po.ps1` meldet sich deshalb mit dem normalen Wiki-Konto an und sendet die Datei über das Formular von `Special:ImportTranslations`. Das normale Konto wird ebenfalls verschlüsselt unter `%LOCALAPPDATA%\ShroudForge\wiki-import-credential.xml` gespeichert. Kein Passwort gehört ins Repository oder in den Chat. Das Bot-Passwort funktioniert nicht für diese Special-Page.

Die aktuelle Wiki-Konfiguration vergibt `translate-import` bereits an normale Benutzer. Eine Benutzergruppe namens `wiki-import` ist dort nicht eingerichtet. „Versionen importieren“ (`import`) und Dateiimporte (`importupload`) sind andere Rechte und werden für PO-Dateien nicht benötigt.

Der Formularablauf wird in mehreren Anfragen abgearbeitet, falls Translate den Import wegen eines Zeitlimits aufteilt. Vor dem Upload prüft das Skript Sprache, Seitengruppe und Nachrichten-IDs der PO-Datei. Danach kontrolliert es, dass alle PO-Einträge in der entsprechenden Translate-Gruppe gespeichert wurden.

Der Import beruht auf dem Formularablauf der Special-Page. MediaWiki dokumentiert ihn nicht als stabile API. Falls ein MediaWiki- oder Translate-Update das Formular ändert, stoppt das Skript bei abweichenden Feldern oder Fehlermeldungen, statt eine nicht verifizierte Übertragung zu melden.

## Rendern der Sprachseiten

Der PO-Import speichert zunächst die Translate-Nachrichten. MediaWiki erzeugt oder aktualisiert die sichtbaren Sprach-Unterseiten anschließend mit `TranslateRenderJob` im Job-Queue. Wenn die Queue nicht regelmäßig verarbeitet wird, können die PO-Einträge vollständig gespeichert sein, während die Seiten wie `Dependencies/hi` noch fehlen. `import-po.ps1` meldet diesen Zustand jetzt ausdrücklich.

Zum Prüfen der Queue braucht der Wiki-Host Zugriff auf die MediaWiki-Installation. Der Host-Administrator kann die Jobtypen mit `php maintenance/run.php showJobs --group` anzeigen und gezielt die Translate-Renderjobs mit `php maintenance/run.php runJobs --type=<angezeigter-jobtyp> --maxjobs=100 --maxtime=60` ausführen. So muss nicht die gesamte Queue auf einmal abgearbeitet werden. MediaWiki dokumentiert, dass `TranslateRenderJob` die Sprachseite nach Änderungen an Übersetzungseinheiten aktualisiert und dass dafür der Job-Queue verarbeitet werden muss.

## Dateien

- `<id>/en.wikitext` wird aus `modules/wiki/articles/<id>/en.md` nach dem Translate-Schema erzeugt.
- `<id>/<sprache>.po` enthält die Übersetzung für `page-<englischer Seitentitel>` in der im PO-Kopf angegebenen Sprache.
- `prepare-localized-po.py` erstellt PO-Dateien aus vorhandenen Deutsch-, Spanisch- und Französischartikeln sowie die Varianten `de-at` und `de-ch`.
- `offline-translations.json` enthält lokal erstellte Texte für die übrigen Sprachen und die zwei fehlenden spanischen und französischen Artikel.
- `populate-translated-po.py` verbindet diese Texte mit den originalen Nachrichten-IDs aus den vorhandenen POs.
- `publish-manifest.json` wird beim Erzeugen der englischen Seiten aktualisiert.
- `build-pages.py` erzeugt Wikitext und Manifest.
- `publish.ps1` erstellt fehlende Seiten, lädt fehlende Bilder hoch, markiert die Seite und setzt alle unterstützten Prioritätssprachen.
- `import-po.ps1` lädt PO-Dateien über `Special:ImportTranslations` hoch, verifiziert die gespeicherten Einträge und prüft, ob die Sprach-Unterseiten gerendert wurden.

Die deutsche PO-Datei für `Dependencies` wurde anhand der echten Translate-Nachrichten-IDs erstellt. Sie ergänzt die fehlenden Zusammenfassungs- und Abschnittstitel und korrigiert den übersetzten Seitentitel.
