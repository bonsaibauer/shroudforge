# MediaWiki-Import der Modloader-Wikiartikel

Dieses Paket überführt die aktuell im Modloader angezeigten Wikiartikel in Wikitext und einen XML-Importdump für MediaWiki. Die Quelle ist `modules/wiki/articles/`. Die Migration enthält alle 10 Artikel-IDs und alle 36 dafür vorhandenen Sprachfassungen. Fehlende Übersetzungen werden nicht ergänzt.

## Einheitliches Artikelschema

Jede importierte Sprachfassung hat denselben Aufbau:

1. Der MediaWiki-Seitentitel enthält den übersetzten Artikeltitel und den Sprachcode, zum Beispiel `Abhängigkeiten (de)`. Der Sprachcode hält gleichnamige Übersetzungen als getrennte Seiten auseinander.
2. Ein nicht angezeigter HTML-Kommentar speichert Titel, Kurzbeschreibung, Sprache, Ersteller und Quelldatei.
3. Der erste sichtbare Absatz ist die Kurzbeschreibung aus dem Modloader-Wiki.
4. Danach folgen die übersetzten Artikelinhalte als MediaWiki-Wikitext.
5. Am Ende steht die Kategorie `guide:shroudforge`.

Der Ersteller ist `Bonsaibauer`. Dieser Name steht in den Seitenmetadaten und als Bearbeiter der importierten Erstfassung. Die Importmaske kann ihn einem gleichnamigen lokalen Konto zuordnen oder mit einem Herkunftspräfix als fremden Bearbeiter führen. Alte interne Artikelgruppen aus `wiki-category` werden nicht übernommen, da alle Seiten der gewünschten Kategorie zugeordnet werden.

## Dateien

- `import/shroudforge-modloader-wiki.xml` enthält die 36 Sprachseiten und die Kategorie-Seite. Den XML-Dump kannst du über `Special:Import` importieren.
- `articles/<artikel-id>/<sprache>.wikitext` enthält jede Seite einzeln zur Durchsicht.
- `pages.csv` ordnet Seitentitel, Kurzbeschreibung, Sprache, Quelldatei und Bilddatei zu.
- `assets/` enthält die zehn SVG-Illustrationen. `assets/asset-map.csv` ordnet jede Datei den Artikeln und Sprachfassungen zu.
- `build-package.py` erstellt den Dump, die Wikitext-Dateien, CSV-Listen und Bildkopien erneut aus `modules/wiki/articles/`.

## Import

1. Sichere die Zielwiki, bevor du den Import startest.
2. Lade `import/shroudforge-modloader-wiki.xml` in `Special:Import` hoch. Für den Dateiimport benötigt das Konto die Rechte `import` und `importupload`. Die Seitentitel bleiben standardmäßig gleich. Das Interwiki-Präfix betrifft die importierten Bearbeiternamen. Wenn es im Zielwiki ein lokales Konto `Bonsaibauer` gibt, aktiviere die Zuordnung importierter Bearbeiter zu gleichnamigen lokalen Konten. Andernfalls kannst du ein Präfix wie `shroudforge` verwenden, damit der fremde Bearbeiter kenntlich bleibt.
3. Lade die SVG-Dateien aus `assets/` über `Special:Upload` hoch. Verwende die Dateinamen aus `assets/asset-map.csv`. Der XML-Seitenimport enthält keine Binärdateien.
4. Prüfe eine Seite je Sprache und öffne die Kategorie `Category:guide:shroudforge`.

MediaWiki kann SVG-Uploads je nach Zielwiki-Konfiguration ablehnen. Die Uploads müssen aktiviert sein und `svg` muss zu den erlaubten Dateiendungen gehören. Für Vorschaubilder wird außerdem ein SVG-Konverter benötigt. Falls das Zielwiki SVG nicht annimmt, müssen die zehn Illustrationen vor dem Upload in PNG umgewandelt und die Dateinamen in Wikitext und Zuordnungsliste angepasst werden.

MediaWiki verwendet standardmäßig eine Großschreibung des ersten Titelzeichens. Je nach Wiki-Konfiguration kann die Kategorie daher als `Category:Guide:shroudforge` erscheinen, obwohl der Import die verlangte Schreibweise `guide:shroudforge` verwendet.

## Recherche

- [MediaWiki: XML-Dumps importieren](https://www.mediawiki.org/wiki/Manual:Importing_XML_dumps): beschreibt `Special:Import`, nötige Importrechte und den Import über `importDump.php`.
- [MediaWiki-Hilfe: Import](https://www.mediawiki.org/wiki/Help:Import): erklärt Seitentitel, Interwiki-Präfixe und die Zuordnung importierter Bearbeiter zu lokalen Konten.
- [MediaWiki-Hilfe: Kategorien](https://www.mediawiki.org/wiki/Help:Categories): beschreibt die Zuordnung über `[[Category:Name]]` und die Kategorie-Seite.
- [MediaWiki-Handbuch: Dateiverwaltung](https://www.mediawiki.org/wiki/Manual:Image_administration): beschreibt `Special:Upload` und die SVG-Konfiguration.
- [MediaWiki-Handbuch: erlaubte Dateiendungen](https://www.mediawiki.org/wiki/Manual:$wgFileExtensions): beschreibt die Upload-Allowlist für Dateitypen.

## Paket neu erstellen

Vom Repository-Hauptordner aus:

```powershell
python -S docs/mediawiki-migration/build-package.py
```
