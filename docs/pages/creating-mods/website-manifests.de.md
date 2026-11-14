---
{
  "id": "website-manifests",
  "locale": "de",
  "title": "Manifest-Editor",
  "summary": "Der bestehende Mod- und Erweiterungsmanifest-Editor mit Vorschau und Download.",
  "view": "manifests",
  "navigation": {
    "path": [
      "MODDING"
    ],
    "label": "Mod-Einstellungen und Vorschau"
  }
}
---


    <div class="page-intro"><div class="eyebrow">DIE ZWEI INFO-DATEIEN</div><h1>Zwei Dateien mit Infos.<br>Einfach erklärt.</h1><p class="lead">Jeder Mod hat einen Infozettel. Manche Mods haben noch eine zweite Datei für eigene Schalter und Regler. Beide Dateien enden auf <code>.json</code> – das ist ein einfaches Textformat.</p></div><div class="manifest-compare"><article><span class="file-icon">{ }</span><h2>mod.json</h2><p>Der Infozettel des Mods: Name, Ersteller, Versionsnummer und was der Mod braucht.</p><ul><li>Pflicht: <code>id</code>, <code>name</code>, <code>version</code></li><li>Welche Erlaubnisse der Mod braucht: <code>patch</code>, <code>export</code>, <code>runtime</code> oder <code>runtime-register-dll</code> (für EML-Mods mit eigener DLL)</li><li>Diese Datei gehört zu jedem Mod</li></ul></article><article><span class="file-icon gold">{ + }</span><h2>extended.mod.json</h2><p>Diese Datei brauchst du nur, wenn dein Mod eigene Schalter oder Regler im Mods-Fenster zeigen soll. Du kannst darin auch Links zum Ersteller angeben.</p><ul><li>Pflicht: <code>schemaVersion: 1</code></li><li>Werte und optionale Details stehen unter <code>settings</code></li><li>Ein Mod-Update kann Werte zurücksetzen</li></ul></article></div><h2 class="section-title">Welche Infos stehen in mod.json?</h2><div class="data-table"><div><code>id</code> · <code>name</code> · <code>version</code><span>Eine feste Kurzkennung für den Mod, der Name im Spiel und die Nummer dieser Mod-Version.</span></div><div><code>authors</code> · <code>description</code><span>Wer den Mod gemacht hat und was er tut.</span></div><div><code>dependencies</code><span>Andere Mods, die zuerst installiert sein müssen. Für einfache Mods bleibt die Liste leer: <code>[]</code>.</span></div><div><code>capabilities</code><span>Welche besonderen Aktionen der Mod ausführen darf. Wähle nur Erlaubnisse aus, die er wirklich braucht.</span></div><div><code>icon</code> · <code>license</code><span>Beide Angaben sind freiwillig. Nutze einen Icon-Dateinamen im Mod-Ordner oder einen Pfad unter <code>assets/</code> sowie einen bekannten Lizenznamen. Sonst <code>null</code> oder Feld weglassen. Details stehen im <a href="https://shroudforge.net/schemas/1.8.0/manifest.schema.json">Manifest-Schema</a>.</span></div></div><h2 class="section-title">Welche Infos kommen in die zweite Datei?</h2><div class="data-table"><div><code>schemaVersion</code><span>Aktuell muss der Wert <code>1</code> sein.</span></div><div><code>enabled</code><span>Der aktuelle Aktivierungszustand. Änderungen schreibt der Loader hier hinein.</span></div><div><code>settings</code><span>Einstellungsnamen und Werte. Einfache Werte brauchen keine Typangabe.</span></div><div><code>groups</code><span>Optional: Einstellungen und Lua-Aktionsknöpfe ordnen.</span></div><div><code>links</code> · <code>changelog</code><span>Optionale Projektlinks und Versionshinweise.</span></div></div>
    <section class="dependency-guide"><div class="eyebrow">ABHÄNGIGKEITEN · MOD.JSON</div><h2>Eine Mod braucht eine andere Mod?</h2><p>Abhängigkeiten werden in <code>mod.json</code> eingetragen, nicht in <code>extended.mod.json</code>. Verwende als <code>id</code> genau die Kennung aus der <code>mod.json</code> der benötigten Mod – nicht ihren Anzeigenamen oder Ordnernamen. <code>version</code> ist ein SemVer-Versionsbereich.</p><div class="dependency-examples"><article><b>Pflicht-Abhängigkeit</b><p>Ohne <code>optional</code> ist die Abhängigkeit erforderlich. Die Mod startet nur, wenn eine passende Version installiert und aktiviert ist.</p><pre><code>{
  "id": "author.shared-library",
  "version": ">=1.2.0, <2.0.0"
}</code></pre></article><article><b>Optionale Abhängigkeit</b><p>Mit <code>optional: true</code> kann die Mod auch ohne diese Mod oder mit einer inkompatiblen Version starten. Lass das Feld weg oder setze es auf <code>false</code>, wenn die Abhängigkeit nötig ist.</p><pre><code>{
  "id": "author.extra-integration",
  "version": "^3.1.0",
  "optional": true
}</code></pre></article></div><div class="callout info"><b>Was macht der Loader damit?</b><span>Er prüft Installation, Aktivierung und Versionsbereich, plant Abhängigkeiten vor der Mod ein und zeigt ihren Status in den Mod-Details. Eine fehlende, deaktivierte oder unpassende Pflicht-Abhängigkeit blockiert den Start dieser Mod. Optionale Abhängigkeiten blockieren ihn nicht. Das deklariert Verfügbarkeit und Reihenfolge – es importiert keinen Code. Für Modulzugriff gelten weiterhin die Lua-<code>require</code>-Regeln.</span></div><p class="muted">Die Zusatzdatei bleibt für Aktivierung, Einstellungen, Gruppen, Links und Versionshinweise zuständig. Ein minimales Beispiel dafür ist <code>{ "schemaVersion": 1, "enabled": true }</code>. Sie enthält keine Dependency-Liste.</p></section>
    <section class="studio-intro"><div><div class="eyebrow">LIVE-VORSCHAU</div><h2>Stell dir die Modloader-Seite zusammen.</h2><p>Links stellst du die Mod-Infos, alle 12 Einstellungsarten, Gruppen und Badges zusammen. Rechts siehst du sofort, wie dein Mod im Modloader aussehen könnte. Arbeite in drei Schritten: fülle die passenden Formularfelder aus, kontrolliere beide JSON-Dateien und prüfe rechts die Modloader-Vorschau. Du kannst die Dateien jederzeit direkt bearbeiten, einzeln kopieren oder herunterladen. Die Prüfung weist auf häufige Fehler hin. Die verlinkten Schemas enthalten alle genauen Regeln.</p></div><div class="studio-badge"><span class="status-dot"></span> Lokaler Baukasten · deine Eingaben bleiben hier</div></section><div data-page-integration="mod-manifest-editor"></div><section class="json-workbench"><div class="workbench-heading"><div><div class="eyebrow">DATEIEN</div><h3>JSON-Dateien bearbeiten</h3><p>Beide Dateien bleiben sichtbar und lassen sich direkt ändern. Änderungen aus dem Formular erscheinen sofort hier.</p></div><button id="reset-example" class="reset-button">Beispiel zurücksetzen ↻</button></div><div class="json-panes"><article class="json-file"><div class="editor-label"><span>MOD-INFORMATIONEN · <code>mod.json</code></span><div class="editor-actions"><button id="copy-mod-json" class="reset-button">JSON kopieren</button><button id="download-mod-json" class="reset-button">Datei speichern ↓</button></div></div><textarea id="manifest-editor" spellcheck="false" aria-label="mod.json"></textarea></article><article class="json-file"><div class="editor-label"><span>MODLOADER-EINSTELLUNGEN · <code>extended.mod.json</code></span><div class="editor-actions"><button id="copy-extended-json" class="reset-button">JSON kopieren</button><button id="download-extended-json" class="reset-button">Datei speichern ↓</button></div></div><textarea id="extended-editor" spellcheck="false" aria-label="extended.mod.json"></textarea></article></div><div id="validation" class="validation"></div></section><section class="setting-bridge"><div class="eyebrow">VOM EINSTELLUNGSFELD ZUM LUA-CODE</div><h2>So bekommt dein Mod den Wert.</h2><p>Eine Einstellung braucht drei zusammenpassende Namen: den Schlüssel in <code>settings</code>, denselben Schlüssel im Lua-Code und einen Eintrag in einer Gruppe, damit Spieler das Feld sehen.</p><div class="bridge-grid"><article><b>01 · Feld in extended.mod.json</b><p><code>greeting</code> ist der Schlüssel. <code>value</code> ist der Startwert, <code>label</code> der Name im Modloader und <code>control</code> zeigt ein Textfeld.</p><pre><code>{
  "schemaVersion": 1,
  "enabled": false,
  "settings": {
    "greeting": {
      "value": "Hello from my mod!",
      "label": "Greeting",
      "description": "Message shown by the mod.",
      "control": "text",
      "maxLength": 120
    }
  },
  "groups": [{ "label": "Messages", "settings": ["greeting"] }]
}</code></pre></article><article><b>02 · Wert in src/mod.lua lesen</b><p>Der Schlüssel in <code>get</code> muss genau <code>greeting</code> heißen. Das zweite Argument wird verwendet, falls dieser Schlüssel keinen Wert hat.</p><pre><code>runtime.require("runtime.lifecycle")

local function log_greeting()
  local greeting = shroudforge.settings.get(
    "greeting",
    "Hello from my mod!"
  )
  shroudforge.log.info(greeting)
end

shroudforge.ui.on_action("logGreeting", log_greeting)

return {
  on_load = log_greeting,
}</code></pre></article></div><div class="setting-flow" aria-label="So kommt der Einstellungswert im Lua-Code an"><div><b>1 · EINSTELLUNGSDATEI</b><code>value: "Hallo!"</code></div><span>→</span><div><b>2 · MODLOADER</b><small>Spieler ändert den Text</small></div><span>→</span><div><b>3 · GESPEICHERT</b><code>settings.greeting.value</code></div><span>→</span><div><b>4 · LUA</b><code>settings.get("greeting", fallback)</code></div></div><div class="callout info"><b>Was passiert nach dem Ändern?</b><span>Der Modloader speichert den Spielerwert unter <code>settings.greeting.value</code> in <code>extended.mod.json</code>. Der Loader gibt diesen aktuellen Wert an <code>shroudforge.settings.get</code> weiter. Rufe <code>get</code> in einer Laufzeit-Aktion oder einem Callback auf, wenn dein Code Änderungen sofort verwenden soll. Lies den Wert dafür nicht nur einmal beim Laden.</span></div><p class="muted">Die Aktionskennung <code>logGreeting</code> in <code>groups[].actions[].id</code> muss zu <code>shroudforge.ui.on_action</code> passen. Das vollständige Beispiel liegt in <a href="https://github.com/bonsaibauer/shroudforge/tree/HEAD/docs/examples/starter-mod">docs/examples/starter-mod</a>. Ein Mod mit dieser Laufzeit-API braucht außerdem <code>"runtime"</code> in der Liste <code>capabilities</code> in <code>mod.json</code>.</p></section><div class="callout info"><b>Spielerwerte stehen in der Erweiterung</b><span>Der Mod-Ersteller legt in der Zusatzdatei fest, ob eine Einstellung ein Schalter oder Regler ist und wie sie zuerst steht. Der Loader schreibt Änderungen direkt in <code>extended.mod.json</code>. Ein Paket-Update kann sie zurücksetzen.</span></div><h2 class="section-title">Beispiel: eine kurze Einstellungsdatei</h2><pre><code>{
  "schemaVersion": 1,
  "enabled": false,
  "settings": {
    "allowDescent": { "value": false, "control": "toggle" },
    "flightSpeed": { "value": 1.5, "control": "slider", "min": 0.5, "max": 3, "step": 0.1 }
  },
  "groups": [
    { "label": "Flight", "settings": ["allowDescent", "flightSpeed"] }
  ]
}</code></pre><p class="muted">Die Gruppe <code>Flight</code> zeigt beide Felder im Modloader. <code>control</code> wählt hier den Schalter oder Regler aus. Ohne diese Angabe wählt der Modloader ein passendes Feld anhand des Startwerts.</p>