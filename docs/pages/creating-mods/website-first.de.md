---
{
  "id": "website-first",
  "locale": "de",
  "title": "Den ersten Mod erstellen",
  "summary": "Die bestehende interaktive Lektion mit Beispielen und Schritten.",
  "view": "first",
  "navigation": {
    "path": [
      "MODDING"
    ],
    "label": "Ersten Mod bauen"
  }
}
---

    <div class="page-intro"><div class="eyebrow">DEIN ERSTER MOD</div><h1>Wir bauen deinen<br>ersten kleinen Mod.</h1><p class="lead">Du brauchst keine Programmiererfahrung. Wir legen einen Ordner an, kopieren ein kurzes Beispiel hinein und prüfen, ob ShroudForge es findet.</p></div><div class="lesson-meta"><span>FÜR <b>Einsteiger</b></span><span>DAUER <b>10–15 Minuten</b></span><span>DU BRAUCHST <b>Einen Texteditor</b></span></div>
    <section class="file-map" aria-label="Die Dateien deines ersten Mods"><div class="map-heading"><span>DREI EINFACHE BAUSTEINE</span><h2>Dein erster Mod auf einen Blick</h2><p>Jede Datei hat eine Aufgabe. ShroudForge liest die Infos und führt die Anweisung aus.</p></div><div class="file-map-row"><article><code>mod.json</code><b>Der Infozettel</b><small>Name, Version und nötige Zugriffe.</small></article><span>+</span><article><code>src/mod.lua</code><b>Die Anweisung</b><small>Was dein Mod ShroudForge tun lässt.</small></article><span>+</span><article><code>extended.mod.json</code><b>Freiwillige Einstellungen</b><small>Schalter und andere Felder, die Spieler ändern können.</small></article></div><div class="file-map-end">ShroudForge liest diese Dateien → zeigt deinen Mod bei <kbd>F9</kbd> → schreibt Meldungen bei <kbd>F10</kbd></div></section>
    <div class="lesson-step"><span class="step-num">01</span><div><h2>Einen Ordner für den Mod anlegen</h2><p>Öffne den Enshrouded-Spielordner und darin den Ordner <code>mods</code>. Lege dort einen neuen Ordner namens <code>deinname.erster-mod</code> an. Er ist die Schachtel für alle Dateien deines Mods.</p><pre><code>mods/deinname.erster-mod/
├── mod.json
└── src/
    └── mod.lua</code></pre><p><code>mod.json</code> ist der Zettel mit den Mod-Infos. <code>mod.lua</code> enthält die eine Anweisung, die unser Beispiel ausführt.</p></div></div>
    <div class="lesson-step"><span class="step-num">02</span><div><h2>Den Mod-Infozettel ausfüllen</h2><p>Erstelle im neuen Ordner die Datei <code>mod.json</code> und kopiere dieses Beispiel hinein. Du kannst deinen Namen und die Beschreibung ändern:</p><pre><code>{
  "$schema": "https://shroudforge.net/schemas/1.8.0/manifest.schema.json",
  "id": "deinname.erster-mod",
  "name": "Mein erster Mod",
  "version": "1.0.0",
  "authors": ["Dein Name"],
  "description": "Schreibt eine Meldung.",
  "license": null,
  "icon": null,
  "capabilities": [],
  "dependencies": []
}</code></pre><div class="data-table"><div><code>id</code><span>Ein eindeutiger Ordnername für den Mod. Am besten klein schreiben.</span></div><div><code>name</code> · <code>authors</code><span>Der Name, den andere sehen, und dein Name als Ersteller.</span></div><div><code>version</code><span>Die Versionsnummer. Für den Anfang kannst du <code>1.0.0</code> lassen.</span></div><div><code>description</code><span>Ein kurzer Satz dazu, was dein Mod macht.</span></div><div><code>capabilities</code> · <code>dependencies</code><span>Welche besonderen Dinge der Mod darf und ob er einen anderen Mod braucht. Für dieses Beispiel bleiben beide leer: <code>[]</code>.</span></div></div></div></div>
    <div class="lesson-step"><span class="step-num">03</span><div><h2>Eine Meldung schreiben lassen</h2><p>Lege im Ordner <code>src</code> die Datei <code>mod.lua</code> an. Kopiere diese Zeile hinein:</p><pre><code>shroudforge.log.info("Mein erster Mod wurde geladen")</code></pre><p>Diese Zeile bittet ShroudForge, den Satz in seine Meldungen zu schreiben. Du kannst sie einfach kopieren. Für dieses Beispiel musst du nicht verstehen, wie der Code dahinter funktioniert.</p></div></div>
    <div class="lesson-step"><span class="step-num">04</span><div><h2>Im Spiel nachsehen</h2><p>Starte Enshrouded. Drücke <kbd>F9</kbd>, um das Mods-Fenster zu öffnen. Dein Mod sollte in der Liste stehen. Drücke <kbd>F10</kbd>, um die Meldungen zu öffnen und deinen Satz zu suchen.</p><div class="callout warning"><b>Du findest den Mod nicht?</b><span>Prüfe, ob du deinen neuen Mod-Ordner in den Spielordner <code>mods</code> kopiert hast. Downloads können unterschiedlich aufgebaut sein. Folge deshalb der Anleitung des Mod-Erstellers.</span></div></div></div>
    <section class="next-panel"><div><div class="eyebrow">DAS HAST DU GESCHAFFT</div><h2>Jetzt kann dein Mod eigene Einstellungen bekommen.</h2><p>Als Nächstes zeigen wir dir, wie ein Schalter oder Regler im Mods-Fenster erscheint. Die beiden JSON-Dateien werden dort ganz einfach erklärt.</p></div><button class="button primary" data-view="manifests">Einstellungen ausprobieren →</button></section>