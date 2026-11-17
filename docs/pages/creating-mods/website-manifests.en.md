---
{
  "id": "website-manifests",
  "locale": "en",
  "title": "Manifest editor",
  "summary": "The existing mod and extension manifest editor with preview and download.",
  "view": "manifests",
  "navigation": {
    "path": [
      "MODDING"
    ],
    "label": "Mod settings and preview"
  }
}
---


    <div class="page-intro"><div class="eyebrow">THE TWO INFORMATION FILES</div><h1>Two files with information.<br>Here is what they do.</h1><p class="lead">Every mod has one information card. Some mods have a second file for their own switches and sliders. Both files end in <code>.json</code>, a simple text format.</p></div><div class="manifest-compare"><article><span class="file-icon">{ }</span><h2>mod.json</h2><p>The mod information card: its name, creator, version number, and what it needs.</p><ul><li>Required: <code>id</code>, <code>name</code>, <code>version</code></li><li>Which permissions the mod needs: <code>patch</code>, <code>export</code>, <code>runtime</code>, or <code>runtime-register-dll</code> (for EML mods that include a DLL)</li><li>The basic information file for every mod</li></ul></article><article><span class="file-icon gold">{ + }</span><h2>extended.mod.json</h2><p>This file is optional. Use it only if your mod needs its own switches, sliders, or links to the creator.</p><ul><li>Required: <code>schemaVersion: 1</code></li><li>Values and optional setting details go under <code>settings</code></li><li>Mod updates may reset the saved values</li></ul></article></div><h2 class="section-title">What information goes in mod.json?</h2><div class="data-table"><div><code>id</code> · <code>name</code> · <code>version</code><span>A fixed short name for the mod, the name players see in the game, and the version number.</span></div><div><code>authors</code> · <code>description</code><span>Who made the mod and what it does.</span></div><div><code>dependencies</code><span>Other mods that must be installed first. For a simple mod, leave this empty: <code>[]</code>.</span></div><div><code>capabilities</code><span>Special actions the mod is allowed to take. Only list what it really needs.</span></div><div><code>icon</code> · <code>license</code><span>Both are optional. Use a root icon filename or a path under <code>assets/</code>, plus a license name when known. Use <code>null</code> when there is no value, or leave either field out. See the <a href="https://shroudforge.net/schemas/1.8.0/manifest.schema.json">manifest schema</a>.</span></div></div><h2 class="section-title">What goes in the second file?</h2><div class="data-table"><div><code>schemaVersion</code><span>The current required value is <code>1</code>.</span></div><div><code>enabled</code><span>The current enabled state. Player changes are written here.</span></div><div><code>settings</code><span>Setting names and values. Simple values need no extra type declaration.</span></div><div><code>groups</code><span>Optional: organize settings and Lua-backed action buttons.</span></div><div><code>links</code> · <code>changelog</code><span>Optional project links and version notes.</span></div></div>
    <section class="dependency-guide"><div class="eyebrow">DEPENDENCIES · MOD.JSON</div><h2>Does your mod need another mod?</h2><p>Declare dependencies in <code>mod.json</code>, not in <code>extended.mod.json</code>. For <code>id</code>, use the exact identifier from the other mod's <code>mod.json</code>—not its display name or folder name. <code>version</code> is a SemVer version requirement.</p><div class="dependency-examples"><article><b>Required dependency</b><p>Without <code>optional</code>, the dependency is required. Your mod starts only when a matching version is installed and enabled.</p><pre><code>{
  "id": "author.shared-library",
  "version": ">=1.2.0, <2.0.0"
}</code></pre></article><article><b>Optional dependency</b><p>With <code>optional: true</code>, your mod may start without that mod or with an incompatible version. Omit the field or set it to <code>false</code> when the dependency is required.</p><pre><code>{
  "id": "author.extra-integration",
  "version": "^3.1.0",
  "optional": true
}</code></pre></article></div><div class="callout info"><b>What does the loader do?</b><span>It checks installation, activation, and the version requirement, plans dependencies before the mod, and shows their status in the Mod details. A missing, disabled, or incompatible required dependency blocks this mod from starting. Optional dependencies do not. This declares availability and load order—it does not import code. Lua module access still follows the existing <code>require</code> rules.</span></div><p class="muted">The extension file remains responsible for enabled state, settings, groups, links, and version notes. A minimal example is <code>{ "schemaVersion": 1, "enabled": true }</code>. It has no dependency list.</p></section>
    <section class="studio-intro"><div><div class="eyebrow">LIVE PREVIEW</div><h2>Build a Modloader settings page.</h2><p>Build the mod details, all 12 setting types, groups, actions, and badges on the left. On the right, see how your mod may look in the Modloader. Work in three steps: fill in the matching form, review both JSON files, then check the Modloader preview on the right. You can edit either file directly, copy it, or download it. The builder catches common mistakes. The linked schemas contain the complete rules.</p></div><div class="studio-badge"><span class="status-dot"></span> Local builder · your changes stay here</div></section><div data-page-integration="mod-manifest-editor"></div><section class="json-workbench"><div class="workbench-heading"><div><div class="eyebrow">FILES</div><h3>Edit the JSON files</h3><p>Both files stay visible and can be edited directly. Form changes appear here right away.</p></div><button id="reset-example" class="reset-button">Reset example ↻</button></div><div class="json-panes"><article class="json-file"><div class="editor-label"><span>MOD INFORMATION · <code>mod.json</code></span><div class="editor-actions"><button id="copy-mod-json" class="reset-button">Copy JSON</button><button id="download-mod-json" class="reset-button">Save file ↓</button></div></div><textarea id="manifest-editor" spellcheck="false" aria-label="mod.json"></textarea></article><article class="json-file"><div class="editor-label"><span>MODLOADER SETTINGS · <code>extended.mod.json</code></span><div class="editor-actions"><button id="copy-extended-json" class="reset-button">Copy JSON</button><button id="download-extended-json" class="reset-button">Save file ↓</button></div></div><textarea id="extended-editor" spellcheck="false" aria-label="extended.mod.json"></textarea></article></div><div id="validation" class="validation"></div></section><section class="setting-bridge"><div class="eyebrow">FROM SETTING FIELD TO LUA CODE</div><h2>How does your mod get the value?</h2><p>A setting needs three matching names: the key under <code>settings</code>, the same key in Lua, and an entry in a group so players can see the field.</p><div class="bridge-grid"><article><b>01 · Field in extended.mod.json</b><p><code>greeting</code> is the key. <code>value</code> is the starting value, <code>label</code> is the name in the Modloader, and <code>control</code> shows a text field.</p><pre><code>{
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
}</code></pre></article><article><b>02 · Read the value in src/mod.lua</b><p>The key passed to <code>get</code> must be exactly <code>greeting</code>. The second argument is used if that key has no value.</p><pre><code>runtime.require("runtime.lifecycle")

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
}</code></pre></article></div><div class="setting-flow" aria-label="How a setting value reaches Lua"><div><b>1 · SETTING FILE</b><code>value: "Hello!"</code></div><span>→</span><div><b>2 · MODLOADER</b><small>Player changes the text</small></div><span>→</span><div><b>3 · SAVED VALUE</b><code>settings.greeting.value</code></div><span>→</span><div><b>4 · LUA</b><code>settings.get("greeting", fallback)</code></div></div><div class="callout info"><b>What happens after a player changes it?</b><span>The Modloader saves the player's value at <code>settings.greeting.value</code> in <code>extended.mod.json</code>. The loader supplies that current value to <code>shroudforge.settings.get</code>. Call <code>get</code> in a runtime action or callback when your code should use a change right away. Do not read the value only once when the mod loads.</span></div><p class="muted">The action ID <code>logGreeting</code> in <code>groups[].actions[].id</code> must match <code>shroudforge.ui.on_action</code>. The complete example is in <a href="https://github.com/bonsaibauer/shroudforge/tree/HEAD/docs/examples/starter-mod">docs/examples/starter-mod</a>. A mod that calls this runtime API must also list <code>"runtime"</code> in the <code>capabilities</code> list in <code>mod.json</code>.</p></section><div class="callout info"><b>Your choices stay on your computer</b><span>The mod creator chooses the kind of setting and its starting value in this file. The loader writes player changes into <code>extended.mod.json</code>. A package update can reset them.</span></div><h2 class="section-title">Example: a small settings file</h2><pre><code>{
  "schemaVersion": 1,
  "enabled": false,
  "settings": {
    "allowDescent": { "value": false, "control": "toggle" },
    "flightSpeed": { "value": 1.5, "control": "slider", "min": 0.5, "max": 3, "step": 0.1 }
  },
  "groups": [
    { "label": "Flight", "settings": ["allowDescent", "flightSpeed"] }
  ]
}</code></pre><p class="muted">The <code>Flight</code> group makes both fields visible in the Modloader. Here, <code>control</code> chooses the switch or slider. Without it, the Modloader picks a matching field from the starting value.</p>