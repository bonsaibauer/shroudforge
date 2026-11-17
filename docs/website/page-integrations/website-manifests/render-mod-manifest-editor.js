export function createModEditor({ locale, copy, esc, state, $, $$, editorSession, validateManifests }) {
  const linkBadges = [
    ["source", "Project page", "Projektseite"], ["source-github", "GitHub source", "GitHub-Quellcode"], ["source-gitlab", "GitLab source", "GitLab-Quellcode"], ["source-codeberg", "Codeberg source", "Codeberg-Quellcode"], ["issues", "Issues / report a problem", "Fehler melden"], ["wiki", "Wiki / guide", "Wiki / Anleitung"], ["website", "Website", "Webseite"], ["store", "Store", "Shop"], ["community-discord", "Discord", "Discord"], ["video-youtube", "YouTube", "YouTube"], ["support-bmac", "Buy Me a Coffee", "Buy Me a Coffee"], ["support-patreon", "Patreon", "Patreon"], ["support-paypal", "PayPal", "PayPal"], ["support-github", "GitHub Sponsors", "GitHub Sponsors"], ["support-ko-fi", "Ko-fi", "Ko-fi"], ["support-open-collective", "Open Collective", "Open Collective"], ["support-other", "Other support", "Weitere Unterstützung"]
  ];

  function rebuildManifestStudio() {
    const view = $("#view-manifests");
    const intro = view.querySelector(".studio-intro");
    const studio = view.querySelector(".studio");
    const editorPane = studio.querySelector(".editor-pane");
    const previewPane = studio.querySelector(".preview-pane");
    const builder = $("#builder-tools");
    const workbench = view.querySelector(".json-workbench");
    const oldCompare = view.querySelector(".manifest-compare");
    const oldReferences = [...view.querySelectorAll(":scope > .section-title, :scope > .data-table")];
    const previewStage = document.createElement("section");
    previewStage.className = "preview-stage";
    previewStage.innerHTML = `<div class="stage-heading"><div><span class="eyebrow">${locale === "de" ? "SO SIEHT ES IM MODLOADER AUS" : "MODLOADER PAGE PREVIEW"}</span><h2>${locale === "de" ? "Mod-Details und Einstellungen" : "Mod details and settings"}</h2><p>${locale === "de" ? "Die Vorschau folgt dem Aufbau der echten Mod-Seite im Modloader." : "This preview follows the layout of the actual Mod page in Modloader."}</p></div><div class="preview-tools"><span class="preview-mode"><i></i>${locale === "de" ? "LIVE-VORSCHAU" : "LIVE PREVIEW"}</span><button id="download-package" class="button small primary" type="button">${locale === "de" ? "Mod-Paket herunterladen · ZIP" : "Download mod package · ZIP"}</button></div></div>`;
    previewStage.append(previewPane);
    const makeStage = (name, copy) => {
      const section = document.createElement("section");
      section.className = "manifest-edit-stage";
      section.innerHTML = `<header class="stage-heading"><div><span class="eyebrow">${name}</span><h2>${copy.title}</h2><p>${copy.description}</p></div></header><div class="stage-columns"></div>`;
      return section;
    };
    const modStage = makeStage("MOD.JSON", { title: locale === "de" ? "Mod-Informationen" : "Mod information", description: locale === "de" ? "Bearbeite den JSON-Text direkt oder nutze das Formular daneben. Beide Ansichten bleiben synchron." : "Edit the JSON directly or use the form beside it. Both views stay in sync." });
    const extStage = makeStage("EXTENDED.MOD.JSON", { title: locale === "de" ? "Einstellungen und Modloader-Aktionen" : "Settings and Modloader actions", description: locale === "de" ? "Füge Einstellungen hinzu, passe sie an und ordne sie Gruppen zu. Änderungen erscheinen direkt im Editor." : "Add settings, edit them, and organize them into groups. Changes appear in the editor immediately." });
    const getJsonFile = id => workbench.querySelector(id === "mod" ? "#manifest-editor" : "#extended-editor").closest(".json-file");
    const modFile = getJsonFile("mod");
    const extFile = getJsonFile("extended");
    const modBuilder = $("#mod-builder");
    const extBuilder = document.createElement("div");
    extBuilder.className = "extended-builder";
    const modColumns = modStage.querySelector(".stage-columns");
    const extColumns = extStage.querySelector(".stage-columns");
    modColumns.append(modFile, modBuilder);
    extColumns.append(extFile, extBuilder);
    const iconUpload = document.createElement("div");
    iconUpload.className = "icon-upload builder-wide";
    iconUpload.innerHTML = `<label>${locale === "de" ? "Mod-Icon hochladen" : "Upload mod icon"}<input id="mod-icon-file" type="file" accept=".png,.jpg,.jpeg,.webp,.svg,image/png,image/jpeg,image/webp,image/svg+xml"><small>${locale === "de" ? "PNG, JPG/JPEG, WebP oder SVG · maximal 2 MiB" : "PNG, JPG/JPEG, WebP, or SVG · up to 2 MiB"}</small></label><div class="icon-upload-preview"><span id="icon-preview-empty">${locale === "de" ? "Kein Icon ausgewählt" : "No icon selected"}</span><img id="icon-preview-image" alt="" hidden></div><button id="download-icon" class="button small ghost" type="button" disabled>${locale === "de" ? "Icon-Datei herunterladen" : "Download icon file"}</button>`;
    modBuilder.querySelector("#mod-icon").closest("label").after(iconUpload);
    const dependencyList = document.createElement("div");
    dependencyList.id = "dependency-list";
    dependencyList.className = "dependency-list builder-wide";
    modBuilder.querySelector(".dependency-builder").after(dependencyList);
    const addToExtended = node => { if (node) extBuilder.append(node); };
    addToExtended($("#extended-core"));
    const targetsPicker = document.createElement("fieldset");
    targetsPicker.className = "target-picker builder-wide";
    targetsPicker.innerHTML = `<legend>${locale === "de" ? "Wo läuft dieser Mod? · targets" : "Where does this mod run? · targets"}</legend><small>${locale === "de" ? "Das wählt den Spielprozess. Es synchronisiert keine Änderungen zwischen Spielern." : "This selects the game process. It does not synchronize changes between players."}</small><div><label><input type="checkbox" id="target-client" value="client"> ${locale === "de" ? "Spieler-Client" : "Player client"}</label><label><input type="checkbox" id="target-server" value="server"> Dedicated Server</label></div>`;
    $("#extended-core").append(targetsPicker);
    const settingHeading = [...builder.children].find(node => node.classList?.contains("builder-heading") && (node.textContent.includes("Einstellung hinzufügen") || node.textContent.includes("Add a mod setting")));
    const settingFields = settingHeading?.nextElementSibling;
    if (settingHeading) { addToExtended(settingHeading); addToExtended(settingFields); }
    addToExtended($(".builder-advanced"));
    addToExtended($(".badge-heading"));
    addToExtended($(".builder-extras"));
    addToExtended($("#badge-builder"));
    const items = document.createElement("section");
    items.className = "builder-items";
    items.innerHTML = `<div class="builder-heading"><b>${locale === "de" ? "Vorhandene Einstellungen und Gruppen" : "Current settings and groups"}</b><small>${locale === "de" ? "Einträge lassen sich hier einzeln entfernen." : "Remove individual items here."}</small></div><div id="extended-items-list"></div>`;
    extBuilder.append(items);
    intro.replaceWith(previewStage);
    previewStage.after(modStage, extStage);
    modStage.querySelector(".stage-heading").append($("#reset-example"));
    extStage.after($("#validation"));
    editorPane.remove();
    studio.remove();
    workbench.remove();
    oldCompare?.remove();
    oldReferences.forEach(node => node.remove());
    const referenceHeading = document.createElement("section");
    referenceHeading.className = "schema-reference";
    referenceHeading.innerHTML = `<div class="stage-heading"><div><span class="eyebrow">${locale === "de" ? "DATENREFERENZ" : "DATA REFERENCE"}</span><h2>${locale === "de" ? "Felder, Datentypen und erlaubte Werte" : "Fields, data types, and allowed values"}</h2><p>${locale === "de" ? "Die Kurzreferenz zu den Regeln der beiden JSON-Dateien." : "A quick reference to the rules for both JSON files."}</p></div></div><div id="schema-reference-tables"></div>`;
    extStage.after(referenceHeading);
    referenceHeading.before($("#validation"));
    const bridge = view.querySelector(".setting-bridge");
    const note = view.querySelector(".callout.info");
    if (bridge) referenceHeading.after(bridge);
    if (note) referenceHeading.after(note);
    const exampleHeading = [...view.querySelectorAll(".section-title")].find(node => node.textContent.includes(locale === "de" ? "Beispiel" : "Example"));
    if (exampleHeading) referenceHeading.after(exampleHeading, exampleHeading.nextElementSibling);
    renderSchemaReference();
    renderExtendedItems();
  }

  function changeEditor(mode) {
    state.editor = mode;
    setBuilderTab(mode);
    $$('[data-editor-tab]').forEach(button => button.classList.toggle('active', button.dataset.editorTab === mode));
    if (mode === 'mod') readManifestIntoBuilder();
    else {
      const extended = parseCurrent() || {};
      $('#extended-enabled').checked = Boolean(extended.enabled);
      syncTargetPicker(extended);
    }
    updatePreview();
  }

  function updatePreview() {
    if (!$("#manifest-editor")) return;
    state.manifest = $("#manifest-editor").value;
    state.extended = $("#extended-editor").value;
    let manifest = {}, extended = {}, errors = [];
    try { manifest = JSON.parse(state.manifest); } catch (error) { errors.push(`${copy.editor.labelMod}: ${locale === "de" ? "Bitte prüfe Kommas, Anführungszeichen und Klammern." : "Check the commas, quotation marks, and brackets."}`); }
    try { extended = JSON.parse(state.extended); } catch (error) { errors.push(`${copy.editor.labelExtended}: ${locale === "de" ? "Bitte prüfe Kommas, Anführungszeichen und Klammern." : "Check the commas, quotation marks, and brackets."}`); }
    if (extended && typeof extended === "object") syncTargetPicker(extended);
    if (!errors.length) errors.push(...validateManifests(manifest, extended));
    const validation = $("#validation");
    validation.className = `validation ${errors.length ? "error" : "success"}`;
    validation.innerHTML = errors.length ? `<span>!</span>${esc(errors[0])}${errors.length > 1 ? `<small>${errors.length - 1} ${locale === "de" ? "weitere Hinweise" : "more issue(s)"}</small>` : ""}` : `<span>✓</span>${esc(copy.editor.valid)}`;
    renderPreview(errors.length ? null : manifest, errors.length ? null : extended);
    renderExtendedItems();
  }

  function renderSchemaReference() {
    const root = $("#schema-reference-tables");
    if (!root) return;
    const de = locale === "de";
    const tables = [
      { title: "mod.json", rows: [
        ["id", "string", de ? "Stabile Kennung des Mods." : "Stable mod identifier.", "1–80: A–Z, a–z, 0–9, Punkt, _ , -"],
        ["name", "string", de ? "Anzeigename des Mods." : "Displayed mod name.", "1–120 Zeichen"],
        ["version", "string", de ? "Mod-Version." : "Mod version.", "SemVer, z. B. 1.0.0"],
        ["description", "string", de ? "Beschreibung für die Mod-Seite." : "Description shown on the Mod page.", "max. 2.000 Zeichen"],
        ["authors", "string[]", de ? "Namen der Ersteller." : "Creator names.", "einmalige, nicht leere Namen"],
        ["license", "string | null", de ? "Lizenzkennung." : "License identifier.", "z. B. MIT oder null"],
        ["icon", "string | null", de ? "Dateiname im Paketstamm oder Pfad unter assets/." : "Filename in the package root or path under assets/.", "Bildtypen: PNG, JPG/JPEG, WebP, SVG; max. 2 MiB"],
        ["capabilities", "string[]", de ? "Vom Mod benötigte Berechtigungen." : "Permissions required by the mod.", "patch · export · runtime · runtime-register-dll"],
        ["dependencies", "object[]", de ? "Abhängigkeiten stehen in mod.json. id ist die Kennung aus der mod.json der anderen Mod; version ist ein SemVer-Versionsbereich. Fehlende, deaktivierte oder unpassende Pflicht-Abhängigkeiten blockieren den Start. optional: true blockiert nicht." : "Dependencies belong in mod.json. id is the identifier from the other mod's mod.json; version is a SemVer version requirement. Missing, disabled, or incompatible required dependencies block startup. optional: true does not block startup.", "id + version erforderlich; z. B. ^1.2.0 oder >=1.2.0, <2.0.0; optional: boolean, Standard false"]
      ] },
      { title: "extended.mod.json", rows: [
        ["schemaVersion", "integer", de ? "Version des Erweiterungsformats." : "Extension format version.", "fest: 1"],
        ["enabled", "boolean", de ? "Ob der Mod eingeschaltet ist." : "Whether the mod is enabled.", "true | false"],
        ["targets", "string[]", de ? "Prozesse, in denen der Mod ausgeführt wird. Das ist keine Netzwerk-Synchronisierung." : "Processes in which the mod runs. This is not network synchronization.", de ? "client · server · beide. EML startet standardmäßig auf beiden, ShroudForge standardmäßig auf dem Client." : "client · server · both. EML defaults to both, ShroudForge defaults to the client."],
        ["launcher", "string", de ? "Ursprünglicher Launcher; EML hält die EML-Herkunft sichtbar." : "Original launcher; EML preserves EML provenance.", "EML | SF; omitted defaults to SF"],
        ["settings.<key>", "value | object", de ? "Startwert und optionale Steuerung/Regeln für ein Feld." : "Starting value and optional control/rules for a field.", "Key: 1–80 Zeichen; value erforderlich: boolean, string, number oder array"],
        ["settings.<key>.control", "string", de ? "Steuerelement im Modloader. keybind speichert eine Zeichenfolge, z. B. SDL:64, Mouse:4 oder Ctrl+SDL:29|Mouse:4. Leer deaktiviert das Kürzel." : "Control displayed in Modloader. keybind stores one string, for example SDL:64, Mouse:4 or Ctrl+SDL:29|Mouse:4. Empty disables the shortcut.", "toggle · checkbox · text · textarea · number · slider · select · radio · segmented · multiselect · keybind · color"],
        ["settings.<key>.min / max / step", "number", de ? "Grenzen und Schrittweite numerischer Felder." : "Bounds and step size for numeric controls.", "min/max Zahl; step > 0"],
        ["settings.<key>.minLength / maxLength", "integer", de ? "Längengrenzen für Textfelder." : "Length bounds for text controls.", "0 oder größer"],
        ["settings.<key>.options", "object<string,string>", de ? "Werte und sichtbare Namen für Auswahlfelder." : "Values and labels for choice controls.", "mindestens eine Option; sichtbarer Name 1–120 Zeichen"],
        ["groups[]", "object", de ? "Überschrift, Beschreibung, Einstellungs-Keys und Aktionen." : "Heading, description, setting keys, and actions.", "label erforderlich: 1–120; Beschreibung max. 500; Keys vorhanden und eindeutig"],
        ["groups[].actions[]", "object", de ? "Modloader-Aktionsknopf." : "Modloader action button.", "id + label erforderlich; style: primary | secondary | danger; confirm max. 500"],
        ["links", "object<string,string>", de ? "Webseiten und bekannte Modloader-Badges." : "Web pages and recognized Modloader badges.", "Key: ^[a-z][a-z0-9-]*$; HTTPS-URL; max. 2.048 Zeichen"],
        ["changelog", "string[]", de ? "Versionshinweise." : "Version notes.", "max. 50 Einträge, je 1–500 Zeichen"]
      ] }
    ];
    root.innerHTML = tables.map(table => `<section class="schema-table-section"><h3>${table.title}</h3><div class="schema-table-wrap"><table class="schema-table"><thead><tr><th>${de ? "Feld" : "Field"}</th><th>${de ? "Datentyp" : "Data type"}</th><th>${de ? "Beschreibung" : "Description"}</th><th>${de ? "Erlaubte Werte / Grenzen" : "Allowed values / limits"}</th></tr></thead><tbody>${table.rows.map(row => `<tr>${row.map((cell, index) => `<td class="${index === 0 ? "schema-key" : ""}">${esc(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table></div></section>`).join("");
  }

  function renderExtendedItems() {
    const root = $("#extended-items-list");
    if (!root) return;
    const data = parseCurrent() || {};
    const settings = data.settings && typeof data.settings === "object" ? data.settings : {};
    const groups = Array.isArray(data.groups) ? data.groups : [];
    const settingRows = Object.entries(settings).map(([key, entry]) => `<article class="builder-item"><div><b>${esc(entry?.label || key)}</b><code>${esc(key)}</code><small>${esc(entry?.control || typeof (entry?.value ?? entry))}${entry?.description ? ` · ${esc(entry.description)}` : ""}</small></div><div class="builder-item-actions"><button type="button" class="edit-item" data-edit-setting="${esc(key)}">${locale === "de" ? "Bearbeiten" : "Edit"}</button><button type="button" class="remove-item" data-remove-setting="${esc(key)}" aria-label="${locale === "de" ? "Einstellung entfernen" : "Remove setting"}">×</button></div></article>`).join("");
    const groupRows = groups.map((group, groupIndex) => `<article class="builder-item group-item"><div><b>${esc(group.label || (locale === "de" ? "Gruppe" : "Group"))}</b><small>${(group.settings || []).length} ${locale === "de" ? "Einstellungen" : "settings"} · ${(group.actions || []).length} ${locale === "de" ? "Aktionen" : "actions"}</small><div class="builder-subitems">${(group.settings || []).map(key => `<span>${esc(key)}</span>`).join("")}${(group.actions || []).map((action, actionIndex) => `<span>${esc(action.label || action.id)} <button type="button" class="remove-inline" data-remove-action="${groupIndex}:${actionIndex}" aria-label="${locale === "de" ? "Aktion entfernen" : "Remove action"}">×</button></span>`).join("")}</div></div><button type="button" class="remove-item" data-remove-group="${groupIndex}" aria-label="${locale === "de" ? "Gruppe entfernen" : "Remove group"}">×</button></article>`).join("");
    const changelogRows = (Array.isArray(data.changelog) ? data.changelog : []).map((item, index) => `<article class="builder-item"><div><b>${esc(item)}</b><small>${locale === "de" ? "Versionshinweis" : "Changelog note"}</small></div><button type="button" class="remove-item" data-remove-changelog="${index}" aria-label="${locale === "de" ? "Versionshinweis entfernen" : "Remove changelog note"}">×</button></article>`).join("");
    root.innerHTML = `${settingRows || `<p class="builder-empty">${locale === "de" ? "Noch keine Einstellungen angelegt." : "No settings added yet."}</p>`}${groupRows}${changelogRows}`;
  }

  function parseCurrent() {
    try { return JSON.parse(state.extended); } catch { return null; }
  }

  function setBuilderTab(mode) {
    state.editor = mode;
  }

  function readManifestIntoBuilder() {
    let mod;
    try { mod = JSON.parse(state.manifest); } catch { return; }
    $("#mod-id").value = mod.id || "";
    $("#mod-name").value = mod.name || "";
    $("#mod-version").value = mod.version || "1.0.0";
    $("#mod-authors").value = (mod.authors || []).join("; ");
    $("#mod-description").value = mod.description || "";
    $("#mod-license").value = mod.license || "";
    $("#mod-icon").value = mod.icon || "";
    $$(`[data-capability]`).forEach(input => input.checked = (mod.capabilities || []).includes(input.value));
    renderDependencyList(mod.dependencies || []);
  }

  function renderDependencyList(dependencies) {
    const root = $("#dependency-list");
    if (!root) return;
    root.innerHTML = dependencies.length ? dependencies.map((item, index) => `<div class="dependency-item"><span><b>${esc(item.id)}</b><small>${locale === "de" ? "Version:" : "Version:"} ${esc(item.version)} · ${item.optional ? (locale === "de" ? "optional" : "optional") : (locale === "de" ? "erforderlich" : "required")}</small></span><button type="button" class="remove-item" data-remove-dependency="${index}" aria-label="${locale === "de" ? "Abhängigkeit entfernen" : "Remove dependency"}">×</button></div>`).join("") : `<small>${locale === "de" ? "Noch keine Abhängigkeiten hinzugefügt." : "No dependencies added yet."}</small>`;
  }

  function showUploadedIcon(file, dataUrl) {
    editorSession.uploadedIcon = file ? { file, dataUrl, name: file.name } : null;
    const img = $("#icon-preview-image");
    const empty = $("#icon-preview-empty");
    if (img) { img.src = dataUrl || ""; img.hidden = !dataUrl; }
    if (empty) empty.hidden = Boolean(dataUrl);
    const download = $("#download-icon");
    if (download) download.disabled = !file;
  }

  function updateExtended(mutator) {
    const extended = parseCurrent();
    if (!extended) return;
    mutator(extended);
    state.extended = JSON.stringify(extended, null, 2);
    $("#extended-editor").value = state.extended;
    updatePreview();
    renderBadgeBuilder();
    renderExtendedItems();
  }

  function syncTargetPicker(extended) {
    const targets = Array.isArray(extended.targets) ? extended.targets : extended.launcher === "EML" ? ["client", "server"] : ["client"];
    const client = $("#target-client"), server = $("#target-server");
    if (client) client.checked = targets.includes("client");
    if (server) server.checked = targets.includes("server");
  }

  function renderBadgeBuilder() {
    const root = $("#badge-builder");
    if (!root) return;
    const extended = parseCurrent() || {};
    root.innerHTML = linkBadges.map(([id, en, de]) => `<div class="badge-row"><span><label><input type="checkbox" data-badge="${id}" ${extended.links?.[id] ? "checked" : ""}><b>${locale === "de" ? de : en}</b></label><small>${id}</small></span><input type="url" data-badge-url="${id}" placeholder="https://…" value="${esc(extended.links?.[id] || "")}" ${extended.links?.[id] ? "" : "disabled"}></div>`).join("");
  }

  function renderPreview(manifest, extended) {
    const preview = $("#mod-preview");
    if (!manifest || !extended) {
      preview.innerHTML = `<div class="preview-error"><span>!</span><b>${locale === "de" ? "Vorschau pausiert" : "Preview paused"}</b><small>${locale === "de" ? "Korrigiere die JSON-Fehler, um die Vorschau zu aktualisieren." : "Fix the JSON errors to update the preview."}</small></div>`;
      return;
    }
    const entries = Object.entries(extended.settings || {});
    const groups = Array.isArray(extended.groups) ? extended.groups : [];
    const sectionHtml = groups.map(group => {
      const settings = (group.settings || []).filter(key => Object.hasOwn(extended.settings || {}, key));
      const actions = (group.actions || []).map(action => `<button class="mock-key ${esc(action.style || "secondary")}" title="${esc(action.confirm || "")}">${esc(action.label || action.id)}</button>`).join("");
      return `<section class="mock-group"><header><b>${esc(group.label)}</b>${group.description ? `<small>${esc(group.description)}</small>` : ""}</header>${settings.map(key => renderSetting(key, extended.settings[key])).join("")}${actions ? `<div class="mock-actions">${actions}</div>` : ""}</section>`;
    }).join("");
    const usedSettings = new Set(groups.flatMap(group => group.settings || []));
    const ungroupedEntries = entries.filter(([key]) => !usedSettings.has(key));
    const ungrouped = ungroupedEntries.length ? `<section class="mock-group"><header><b>${locale === "de" ? "Einstellungen" : "Settings"}</b></header>${ungroupedEntries.map(([key, setting]) => renderSetting(key, setting)).join("")}</section>` : "";
    const links = linkBadges.filter(([id]) => extended.links?.[id]).map(([id, en, de]) => `<a class="mock-badge" href="${esc(extended.links[id])}" target="_blank" rel="noreferrer">${esc(locale === "de" ? de : en)} ↗</a>`).join("");
    const changelog = (extended.changelog || []).map(item => `<li>${esc(item)}</li>`).join("");
    const capabilities = manifest.capabilities || [];
    const effectiveTargets = Array.isArray(extended.targets) ? extended.targets : extended.launcher === "EML" ? ["client", "server"] : ["client"];
    const target = effectiveTargets.includes("client") && effectiveTargets.includes("server")
      ? (locale === "de" ? "Client & Server" : "Client & server")
      : effectiveTargets.includes("server") ? "Server" : "Client";
    const runtime = capabilities.includes("runtime") && !capabilities.includes("patch");
    const ecosystem = extended.launcher || "SF";
    const icon = editorSession.uploadedIcon?.dataUrl || "";
    const details = `<div class="mock-details"><div class="mock-section-title">${locale === "de" ? "MOD-DETAILS" : "MOD DETAILS"}</div><div class="mock-meta"><span>${locale === "de" ? "Quelle" : "Ecosystem"}<b>${ecosystem === "EML" ? "EML" : "ShroudForge"}</b></span><span>Version<b>${esc(manifest.version || "0.0.0")}</b></span><span>${locale === "de" ? "Ziel" : "Target"}<b>${target}</b></span><span>${locale === "de" ? "Typ" : "Type"}<b>${runtime ? (locale === "de" ? "Laufzeit-Mod" : "Runtime mod") : (locale === "de" ? "Daten-Mod" : "Asset mod")}</b></span>${manifest.license ? `<span>${locale === "de" ? "Lizenz" : "License"}<b>${esc(manifest.license)}</b></span>` : ""}</div>${manifest.authors?.length ? `<small class="mock-authors">${locale === "de" ? "Erstellt von" : "Created by"}: ${esc(manifest.authors.join(", "))}</small>` : ""}${links ? `<div class="mock-badges">${links}</div>` : ""}${changelog ? `<div class="mock-changelog"><b>${locale === "de" ? "ÄNDERUNGEN" : "WHAT’S NEW"}</b><ul>${changelog}</ul></div>` : ""}</div>`;
    preview.innerHTML = `<header class="mock-page-head"><div><span class="eyebrow">MOD</span><h3>${esc(manifest.name || (locale === "de" ? "Unbenannter Mod" : "Unnamed mod"))}</h3><p>${esc(manifest.description || manifest.id || copy.editor.noDescription)}</p></div><div class="mock-page-actions"><button>${locale === "de" ? "Aktualisieren" : "Refresh"}</button><button>${locale === "de" ? "Speichern" : "Save"}</button></div></header><div class="mock-status-row"><span class="mock-state">${locale === "de" ? "BEREIT" : "READY"}</span><label>${locale === "de" ? "Aktiv" : "Active"}<span class="switch"><input type="checkbox" ${extended.enabled ? "checked" : ""} aria-label="${locale === "de" ? "Mod aktivieren" : "Enable mod"}"><i></i></span></label></div><article class="mock-mod-card"><div class="mock-mod-head"><div class="mock-avatar">${icon ? `<img src="${icon}" alt="">` : esc((manifest.name || "SF").slice(0, 2).toUpperCase())}</div><div class="mock-title"><strong>${esc(manifest.name || "Unnamed mod")}</strong><small>${esc(manifest.id || "mod.id")} · v${esc(manifest.version || "0.0.0")}</small></div></div><p class="mock-description">${esc(manifest.description || copy.editor.noDescription)}</p><div class="mock-divider"></div><div class="mock-section-title">${locale === "de" ? "EINSTELLUNGEN" : "SETTINGS"}<span>${entries.length}</span></div>${sectionHtml}${ungrouped}${entries.length || groups.length ? "" : `<div class="preview-empty">${esc(copy.editor.noSettings)}</div>`}</article>${details}`;
    if (manifest.dependencies?.length) {
      const rows = manifest.dependencies.map(item => `<div class="mock-dependency"><span><b>${esc(item.id)}</b><small>${locale === "de" ? "Benötigt" : "Requires"} ${esc(item.version)}</small></span><span class="mock-dependency-kind ${item.optional ? "optional" : "required"}">${item.optional ? (locale === "de" ? "Optional" : "Optional") : (locale === "de" ? "Erforderlich" : "Required")}</span></div>`).join("");
      preview.insertAdjacentHTML("beforeend", `<section class="mock-details mock-dependencies"><div class="mock-section-title">${locale === "de" ? "ABHÄNGIGKEITEN" : "DEPENDENCIES"}</div>${rows}<small class="mock-dependency-note">${locale === "de" ? "Die echte Modloader-Ansicht ergänzt den Installations- und Laufzeitstatus." : "The actual Modloader view also resolves installation and runtime status."}</small></section>`);
    }
  }

  function renderSetting(key, setting) {
    const metadata = setting && typeof setting === "object" && !Array.isArray(setting) ? setting : {};
    const value = Object.hasOwn(metadata, "value") ? metadata.value : setting;
    const type = typeof value === "boolean" ? "boolean" : typeof value === "number" ? Number.isInteger(value) ? "integer" : "number" : Array.isArray(value) ? "array" : "string";
    const control = metadata.control || (metadata.options ? "select" : type === "boolean" ? "toggle" : type === "number" || type === "integer" ? "number" : "text");
    const title = esc(metadata.label || key.replace(/([A-Z])/g, " $1").replace(/^./, char => char.toUpperCase()));
    const description = esc(metadata.description || "");
    let input = "";
    if (["toggle", "checkbox"].includes(control)) input = `<label class="switch"><input type="checkbox" ${value ? "checked" : ""} aria-label="${title}"><i></i></label>`;
    else if (control === "slider") input = `<div class="mock-slider"><input type="range" min="${esc(metadata.min ?? 0)}" max="${esc(metadata.max ?? 100)}" step="${esc(metadata.step ?? 0.1)}" value="${esc(value)}" aria-label="${title}"><output>${esc(value)}</output></div>`;
    else if (control === "multiselect") {
      const selected = Array.isArray(value) ? value : [];
      input = `<div class="mock-multiselect">${Object.entries(metadata.options || {}).map(([option, label]) => `<label><input type="checkbox" ${selected.includes(option) ? "checked" : ""}>${esc(label)}</label>`).join("")}</div>`;
    } else if (["select", "radio", "segmented"].includes(control)) {
      const options = Object.entries(metadata.options || {}).map(([option, label]) => [option, label]);
      input = control === "select" ? `<select aria-label="${title}">${options.map(([option, label]) => `<option value="${esc(option)}" ${String(option) === String(value) ? "selected" : ""}>${esc(label)}</option>`).join("")}</select>` : `<div class="mock-choices ${control}">${options.map(([option, label]) => `<button class="mock-choice ${String(option) === String(value) ? "active" : ""}">${esc(label)}</button>`).join("")}</div>`;
    } else if (control === "color") input = `<input type="color" value="${esc(value || "#c9ad6e")}" aria-label="${title}">`;
    else if (control === "keybind") input = `<input class="mock-input mock-keybind" value="${esc(value || "")}" readonly placeholder="${locale === "de" ? "Deaktiviert" : "Disabled"}" title="${locale === "de" ? "Gespeicherte Tastatur- und/oder Mausbelegung. SDL zeigt die Beschriftungen im Modloader." : "Stored keyboard and/or mouse binding. SDL displays the labels in Modloader."}" aria-label="${title}">`;
    else if (control === "textarea") input = `<textarea minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" placeholder="${title}">${esc(value)}</textarea>`;
    else input = `<input class="mock-input" type="${type === "number" || type === "integer" ? "number" : "text"}" min="${esc(metadata.min ?? "")}" max="${esc(metadata.max ?? "")}" step="${esc(metadata.step ?? "")}" minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" value="${esc(value)}" aria-label="${title}">`;
    return `<div class="mock-setting"><div class="mock-setting-copy"><b>${title}</b>${description ? `<small>${description}</small>` : ""}</div><div class="mock-control">${input}</div></div>`;
  }

  return { rebuildManifestStudio, changeEditor, updatePreview, parseCurrent, setBuilderTab, readManifestIntoBuilder, showUploadedIcon, updateExtended, syncTargetPicker, renderBadgeBuilder };
}
