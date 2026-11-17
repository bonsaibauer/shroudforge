---
{
  "id": "website-first",
  "locale": "en",
  "title": "Build your first mod",
  "summary": "The existing interactive lesson with examples and steps.",
  "view": "first",
  "navigation": {
    "path": [
      "MODDING"
    ],
    "label": "Build your first mod"
  }
}
---

    <div class="page-intro"><div class="eyebrow">YOUR FIRST MOD</div><h1>Let us build your<br>first small mod.</h1><p class="lead">You do not need programming experience. We will make a folder, copy in a short example, and check that ShroudForge can find it.</p></div><div class="lesson-meta"><span>FOR <b>Beginners</b></span><span>TIME <b>10–15 minutes</b></span><span>YOU NEED <b>A text editor</b></span></div>
    <section class="file-map" aria-label="What the files in your first mod do"><div class="map-heading"><span>THREE SIMPLE PIECES</span><h2>Your first mod, in one picture</h2><p>Each file has one job. ShroudForge reads the information and runs the instruction.</p></div><div class="file-map-row"><article><code>mod.json</code><b>The information card</b><small>Name, version, and the access your mod needs.</small></article><span>+</span><article><code>src/mod.lua</code><b>The instruction</b><small>What your mod asks ShroudForge to do.</small></article><span>+</span><article><code>extended.mod.json</code><b>Optional settings</b><small>Switches and other controls players can change.</small></article></div><div class="file-map-end">ShroudForge reads these files → shows your mod in <kbd>F9</kbd> → writes messages to <kbd>F10</kbd></div></section>
    <div class="lesson-step"><span class="step-num">01</span><div><h2>Make a folder for your mod</h2><p>Open your Enshrouded game folder, then open <code>mods</code>. Make a new folder called <code>yourname.first-mod</code>. This folder is the box that holds all the mod files.</p><pre><code>mods/yourname.first-mod/
├── mod.json
└── src/
    └── mod.lua</code></pre><p><code>mod.json</code> is a small card with information about your mod. <code>mod.lua</code> contains the one instruction in this example.</p></div></div>
    <div class="lesson-step"><span class="step-num">02</span><div><h2>Fill in the mod information card</h2><p>Make a file called <code>mod.json</code> in your new folder and copy this example into it. You can change your name and the description:</p><pre><code>{
  "$schema": "https://shroudforge.net/schemas/1.8.0/manifest.schema.json",
  "id": "yourname.first-mod",
  "name": "My First Mod",
  "version": "1.0.0",
  "authors": ["Your name"],
  "description": "Writes a message.",
  "license": null,
  "icon": null,
  "capabilities": [],
  "dependencies": []
}</code></pre><div class="data-table"><div><code>id</code><span>A unique folder name for the mod. Lowercase is easiest.</span></div><div><code>name</code> · <code>authors</code><span>The name other players see and your name as the creator.</span></div><div><code>version</code><span>The version number. You can leave it as <code>1.0.0</code> for now.</span></div><div><code>description</code><span>A short sentence saying what your mod does.</span></div><div><code>capabilities</code> · <code>dependencies</code><span>What special actions the mod may take, and whether it needs another mod. Leave both empty as <code>[]</code> for this example.</span></div></div></div></div>
    <div class="lesson-step"><span class="step-num">03</span><div><h2>Make it write a message</h2><p>Inside the <code>src</code> folder, make a file called <code>mod.lua</code>. Copy this line into it:</p><pre><code>shroudforge.log.info("My first mod loaded")</code></pre><p>This line asks ShroudForge to put the sentence in its messages. You can just copy it. You do not need to understand how the code works for this example.</p></div></div>
    <div class="lesson-step"><span class="step-num">04</span><div><h2>Look for it in the game</h2><p>Start Enshrouded. Press <kbd>F9</kbd> to open the Mods window. Your mod should be in the list. Press <kbd>F10</kbd> to open the messages and look for the sentence you added.</p><div class="callout warning"><b>Cannot find your mod?</b><span>Check that you copied your new mod folder into the game’s <code>mods</code> folder. Downloaded mods can use different folder layouts, so follow their creator’s instructions.</span></div></div></div>
    <section class="next-panel"><div><div class="eyebrow">YOU DID IT</div><h2>Now your mod can have its own settings.</h2><p>Next, we will show how a switch or slider appears in the Mods window. The two JSON files are explained in simple terms.</p></div><button class="button primary" data-view="manifests">Try the settings editor →</button></section>