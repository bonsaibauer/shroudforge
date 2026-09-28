import de from "./content.de.js";
import en from "./content.en.js";

const locale = document.documentElement.dataset.locale === "de" ? "de" : "en";
const copy = locale === "de" ? de : en;
const otherLocale = locale === "de" ? "en" : "de";
const siteRoot = new URL("./", import.meta.url);
const pageIds = ["home", "play", "server", "workings", "first", "manifests", "community", "api"];
const state = { page: "home", catalog: "api", apiSource: "all", api: [], profile: null, types: {}, resources: {}, manifest: null, extended: null, editor: "mod" };
const esc = value => String(value ?? "").replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
const fmt = value => Number(value || 0).toLocaleString(locale === "de" ? "de-DE" : "en-US");

document.documentElement.lang = locale;
document.title = locale === "de" ? "ShroudForge — Spieler- und Modding-Guide" : "ShroudForge — Player and modding guide";
document.querySelector('meta[name="description"]')?.setAttribute("content", locale === "de"
  ? "ShroudForge einrichten, Mods installieren und eigene EML- und ShroudForge-Mods entwickeln."
  : "Set up ShroudForge, install mods, and build your own EML and ShroudForge mods.");
localStorage.setItem("sf-language", locale);

document.querySelector("#app").innerHTML = `
  <header class="topbar"><a href="#home" class="brand" data-view="home" aria-label="ShroudForge home"><img src="../media/shroudforge-mark.svg" alt=""><span><b>SHROUDFORGE</b><small>${copy.brand}</small></span></a><div class="top-actions"><a class="top-link" href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><a class="top-link release-link" href="https://github.com/bonsaibauer/shroudforge/releases/latest" target="_blank" rel="noreferrer">${copy.download} ↗</a><a class="language-switch" href="../${otherLocale}/#${location.hash.slice(1) || "home"}" aria-label="${copy.languageLabel}">${copy.language}<span>⌄</span></a><button class="mobile-menu" id="mobile-menu" aria-label="${copy.menu}">☰</button></div></header>
  <aside class="sidebar"><div class="sidebar-label">${copy.menu}</div><nav>${copy.nav.map((label, i) => `<button class="nav-item ${i === 0 ? "active" : ""}" data-view="${pageIds[i]}"><span class="nav-index">0${i + 1}</span><span>${label}</span>${i === 0 ? '<i class="nav-glow"></i>' : ""}</button>`).join("")}</nav><div class="sidebar-bottom"><div class="sidebar-mark"><span class="status-dot"></span><span>ENSHROUDED<br><small>MODDING PLATFORM</small></span></div><a href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a></div></aside>
  <main class="main-content"><div class="content-wrap">${pageIds.map(id => `<section id="view-${id}" class="view ${id === "home" ? "active" : ""}">${copy[id]}</section>`).join("")}<footer class="site-footer"><span>© SHROUDFORGE · ${copy.footer}</span><span><a href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><b>·</b><a href="#api" data-view="api">${copy.nav[7]}</a></span></footer></div></main>`;

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
function setPage(page, updateHash = true) {
  if (!pageIds.includes(page)) page = "home";
  state.page = page;
  $$(".view").forEach(view => view.classList.toggle("active", view.id === `view-${page}`));
  $$(".nav-item").forEach(item => item.classList.toggle("active", item.dataset.view === page));
  if (updateHash) history.replaceState(null, "", `#${page}`);
  $(".sidebar").classList.remove("open");
  window.scrollTo({ top: 0, behavior: "smooth" });
  if (page === "api") loadCatalog();
}

document.addEventListener("click", event => {
  const viewButton = event.target.closest("[data-view]");
  if (viewButton) { event.preventDefault(); setPage(viewButton.dataset.view); }
  const catalogButton = event.target.closest("[data-catalog]");
  if (catalogButton) setCatalog(catalogButton.dataset.catalog);
  const sourceButton = event.target.closest("[data-api-source]");
  if (sourceButton) {
    state.apiSource = sourceButton.dataset.apiSource;
    $$("[data-api-source]").forEach(button => button.classList.toggle("active", button === sourceButton));
    renderApi($("#api-search").value);
  }
  const editorTab = event.target.closest("[data-editor-tab]");
  if (editorTab) changeEditor(editorTab.dataset.editorTab);
});
$("#mobile-menu").addEventListener("click", () => $(".sidebar").classList.toggle("open"));
$(".language-switch").addEventListener("click", () => localStorage.setItem("sf-language", otherLocale));
window.addEventListener("hashchange", () => setPage(location.hash.slice(1), false));
setPage(pageIds.includes(location.hash.slice(1)) ? location.hash.slice(1) : "home", false);

const exampleManifest = {
  $schema: "../../src/loader/package/src/registry/manifest.schema.json",
  id: "example.hello-ember",
  name: "Hello Ember",
  version: "1.0.0",
  description: "A small ShroudForge starter mod that demonstrates saved settings, a Modloader action, and runtime logging.",
  authors: ["Your Name"],
  license: null,
  icon: "icon.svg",
  dependencies: [],
  capabilities: ["runtime"]
};
const exampleExtended = {
  $schema: "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  schemaVersion: 1,
  enabled: false,
  links: {
    "source-github": "https://github.com/your-name/hello-ember",
    issues: "https://github.com/your-name/hello-ember/issues"
  },
  changelog: ["Initial example: saved settings, a grouped Modloader action, and runtime logging."],
  settings: {
    showOnLoad: {
      value: true,
      label: "Show greeting on load",
      description: "Write the greeting to the mod log when the runtime starts.",
      control: "toggle"
    },
    greeting: {
      value: "Hello from the ShroudForge runtime!",
      label: "Greeting",
      description: "Message written by the startup callback and the button below.",
      control: "text",
      minLength: 1,
      maxLength: 120
    },
    severity: {
      value: "info",
      label: "Log level",
      description: "Choose how the example message appears in the mod log.",
      options: { info: "Information", warn: "Warning" }
    }
  },
  groups: [{
    label: "Greeting",
    description: "A working example of settings connected to runtime Lua code.",
    settings: ["showOnLoad", "greeting", "severity"],
    actions: [{ id: "logGreeting", label: "Write greeting to mod log", style: "secondary" }]
  }]
};state.manifest = JSON.stringify(exampleManifest, null, 2);
state.extended = JSON.stringify(exampleExtended, null, 2);

function changeEditor(mode) {
  if (state.editor === "mod") state.manifest = $("#manifest-editor").value;
  else state.extended = $("#manifest-editor").value;
  state.editor = mode;
  $("#builder-tools").hidden = false;
  setBuilderTab(mode);
  $$("[data-editor-tab]").forEach(button => button.classList.toggle("active", button.dataset.editorTab === mode));
  $("#editor-label").textContent = mode === "mod" ? copy.editor.labelMod : copy.editor.labelExtended;
  $("#manifest-editor").value = mode === "mod" ? state.manifest : state.extended;
  if (mode === "mod") readManifestIntoBuilder();
  else $("#extended-enabled").checked = Boolean(parseCurrent()?.enabled);
  updatePreview();
}

function updatePreview() {
  if (!$("#manifest-editor")) return;
  if (state.editor === "mod") state.manifest = $("#manifest-editor").value;
  else state.extended = $("#manifest-editor").value;
  let manifest = {}, extended = {}, errors = [];
  try { manifest = JSON.parse(state.manifest); } catch (error) { errors.push(`${copy.editor.labelMod}: ${locale === "de" ? "Bitte prüfe Kommas, Anführungszeichen und Klammern." : "Check the commas, quotation marks, and brackets."}`); }
  try { extended = JSON.parse(state.extended); } catch (error) { errors.push(`${copy.editor.labelExtended}: ${locale === "de" ? "Bitte prüfe Kommas, Anführungszeichen und Klammern." : "Check the commas, quotation marks, and brackets."}`); }
  if (!errors.length) errors.push(...validateManifests(manifest, extended));
  const validation = $("#validation");
  validation.className = `validation ${errors.length ? "error" : "success"}`;
  validation.innerHTML = errors.length ? `<span>!</span>${esc(errors[0])}${errors.length > 1 ? `<small>${errors.length - 1} ${locale === "de" ? "weitere Hinweise" : "more issue(s)"}</small>` : ""}` : `<span>✓</span>${esc(copy.editor.valid)}`;
  renderPreview(errors.length ? null : manifest, errors.length ? null : extended);
}

function validateManifests(manifest, extended) {
  const errors = [];
  const fail = message => errors.push(message);
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) return ["mod.json must start with an object: { ... }."];
  if (!extended || typeof extended !== "object" || Array.isArray(extended)) return ["extended.mod.json must start with an object: { ... }."];
  const checkKeys = (object, allowed, label) => { for (const key of Object.keys(object || {})) if (!allowed.includes(key)) fail(`${label}: ${locale === "de" ? "unbekanntes Feld" : "unknown field"} “${key}”.`); };
  checkKeys(manifest, ["$schema", "id", "name", "version", "authors", "description", "license", "icon", "dependencies", "capabilities"], "mod.json");
  if (manifest["$schema"] !== undefined && typeof manifest["$schema"] !== "string") fail("mod.json: $schema must be a text address.");
  if (typeof manifest.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(manifest.id)) fail(locale === "de" ? "mod.json: id braucht 1–80 Buchstaben, Zahlen, Punkte, Bindestriche oder Unterstriche." : "mod.json: id must use 1–80 letters, numbers, dots, hyphens, or underscores.");
  if (typeof manifest.name !== "string" || manifest.name.length < 1 || manifest.name.length > 120) fail("mod.json: name must contain 1–120 characters.");
  if (typeof manifest.version !== "string" || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(manifest.version)) fail("mod.json: version must look like 1.0.0.");
  if (manifest.description !== undefined && (typeof manifest.description !== "string" || manifest.description.length > 2000)) fail("mod.json: description allows up to 2,000 characters.");
  if (manifest.authors !== undefined && (!Array.isArray(manifest.authors) || manifest.authors.some(value => typeof value !== "string" || !value) || new Set(manifest.authors).size !== manifest.authors.length)) fail("mod.json: authors must be a list of unique, non-empty names.");
  if (manifest.license !== undefined && manifest.license !== null && typeof manifest.license !== "string") fail("mod.json: license must be text or null.");
  if (manifest.icon !== undefined && manifest.icon !== null && (typeof manifest.icon !== "string" || !/^(?:assets\/[A-Za-z0-9._/-]+|[A-Za-z0-9_-][A-Za-z0-9._-]*)$/.test(manifest.icon))) fail("mod.json: icon must be a file name or a path under assets/.");
  if (manifest.capabilities !== undefined && (!Array.isArray(manifest.capabilities) || manifest.capabilities.some(value => !["patch", "export", "runtime", "runtime-register-dll"].includes(value)) || new Set(manifest.capabilities).size !== manifest.capabilities.length)) fail("mod.json: capabilities can contain patch, export, runtime, and runtime-register-dll once each.");
  if (manifest.dependencies !== undefined && (!Array.isArray(manifest.dependencies) || manifest.dependencies.some(item => !item || typeof item !== "object" || Array.isArray(item) || Object.keys(item).some(key => !["id", "version", "optional"].includes(key)) || typeof item.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(item.id) || typeof item.version !== "string" || (item.optional !== undefined && typeof item.optional !== "boolean")))) fail("mod.json: each dependency needs an id and version; optional can be true or false.");
  checkKeys(extended, ["$schema", "schemaVersion", "enabled", "links", "changelog", "settings", "groups"], "extended.mod.json");
  if (extended.schemaVersion !== 1) fail("extended.mod.json: schemaVersion must be 1.");
  if (extended["$schema"] !== undefined && typeof extended["$schema"] !== "string") fail("extended.mod.json: $schema must be a text address.");
  if (typeof extended.enabled !== "boolean") fail("extended.mod.json: enabled must be true or false.");
  if (extended.links !== undefined && (!extended.links || typeof extended.links !== "object" || Array.isArray(extended.links) || Object.entries(extended.links).some(([key, url]) => !/^[a-z][a-z0-9-]*$/.test(key) || typeof url !== "string" || !url.startsWith("https://") || url.length > 2048))) fail("extended.mod.json: link names must be lowercase and each link must be an HTTPS address.");
  if (extended.changelog !== undefined && (!Array.isArray(extended.changelog) || extended.changelog.length > 50 || extended.changelog.some(item => typeof item !== "string" || !item || item.length > 500))) fail("extended.mod.json: changelog allows up to 50 notes, each 1–500 characters.");
  const settings = extended.settings || {};
  if (extended.settings !== undefined && (!settings || typeof settings !== "object" || Array.isArray(settings))) fail("extended.mod.json: settings must be an object.");
  const controls = ["toggle", "checkbox", "text", "textarea", "number", "slider", "select", "radio", "segmented", "multiselect", "keybind", "color"];
  for (const [key, setting] of Object.entries(settings)) {
    if (!/^[A-Za-z0-9._-]{1,80}$/.test(key)) fail(`Setting “${key}”: use 1–80 letters, numbers, dots, hyphens, or underscores.`);
    if (setting && typeof setting === "object" && !Array.isArray(setting)) {
      checkKeys(setting, ["value", "label", "description", "control", "min", "max", "step", "minLength", "maxLength", "options"], `Setting “${key}”`);
      if (!("value" in setting) || !["boolean", "string", "number"].includes(typeof setting.value) && !Array.isArray(setting.value)) fail(`Setting “${key}” needs a value.`);
      if (setting.label !== undefined && (typeof setting.label !== "string" || !setting.label || setting.label.length > 120)) fail(`Setting “${key}”: label allows 1–120 characters.`);
      if (setting.description !== undefined && (typeof setting.description !== "string" || setting.description.length > 500)) fail(`Setting “${key}”: description allows up to 500 characters.`);
      if (setting.control !== undefined && !controls.includes(setting.control)) fail(`Setting “${key}”: choose one of the listed controls.`);
      for (const field of ["min", "max"]) if (setting[field] !== undefined && typeof setting[field] !== "number") fail(`Setting “${key}”: ${field} must be a number.`);
      if (setting.step !== undefined && (typeof setting.step !== "number" || setting.step <= 0)) fail(`Setting “${key}”: step must be greater than zero.`);
      for (const field of ["minLength", "maxLength"]) if (setting[field] !== undefined && (!Number.isInteger(setting[field]) || setting[field] < 0)) fail(`Setting “${key}”: ${field} must be zero or a positive whole number.`);
      if (setting.options !== undefined && (!setting.options || typeof setting.options !== "object" || Array.isArray(setting.options) || !Object.keys(setting.options).length || Object.values(setting.options).some(value => typeof value !== "string" || !value || value.length > 120))) fail(`Setting “${key}”: options need at least one named choice.`);
    } else if (!["boolean", "string", "number"].includes(typeof setting) && !Array.isArray(setting)) fail(`Setting “${key}” needs a text, number, switch, list, or setting object.`);
  }
  if (extended.groups !== undefined && !Array.isArray(extended.groups)) fail("extended.mod.json: groups must be a list.");
  const groups = Array.isArray(extended.groups) ? extended.groups : [];
  for (const group of groups) {
    if (!group || typeof group !== "object" || Array.isArray(group)) { fail("Each group must be an object."); continue; }
    checkKeys(group, ["label", "description", "settings", "actions"], "Group");
    if (typeof group.label !== "string" || !group.label || group.label.length > 120) fail("Group: label allows 1–120 characters.");
    if (group.description !== undefined && (typeof group.description !== "string" || group.description.length > 500)) fail("Group: description allows up to 500 characters.");
    if (group.settings !== undefined && (!Array.isArray(group.settings) || group.settings.some(key => typeof key !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(key) || !(key in settings)) || new Set(group.settings).size !== group.settings.length)) fail("Group: settings must be unique names that exist in the settings list.");
    if (group.actions !== undefined && (!Array.isArray(group.actions) || group.actions.some(action => !action || typeof action !== "object" || Array.isArray(action) || Object.keys(action).some(key => !["id", "label", "style", "confirm"].includes(key)) || typeof action.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(action.id) || typeof action.label !== "string" || !action.label || action.label.length > 120 || (action.style !== undefined && !["primary", "secondary", "danger"].includes(action.style)) || (action.confirm !== undefined && (typeof action.confirm !== "string" || action.confirm.length > 500))))) fail("Group: each action needs an id and label; style and confirmation are optional.");
  }
  return errors;
}
const linkBadges = [
  ["source", "Project page", "Projektseite"], ["source-github", "GitHub source", "GitHub-Quellcode"], ["source-gitlab", "GitLab source", "GitLab-Quellcode"], ["source-codeberg", "Codeberg source", "Codeberg-Quellcode"], ["issues", "Issues / report a problem", "Fehler melden"], ["wiki", "Wiki / guide", "Wiki / Anleitung"], ["website", "Website", "Webseite"], ["store", "Store", "Shop"], ["support-bmac", "Buy Me a Coffee", "Buy Me a Coffee"], ["support-patreon", "Patreon", "Patreon"], ["support-paypal", "PayPal", "PayPal"], ["support-github", "GitHub Sponsors", "GitHub Sponsors"], ["support-ko-fi", "Ko-fi", "Ko-fi"], ["support-open-collective", "Open Collective", "Open Collective"], ["support-other", "Other support", "Weitere Unterstützung"]
];
function parseCurrent() {
  try { return JSON.parse(state.extended); } catch { return null; }
}
function syncEditorFromExtended() {
  state.extended = JSON.stringify(parseCurrent(), null, 2);
  if (state.editor === "extended") $("#manifest-editor").value = state.extended;
  updatePreview();
  renderBadgeBuilder();
}
function setBuilderTab(mode) {
  $("#mod-builder").hidden = mode !== "mod";
  $("#extended-core").hidden = mode !== "extended";
  $("#badge-builder").hidden = mode === "mod";
  $("#builder-tools > .builder-heading").hidden = mode === "mod";
  $("#builder-tools > .builder-grid").hidden = mode === "mod";
  $(".builder-advanced").hidden = mode === "mod";
  $(".badge-heading").hidden = mode === "mod";
  $(".builder-extras").hidden = mode === "mod";
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
}
$("#add-dependency")?.addEventListener("click", () => {
  const id = $("#dependency-id").value.trim();
  const version = $("#dependency-version").value.trim();
  if (!id || !version) { alert(locale === "de" ? "Bitte trage Mod-Kennung und Version ein." : "Please enter the mod ID and version."); return; }
  let mod; try { mod = JSON.parse(state.manifest); } catch { return; }
  mod.dependencies ||= [];
  const dependency = { id, version };
  if ($("#dependency-optional").checked) dependency.optional = true;
  mod.dependencies.push(dependency);
  state.manifest = JSON.stringify(mod, null, 2);
  if (state.editor === "mod") $("#manifest-editor").value = state.manifest;
  $("#dependency-id").value = ""; $("#dependency-version").value = ""; $("#dependency-optional").checked = false;
  updatePreview();
});
$("#apply-mod-info")?.addEventListener("click", () => {
  let mod;
  try { mod = JSON.parse(state.manifest); } catch { alert(locale === "de" ? "Bitte korrigiere zuerst mod.json." : "Please fix mod.json first."); return; }
  mod.id = $("#mod-id").value.trim();
  mod.name = $("#mod-name").value.trim();
  mod.version = $("#mod-version").value.trim();
  const authors = $("#mod-authors").value.split(";").map(value => value.trim()).filter(Boolean);
  if (authors.length) mod.authors = authors; else delete mod.authors;
  for (const [key, selector] of [["description", "#mod-description"], ["license", "#mod-license"], ["icon", "#mod-icon"]]) {
    const value = $(selector).value.trim();
    if (value) mod[key] = value; else delete mod[key];
  }
  mod.capabilities = $$(`[data-capability]:checked`).map(input => input.value);
  state.manifest = JSON.stringify(mod, null, 2);
  if (state.editor === "mod") $("#manifest-editor").value = state.manifest;
  updatePreview();
});
function updateExtended(mutator) {
  const extended = parseCurrent();
  if (!extended) return;
  mutator(extended);
  state.extended = JSON.stringify(extended, null, 2);
  if (state.editor === "extended") $("#manifest-editor").value = state.extended;
  updatePreview();
  renderBadgeBuilder();
}
$("#add-group")?.addEventListener("click", () => {
  const label = $("#group-label").value.trim();
  if (!label) return;
  updateExtended(extended => { extended.groups ||= []; const group = { label, settings: [] }; const description = $("#group-description").value.trim(); if (description) group.description = description; extended.groups.push(group); });
  $("#group-label").value = "";
  $("#group-description").value = "";
});
$("#add-action")?.addEventListener("click", () => {
  const id = $("#action-id").value.trim();
  const label = $("#action-label").value.trim();
  if (!id || !label) { alert(locale === "de" ? "Bitte trage eine Knopf-ID und einen Knopftext ein." : "Please enter a button ID and button text."); return; }
  updateExtended(extended => {
    extended.groups ||= [];
    const groupName = $("#action-group").value.trim() || (locale === "de" ? "Einstellungen" : "Settings");
    let group = extended.groups.find(item => item.label === groupName);
    if (!group) { group = { label: groupName, settings: [] }; extended.groups.push(group); }
    group.actions ||= [];
    const action = { id, label, style: $("#action-style").value };
    const confirm = $("#action-confirm").value.trim();
    if (confirm) action.confirm = confirm;
    group.actions.push(action);
  });
});
$("#add-changelog")?.addEventListener("click", () => {
  const item = $("#changelog-item").value.trim();
  if (!item) return;
  updateExtended(extended => { extended.changelog ||= []; extended.changelog.push(item); });
  $("#changelog-item").value = "";
});
function renderBadgeBuilder() {
  const root = $("#badge-builder");
  if (!root) return;
  const extended = parseCurrent() || {};
  root.innerHTML = linkBadges.map(([id, en, de]) => `<div class="badge-row"><span><label><input type="checkbox" data-badge="${id}" ${extended.links?.[id] ? "checked" : ""}><b>${locale === "de" ? de : en}</b></label><small>${id}</small></span><input type="url" data-badge-url="${id}" placeholder="https://…" value="${esc(extended.links?.[id] || "")}" ${extended.links?.[id] ? "" : "disabled"}></div>`).join("");
}
$("#apply-extended-core")?.addEventListener("click", () => updateExtended(extended => { extended.enabled = $("#extended-enabled").checked; }));$("#add-setting")?.addEventListener("click", () => {
  const extended = parseCurrent();
  if (!extended) return;
  const key = $("#setting-key").value.trim();
  const control = $("#setting-control").value;
  if (!/^[A-Za-z0-9._-]{1,80}$/.test(key)) { alert(locale === "de" ? "Bitte nutze für den Einstellungsnamen 1 bis 80 Buchstaben, Zahlen, Punkte, Bindestriche oder Unterstriche." : "Use 1 to 80 letters, numbers, dots, hyphens, or underscores for the setting name."); return; }
  let options = {};
  for (const line of $("#setting-options").value.split(/\r?\n/).map(value => value.trim()).filter(Boolean)) {
    const [option, ...label] = line.split("=");
    options[option.trim()] = (label.join("=").trim() || option.trim());
  }
  const raw = $("#setting-value").value.trim();
  let value = raw;
  if (["toggle", "checkbox"].includes(control)) value = raw.toLowerCase() === "true";
  else if (["number", "slider"].includes(control)) value = Number(raw || 0);
  else if (control === "multiselect") value = raw ? raw.split(",").map(item => item.trim()).filter(Boolean) : [];
  else if (control === "color") value = raw || "#c9ad6e";
  else if (control === "keybind") value = raw || "F";
  else if (["select", "radio", "segmented"].includes(control) && Object.keys(options).length) value = Object.keys(options).includes(raw) ? raw : Object.keys(options)[0];
  const setting = { value, label: $("#setting-label").value.trim() || key, control };
  const description = $("#setting-description").value.trim();
  if (description) setting.description = description;
  for (const field of ["min", "max", "step", "minLength", "maxLength"]) {
    const element = $(`#setting-${field.replace("Length", "-length").toLowerCase()}`);
    if (element.value !== "") setting[field] = Number(element.value);
  }
  if (Object.keys(options).length) setting.options = options;
  extended.settings ||= {};
  extended.settings[key] = setting;
  if (!Array.isArray(extended.groups)) extended.groups = [];
  if (!extended.groups.length) extended.groups.push({ label: locale === "de" ? "Einstellungen" : "Settings", settings: [] });
  const groupName = $("#setting-group").value.trim();
  const targetGroup = groupName ? (extended.groups.find(group => group.label === groupName) || (extended.groups.push({ label: groupName, settings: [] }), extended.groups.at(-1))) : extended.groups[0];
  targetGroup.settings ||= [];
  if (!targetGroup.settings.includes(key)) targetGroup.settings.push(key);
  state.extended = JSON.stringify(extended, null, 2);
  if (state.editor === "extended") $("#manifest-editor").value = state.extended;
  updatePreview();
});
$("#badge-builder")?.addEventListener("change", event => {
  const target = event.target;
  if (target.matches("[data-badge]")) {
    const url = $(`[data-badge-url="${target.dataset.badge}"]`);
    url.disabled = !target.checked;
    if (!target.checked) { url.value = ""; updateExtended(extended => { if (extended.links) delete extended.links[target.dataset.badge]; if (extended.links && !Object.keys(extended.links).length) delete extended.links; }); }
    else url.focus();
  }
  if (target.matches("[data-badge-url]")) {
    const extended = parseCurrent();
    if (!extended) return;
    const { badgeUrl } = target.dataset;
    extended.links ||= {};
    if (target.value.trim()) extended.links[badgeUrl] = target.value.trim();
    else delete extended.links[badgeUrl];
    if (!Object.keys(extended.links).length) delete extended.links;
    state.extended = JSON.stringify(extended, null, 2);
    if (state.editor === "extended") $("#manifest-editor").value = state.extended;
    updatePreview();
  }
});
$("#badge-builder")?.addEventListener("input", event => {
  if (event.target.matches("[data-badge-url]")) event.target.dispatchEvent(new Event("change", { bubbles: true }));
});
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
  const ungrouped = entries.filter(([key]) => !usedSettings.has(key)).map(([key, setting]) => renderSetting(key, setting)).join("");
  const links = linkBadges.filter(([id]) => extended.links?.[id]).map(([id, en, de]) => `<a class="mock-badge" href="${esc(extended.links[id])}" target="_blank" rel="noreferrer">${esc(locale === "de" ? de : en)} ↗</a>`).join("");
  const changelog = (extended.changelog || []).map(item => `<li>${esc(item)}</li>`).join("");
  preview.innerHTML = `<div class="mock-mod-head"><div class="mock-avatar">${esc((manifest.name || "SF").slice(0, 2).toUpperCase())}</div><div class="mock-title"><strong>${esc(manifest.name || "Unnamed mod")}</strong><small>${esc(manifest.id || "mod.id")} · v${esc(manifest.version || "0.0.0")}</small></div><label class="switch"><input type="checkbox" ${extended.enabled ? "checked" : ""} aria-label="${locale === "de" ? "Mod aktivieren" : "Enable mod"}"><i></i></label></div><p class="mock-description">${esc(manifest.description || copy.editor.noDescription)}</p>${manifest.authors?.length ? `<small class="mock-authors">${esc(manifest.authors.join(", "))}</small>` : ""}<div class="mock-divider"></div><div class="mock-section-title">${locale === "de" ? "EINSTELLUNGEN" : "SETTINGS"}<span>${entries.length}</span></div>${sectionHtml}${ungrouped}${entries.length || groups.length ? "" : `<div class="preview-empty">${esc(copy.editor.noSettings)}</div>`}${links ? `<div class="mock-badges">${links}</div>` : ""}${changelog ? `<div class="mock-changelog"><b>${locale === "de" ? "ÄNDERUNGEN" : "WHAT\u2019S NEW"}</b><ul>${changelog}</ul></div>` : ""}<div class="mock-footer"><span class="status-dot"></span>${locale === "de" ? "Änderungen werden in extended.mod.json gespeichert" : "Changes are saved in extended.mod.json"}</div>`;
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
  else if (control === "keybind") input = `<input class="mock-input mock-keybind" value="${esc(value || "")}" readonly aria-label="${title}">`;
  else if (control === "textarea") input = `<textarea minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" placeholder="${title}">${esc(value)}</textarea>`;
  else input = `<input class="mock-input" type="${type === "number" || type === "integer" ? "number" : "text"}" min="${esc(metadata.min ?? "")}" max="${esc(metadata.max ?? "")}" step="${esc(metadata.step ?? "")}" minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" value="${esc(value)}" aria-label="${title}">`;
  return `<div class="mock-setting"><div class="mock-setting-copy"><b>${title}</b>${description ? `<small>${description}</small>` : ""}</div><div class="mock-control">${input}</div></div>`;
}

$("#manifest-editor").value = state.manifest;
$("#builder-tools").hidden = false;
setBuilderTab("mod");
readManifestIntoBuilder();
renderBadgeBuilder();
$("#manifest-editor").addEventListener("input", updatePreview);
$("#copy-json")?.addEventListener("click", async () => {
  const text = state.editor === "mod" ? state.manifest : state.extended;
  try { await navigator.clipboard.writeText(text); $("#copy-json").textContent = locale === "de" ? "Kopiert ✓" : "Copied ✓"; setTimeout(() => $("#copy-json").textContent = locale === "de" ? "JSON kopieren" : "Copy JSON", 1400); }
  catch { $("#manifest-editor").focus(); $("#manifest-editor").select(); document.execCommand("copy"); }
});
$("#download-json")?.addEventListener("click", () => {
  const mode = state.editor;
  const text = mode === "mod" ? state.manifest : state.extended;
  const file = new Blob([text], { type: "application/json" });
  const href = URL.createObjectURL(file);
  const link = document.createElement("a"); link.href = href; link.download = mode === "mod" ? "mod.json" : "extended.mod.json"; link.click(); URL.revokeObjectURL(href);
});
$("#reset-example").addEventListener("click", () => {
  state.manifest = JSON.stringify(exampleManifest, null, 2);
  state.extended = JSON.stringify(exampleExtended, null, 2);
  $("#manifest-editor").value = state.editor === "mod" ? state.manifest : state.extended;
  updatePreview();
});
$("#api-search").addEventListener("input", event => renderApi(event.target.value));
$("#type-search").addEventListener("input", event => renderTypes(event.target.value));
$("#resource-search").addEventListener("input", event => renderResources(event.target.value));
$("#mod-preview").addEventListener("input", event => {
  if (event.target.matches('input[type="range"]')) event.target.nextElementSibling.value = event.target.value;
});
updatePreview();

let loadPromise;
async function loadCatalog() {
  if (state.profile) { renderApi($("#api-search")?.value || ""); return; }
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    try {
      const current = await fetch(new URL("data/current.json", siteRoot)).then(checkJson);
      const dataBase = new URL(`data/${current.snapshot}/`, siteRoot);
      const [profile, api, types, resources] = await Promise.all([
        fetch(new URL("profile.json", dataBase)).then(checkJson), fetch(new URL("data/api.json", siteRoot)).then(checkJson),
        fetch(new URL("types.json", dataBase)).then(checkJson), fetch(new URL("resources.json", dataBase)).then(checkJson)
      ]);
      Object.assign(state, { profile, api: api.symbols || [], types, resources });
      $("#catalog-version").textContent = `${locale === "de" ? "API" : "API"} ${api.version} · ${profile.game_version.split("|")[0]}`;
      $("#build-state").textContent = locale === "de" ? "Spieldaten geladen" : "Game catalog loaded";
      $("#build-state").parentElement.classList.add("loaded");
      renderApi($("#api-search").value); renderTypes($("#type-search").value); renderResources($("#resource-search").value);
    } catch (error) {
      $("#build-state").textContent = copy.editor.loadingError;
      $("#build-state").parentElement.classList.add("failed");
      console.error(error);
    }
  })();
  return loadPromise;
}
async function checkJson(response) { if (!response.ok) throw new Error(`${response.status} ${response.url}`); return response.json(); }
function setCatalog(name) {
  state.catalog = name;
  $$("[data-catalog]").forEach(button => button.classList.toggle("active", button.dataset.catalog === name));
  $$(".catalog-section").forEach(section => section.classList.toggle("hidden", section.id !== `catalog-${name}`));
}
function renderApi(query = "") {
  if (!$("#api-results")) return;
  const needle = query.trim().toLocaleLowerCase();
  const matches = state.api.filter(item => {
    const isEml = item.source?.includes("eml/v1");
    const isSf = item.source?.includes("shroudforge/v1");
    const selected = state.apiSource === "all" || (state.apiSource === "eml" ? isEml : isSf);
    return selected && `${item.name} ${item.signature} ${item.description} ${item.source}`.toLocaleLowerCase().includes(needle);
  }).slice(0, 160);
  $("#api-results").innerHTML = matches.length ? matches.map(item => `<details class="api-result"><summary><code>${esc(item.name)}</code><span>${esc(item.kind === "field" ? (locale === "de" ? "ANGABE" : "VALUE") : (locale === "de" ? "FUNKTION" : "FUNCTION"))}</span><i>⌄</i></summary><div class="api-detail"><pre><code>${esc(item.signature)}</code></pre>${item.description ? `<p>${esc(item.description)}</p>` : ""}${item.params?.length ? `<h4>${esc(copy.editor.params)}</h4><div class="api-mini-table">${item.params.map(param => `<div><code>${esc(param.name)}</code><span>${esc(param.type)}</span><small>${esc(param.description)}</small></div>`).join("")}</div>` : ""}${item.returns?.length ? `<p>${esc(copy.editor.returns)}: <code>${esc(item.returns.join(", "))}</code></p>` : ""}<small>${esc(copy.editor.source)}: <code>${esc(item.source)}</code></small></div></details>`).join("") : `<div class="empty-results">${esc(copy.editor.noSymbols)}</div>`;
}
function renderTypes(query = "") {
  if (!$("#type-results")) return;
  const needle = query.trim().toLowerCase();
  const matches = Object.values(state.types).filter(type => !needle || type.qualified_name.toLowerCase().includes(needle) || Object.values(type.fields || {}).some(field => `${field.name} ${field.type_name}`.toLowerCase().includes(needle))).slice(0, 100);
  $("#type-results").innerHTML = matches.length ? matches.map(type => `<details class="api-result"><summary><code>${esc(type.qualified_name)}</code><span>${fmt(type.field_count)} ${esc(copy.editor.fields)}</span><i>⌄</i></summary><div class="api-detail"><div class="type-meta">${locale === "de" ? "Größe" : "Size"}: ${fmt(type.size)} bytes · ${locale === "de" ? "Ausrichtung" : "Alignment"}: ${fmt(type.alignment)}</div>${Object.values(type.fields || {}).slice(0, 120).map(field => `<div class="type-field"><code>${esc(field.name)}</code><span>${esc(field.type_name)}</span><small>+${fmt(field.data_offset)}</small></div>`).join("")}</div></details>`).join("") : `<div class="empty-results">${esc(copy.editor.noSymbols)}</div>`;
}
function renderResources(query = "") {
  if (!$("#resource-results")) return;
  const needle = query.trim().toLowerCase();
  const matches = Object.entries(state.resources).filter(([name, values]) => !needle || name.toLowerCase().includes(needle) || values.some(value => value.guid.toLowerCase().includes(needle))).slice(0, 100);
  $("#resource-results").innerHTML = matches.length ? matches.map(([name, values]) => `<details class="api-result"><summary><code>${esc(name)}</code><span>${fmt(values.length)} ${locale === "de" ? "Einträge" : "entries"}</span><i>⌄</i></summary><div class="api-detail">${values.slice(0, 100).map(value => `<div class="type-field"><code>${esc(value.guid)}</code><span>${esc(value.part)}</span></div>`).join("")}</div></details>`).join("") : `<div class="empty-results">${esc(copy.editor.noSymbols)}</div>`;
}

