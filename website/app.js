import de from "./content.de.js";
import en from "./content.en.js";
import docPages from "./pages/index.js";

const locale = document.documentElement.dataset.locale === "de" ? "de" : "en";
const copy = locale === "de" ? de : en;
const siteRoot = new URL("./", import.meta.url);
const staticPageIds = ["home", "play", "server", "server-files", "server-windows", "server-docker", "workings", "runtime", "first", "manifests", "community", "api"];
const docRoutes = docPages.map(page => ({ ...page, route: `doc-${page.id}` }));
const docPageByRoute = new Map(docRoutes.map(page => [page.route, page]));
const docPageIds = docRoutes.map(page => page.route);
const apiGuides = docRoutes.filter(page => page.apiSymbols?.length || page.apiPrefixes?.length);
const pageIds = [...staticPageIds, ...docPageIds];
const docNavGroups = groupDocsForNavigation(docRoutes);
const state = { page: "home", catalog: "api", apiSource: "all", api: [], apiVersion: null, apiLoaded: false, profile: null, snapshot: null, types: {}, typesLoaded: false, resources: {}, resourcesLoaded: false, manifest: null, extended: null, editor: "mod" };
let apiLoadPromise;
let gameProfilePromise;
const gameDataPromises = {};
let uploadedIcon = null;
let editingSettingKey = null;
const esc = value => String(value ?? "").replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
const fmt = value => Number(value || 0).toLocaleString(locale === "de" ? "de-DE" : "en-US");
const apiText = value => esc(value)
  .replace(/;\s*/g, ", ")
  .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
  .replace(/`([^`]+)`/g, "<code>$1</code>")
  .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");

document.documentElement.lang = locale;
document.title = locale === "de" ? "ShroudForge — Spiel-, Server- und Modding-Guide" : "ShroudForge — Player, server and modding guide";
document.querySelector('meta[name="description"]')?.setAttribute("content", locale === "de"
  ? "ShroudForge für PC und Server einrichten, Mods installieren und verstehen, wie Loader, Lua Runtime und KFC Runtime arbeiten."
  : "Set up ShroudForge for PC and servers, install mods, and learn how the loader, Lua runtime, and KFC Runtime work.");
localStorage.setItem("sf-language", locale);

function apiGuideFor(item) {
  const exact = apiGuides.find(page => page.apiSymbols?.includes(item.name));
  if (exact) return exact;
  return apiGuides.flatMap(page => (page.apiPrefixes || [])
    .filter(prefix => item.name.startsWith(prefix))
    .map(prefix => ({ page, length: prefix.length })))
    .sort((left, right) => right.length - left.length)[0]?.page;
}

function groupDocsForNavigation(pages) {
  const groups = new Map();
  for (const page of pages) {
    const label = page.navGroup?.[locale] || (locale === "de" ? "Modding-Referenz" : "Modding reference");
    const order = page.navOrder ?? 1000;
    const key = `${order}:${label}`;
    if (!groups.has(key)) groups.set(key, { label, order, items: [] });
    groups.get(key).items.push({ id: page.route, label: page.title[locale].replaceAll("`", ""), order: page.order ?? 1000 });
  }
  return [...groups.values()].sort((left, right) => left.order - right.order || left.label.localeCompare(right.label, locale))
    .map(group => ({ ...group, items: group.items.sort((left, right) => left.order - right.order || left.label.localeCompare(right.label, locale)) }));
}

function apiDescription(item, guide) {
  const localized = locale === "de" ? item.description_de : "";
  const fallback = locale === "de"
    ? item.description || item.owner_description || guide?.summary?.de || guide?.summary?.en
    : item.description || item.owner_description || guide?.summary?.en;
  return localized || fallback || (locale === "de"
    ? "Für diesen Eintrag gibt es noch keine ausführliche Erklärung. Signatur und Rückgabewerte stehen unten."
    : "A detailed explanation is not available for this entry yet. Its signature and return values are shown below.");
}

const docNavigation = docNavGroups.map(group => {
  const label = group.label.split(" · ").at(-1).split(", ").at(-1);
  return `<section class="nav-doc-group"><div class="nav-doc-label">${esc(label)}</div>${group.items.map(item => `<button class="nav-item nav-child" data-view="${item.id}"><span>${esc(item.label)}</span></button>`).join("")}</section>`;
}).join("");
const pageMarkup = pageIds.map(id => docPageByRoute.has(id)
  ? `<section id="view-${id}" class="view doc-view"><article class="markdown-page" data-doc-page="${id}"><div class="doc-loading">${locale === "de" ? "Artikel wird geladen …" : "Loading article …"}</div></article></section>`
  : `<section id="view-${id}" class="view ${id === "home" ? "active" : ""}">${copy[id]}</section>`).join("");

document.querySelector("#app").innerHTML = `
  <header class="topbar"><a href="#home" class="brand" data-view="home" aria-label="ShroudForge home"><img src="../media/shroudforge-mark.svg" alt=""><span><b>SHROUDFORGE</b><small>${copy.brand}</small></span></a><div class="top-actions"><a class="top-link" href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><a class="top-link release-link" href="https://github.com/bonsaibauer/shroudforge/releases/latest" target="_blank" rel="noreferrer">${copy.download} ↗</a><nav class="language-switch" aria-label="${locale === "de" ? "Sprache wählen" : "Choose language"}"><a data-language="de" href="../de/#${location.hash.slice(1) || "home"}" lang="de" title="Deutsch" aria-label="Deutsch" ${locale === "de" ? 'aria-current="page"' : ""}><img src="../media/flag-de.svg" alt=""></a><a data-language="en" href="../en/#${location.hash.slice(1) || "home"}" lang="en" title="English" aria-label="English" ${locale === "en" ? 'aria-current="page"' : ""}><img src="../media/flag-en.svg" alt=""></a></nav><button class="mobile-menu" id="mobile-menu" aria-label="${copy.menu}">☰</button></div></header>
  <aside class="sidebar"><div class="sidebar-label">${copy.menu}</div><nav>${copy.navGroups.map(group => `<div class="nav-group"><div class="nav-group-title">${group.label}</div>${group.items.map(item => `<button class="nav-item ${item.child ? "nav-child" : ""} ${item.id === "home" ? "active" : ""}" data-view="${item.id}"><span>${item.label}</span>${item.id === "home" ? '<i class="nav-glow"></i>' : ""}</button>${group.items.some(entry => entry.id === "manifests") && item.id === "api" ? docNavigation : ""}`).join("")}</div>`).join("")}</nav></aside>
  <main class="main-content"><div class="content-wrap">${pageMarkup}<footer class="site-footer"><span>© SHROUDFORGE · ${copy.footer}</span><span><a href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><b>·</b><a href="#api" data-view="api">${copy.apiLabel}</a></span></footer></div></main>`;

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
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
function setPage(page, updateHash = true) {
  if (!pageIds.includes(page)) page = "home";
  state.page = page;
  $$(".view").forEach(view => view.classList.toggle("active", view.id === `view-${page}`));
  $$(".nav-item").forEach(item => item.classList.toggle("active", item.dataset.view === page));
  if (updateHash) history.replaceState(null, "", `#${page}`);
  $(".sidebar").classList.remove("open");
  window.scrollTo({ top: 0, behavior: "smooth" });
  if (page === "api") loadCatalog();
  const docPage = docPageByRoute.get(page);
  if (docPage) loadMarkdownPage(docPage);
}

async function loadMarkdownPage(page) {
  const article = document.querySelector(`[data-doc-page="${page.route}"]`);
  if (!article || article.dataset.locale === locale) return;
  article.innerHTML = `<div class="doc-loading">${locale === "de" ? "Artikel wird geladen …" : "Loading article …"}</div>`;
  try {
    const response = await fetch(new URL(`${page.path}${locale}.md`, import.meta.url));
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const markdown = await response.text();
    article.innerHTML = renderMarkdown(markdown, page);
    article.dataset.locale = locale;
    addCopyControls();
  } catch (error) {
    article.innerHTML = `<div class="callout error"><b>${locale === "de" ? "Artikel konnte nicht geladen werden" : "Could not load article"}</b><span>${esc(error.message)}</span></div>`;
  }
}

function renderMarkdown(markdown, page) {
  const lines = markdown.replaceAll("\r", "").split("\n");
  const blocks = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) { index++; continue; }
    const fence = line.match(/^\s*```([\w-]*)\s*$/);
    if (fence) {
      const codeLines = [];
      index++;
      while (index < lines.length && !/^\s*```\s*$/.test(lines[index])) codeLines.push(lines[index++]);
      index++;
      blocks.push(`<pre><code${fence[1] ? ` class="language-${esc(fence[1])}"` : ""}>${esc(codeLines.join("\n"))}</code></pre>`);
      continue;
    }
    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      const level = heading[1].length;
      blocks.push(`<h${level}>${renderMarkdownInline(heading[2], page)}</h${level}>`);
      index++;
      continue;
    }
    if (/^\s*(?:---+|\*\*\*+)\s*$/.test(line)) { blocks.push("<hr>"); index++; continue; }
    if (line.startsWith("> ")) {
      const quote = [];
      while (index < lines.length && lines[index].startsWith("> ")) quote.push(lines[index++].slice(2));
      blocks.push(`<blockquote><p>${renderMarkdownInline(quote.join(" "), page)}</p></blockquote>`);
      continue;
    }
    if (line.includes("|") && index + 1 < lines.length && /^\s*\|?\s*:?-{3,}/.test(lines[index + 1])) {
      const rows = [line, lines[index + 1]];
      index += 2;
      while (index < lines.length && lines[index].includes("|")) rows.push(lines[index++]);
      const cells = row => row.trim().replace(/^\||\|$/g, "").split("|").map(cell => cell.trim());
      const header = cells(rows[0]);
      const tableRows = rows.slice(2).map(cells);
      blocks.push(`<div class="doc-table-wrap"><table class="doc-table"><thead><tr>${header.map(cell => `<th>${renderMarkdownInline(cell, page)}</th>`).join("")}</tr></thead><tbody>${tableRows.map(row => `<tr>${row.map(cell => `<td>${renderMarkdownInline(cell, page)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`);
      continue;
    }
    if (/^\s*[-*+]\s+/.test(line)) {
      const items = [];
      while (index < lines.length && /^\s*[-*+]\s+/.test(lines[index])) items.push(lines[index++].replace(/^\s*[-*+]\s+/, ""));
      blocks.push(`<ul>${items.map(item => `<li>${renderMarkdownInline(item, page)}</li>`).join("")}</ul>`);
      continue;
    }
    if (/^\s*\d+\.\s+/.test(line)) {
      const items = [];
      while (index < lines.length && /^\s*\d+\.\s+/.test(lines[index])) items.push(lines[index++].replace(/^\s*\d+\.\s+/, ""));
      blocks.push(`<ol>${items.map(item => `<li>${renderMarkdownInline(item, page)}</li>`).join("")}</ol>`);
      continue;
    }
    const paragraph = [line.trim()];
    index++;
    while (index < lines.length && lines[index].trim() && !/^(#{1,4})\s+/.test(lines[index]) && !/^\s*```/.test(lines[index]) && !/^\s*[-*+]\s+/.test(lines[index]) && !/^\s*\d+\.\s+/.test(lines[index]) && !lines[index].startsWith("> ")) paragraph.push(lines[index++].trim());
    blocks.push(`<p>${renderMarkdownInline(paragraph.join("\n"), page)}</p>`);
  }
  return blocks.join("\n");
}

function renderMarkdownInline(value, page) {
  const codeParts = [];
  let html = esc(value).replace(/`([^`]+)`/g, (_, content) => {
    const token = `\u0000CODE${codeParts.length}\u0000`;
    codeParts.push(`<code>${content}</code>`);
    return token;
  });
  html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_, alt, href) => `<img src="${esc(resolveMarkdownUrl(href, page))}" alt="${alt}">`);
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, href) => {
    if (href.startsWith("#")) {
      const target = href.slice(1);
      return `<a href="${esc(href)}"${pageIds.includes(target) ? ` data-view="${esc(target)}"` : ""}>${label}</a>`;
    }
    const external = /^https?:\/\//i.test(href);
    return `<a href="${esc(resolveMarkdownUrl(href, page))}"${external ? ' target="_blank" rel="noreferrer"' : ""}>${label}</a>`;
  });
  html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>").replace(/\*([^*]+)\*/g, "<em>$1</em>");
  return html.replace(/\u0000CODE(\d+)\u0000/g, (_, number) => codeParts[Number(number)]);
}

function resolveMarkdownUrl(href, page) {
  try { return new URL(href, new URL(page.path, import.meta.url)).href; }
  catch { return href; }
}

document.addEventListener("click", event => {
  const editSetting = event.target.closest("[data-edit-setting]");
  if (editSetting) {
    const data = parseCurrent() || {}, key = editSetting.dataset.editSetting, raw = data.settings?.[key];
    const item = raw && typeof raw === "object" && !Array.isArray(raw) ? raw : { value: raw };
    editingSettingKey = key;
    $("#setting-key").value = key;
    $("#setting-label").value = item.label || key;
    $("#setting-description").value = item.description || "";
    $("#setting-control").value = item.control || (item.options ? "select" : typeof item.value === "boolean" ? "toggle" : typeof item.value === "number" ? "number" : "text");
    $("#setting-value").value = Array.isArray(item.value) ? item.value.join(", ") : String(item.value ?? "");
    $("#setting-options").value = Object.entries(item.options || {}).map(([value, label]) => `${value} = ${label}`).join("\n");
    $("#setting-group").value = data.groups?.find(group => group.settings?.includes(key))?.label || "";
    for (const field of ["min", "max", "step", "minLength", "maxLength"]) $(`#setting-${field.replace("Length", "-length").toLowerCase()}`).value = item[field] ?? "";
    $("#add-setting").textContent = locale === "de" ? "Einstellung speichern" : "Save setting";
    $("#setting-key").scrollIntoView({ behavior: "smooth", block: "center" });
  }
  const removeSetting = event.target.closest("[data-remove-setting]");
  if (removeSetting) updateExtended(extended => {
    delete extended.settings?.[removeSetting.dataset.removeSetting];
    for (const group of extended.groups || []) group.settings = (group.settings || []).filter(key => key !== removeSetting.dataset.removeSetting);
  });
  const removeGroup = event.target.closest("[data-remove-group]");
  if (removeGroup) updateExtended(extended => { extended.groups?.splice(Number(removeGroup.dataset.removeGroup), 1); });
  const removeAction = event.target.closest("[data-remove-action]");
  if (removeAction) {
    const [groupIndex, actionIndex] = removeAction.dataset.removeAction.split(":").map(Number);
    updateExtended(extended => { extended.groups?.[groupIndex]?.actions?.splice(actionIndex, 1); });
  }
  const removeChangelog = event.target.closest("[data-remove-changelog]");
  if (removeChangelog) updateExtended(extended => { extended.changelog?.splice(Number(removeChangelog.dataset.removeChangelog), 1); });
  const removeDependency = event.target.closest("[data-remove-dependency]");
  if (removeDependency) {
    const index = Number(removeDependency.dataset.removeDependency);
    let mod; try { mod = JSON.parse(state.manifest); } catch { return; }
    mod.dependencies?.splice(index, 1);
    state.manifest = JSON.stringify(mod, null, 2); $("#manifest-editor").value = state.manifest;
    readManifestIntoBuilder(); updatePreview();
  }
  const viewButton = event.target.closest("[data-view]");
  if (viewButton) { event.preventDefault(); setPage(viewButton.dataset.view); }
  const catalogButton = event.target.closest("[data-catalog]");
  if (catalogButton) setCatalog(catalogButton.dataset.catalog);
  const apiQueryButton = event.target.closest("[data-api-query]");
  if (apiQueryButton) {
    const search = $("#api-search");
    search.value = apiQueryButton.dataset.apiQuery;
    state.apiSource = "all";
    $$(`[data-api-source]`).forEach(button => button.classList.toggle("active", button.dataset.apiSource === "all"));
    renderApi(search.value);
    search.focus();
  }
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
document.querySelectorAll("[data-language]").forEach(link => link.addEventListener("click", () => localStorage.setItem("sf-language", link.dataset.language)));
window.addEventListener("hashchange", () => setPage(location.hash.slice(1), false));
setPage(pageIds.includes(location.hash.slice(1)) ? location.hash.slice(1) : "home", false);

const exampleManifest = {
  $schema: "https://bonsaibauer.github.io/shroudforge/schemas/manifest.schema.json",
  id: "example.hello-ember",
  name: "Hello Ember",
  version: "1.0.0",
  description: "A small ShroudForge starter mod that demonstrates saved settings, a Modloader action, and runtime logging.",
  authors: ["Your Name"],
  license: null,
  icon: null,
  dependencies: [],
  capabilities: ["runtime"]
};
const exampleExtended = {
  $schema: "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  schemaVersion: 1,
  enabled: false,
  launcher: "SF",
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
rebuildManifestStudio();

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
  if (manifest.dependencies !== undefined) {
    if (!Array.isArray(manifest.dependencies)) fail("mod.json: dependencies must be a list.");
    else {
      const dependencyIds = new Set();
      for (const item of manifest.dependencies) {
        if (!item || typeof item !== "object" || Array.isArray(item)) { fail("mod.json: each dependency must be an object with an id and version."); continue; }
        if (Object.keys(item).some(key => !["id", "version", "optional"].includes(key))) fail("mod.json: a dependency only allows id, version, and optional.");
        if (typeof item.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(item.id)) fail("mod.json: dependency id must use 1–80 letters, numbers, dots, hyphens, or underscores.");
        else {
          if (item.id === manifest.id) fail("mod.json: a mod cannot depend on itself.");
          if (dependencyIds.has(item.id)) fail(`mod.json: dependency “${item.id}” is listed more than once.`);
          dependencyIds.add(item.id);
        }
        if (typeof item.version !== "string" || !item.version.trim()) fail("mod.json: every dependency needs a non-empty SemVer version requirement.");
        if (item.optional !== undefined && typeof item.optional !== "boolean") fail("mod.json: dependency optional must be true or false.");
      }
    }
  }
  checkKeys(extended, ["$schema", "schemaVersion", "enabled", "targets", "launcher", "links", "changelog", "settings", "groups"], "extended.mod.json");
  if (extended.schemaVersion !== 1) fail("extended.mod.json: schemaVersion must be 1.");
  if (extended["$schema"] !== undefined && typeof extended["$schema"] !== "string") fail("extended.mod.json: $schema must be a text address.");
  if (typeof extended.enabled !== "boolean") fail("extended.mod.json: enabled must be true or false.");
  if (extended.targets !== undefined && (!Array.isArray(extended.targets) || extended.targets.length < 1 || extended.targets.some(target => !["client", "server"].includes(target)) || new Set(extended.targets).size !== extended.targets.length)) fail("extended.mod.json: targets must contain client, server, or both once each.");
  if (extended.launcher !== undefined && !["EML", "SF"].includes(extended.launcher)) fail("extended.mod.json: launcher must be EML or SF.");
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
      ["settings.<key>.control", "string", de ? "Steuerelement im Modloader." : "Control displayed in Modloader.", "toggle · checkbox · text · textarea · number · slider · select · radio · segmented · multiselect · keybind · color"],
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
function syncEditorFromExtended() {
  state.extended = JSON.stringify(parseCurrent(), null, 2);
  $("#extended-editor").value = state.extended;
  updatePreview();
  renderBadgeBuilder();
  renderExtendedItems();
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
  uploadedIcon = file ? { file, dataUrl, name: file.name } : null;
  const img = $("#icon-preview-image");
  const empty = $("#icon-preview-empty");
  if (img) { img.src = dataUrl || ""; img.hidden = !dataUrl; }
  if (empty) empty.hidden = Boolean(dataUrl);
  const download = $("#download-icon");
  if (download) download.disabled = !file;
}
$("#mod-icon-file")?.addEventListener("change", event => {
  const file = event.currentTarget.files?.[0];
  if (!file) return;
  const extension = file.name.split(".").pop()?.toLowerCase();
  if (!["png", "jpg", "jpeg", "webp", "svg"].includes(extension)) {
    alert(locale === "de" ? "Erlaubt sind PNG, JPG/JPEG, WebP und SVG." : "Supported formats are PNG, JPG/JPEG, WebP, and SVG.");
    event.currentTarget.value = "";
    return;
  }
  if (file.size > 2 * 1024 * 1024) {
    alert(locale === "de" ? "Das Icon darf höchstens 2 MiB groß sein." : "The icon must be 2 MiB or smaller.");
    event.currentTarget.value = "";
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    const safeName = `icon.${extension}`;
    const renamed = new File([file], safeName, { type: file.type, lastModified: file.lastModified });
    showUploadedIcon(renamed, String(reader.result));
    $("#mod-icon").value = safeName;
    $("#apply-mod-info").click();
  };
  reader.readAsDataURL(file);
});
$("#download-icon")?.addEventListener("click", () => {
  if (!uploadedIcon) return;
  const href = URL.createObjectURL(uploadedIcon.file);
  const link = document.createElement("a"); link.href = href; link.download = uploadedIcon.file.name; link.click();
  setTimeout(() => URL.revokeObjectURL(href), 1000);
});
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function write16(view, offset, value) { view.setUint16(offset, value, true); }
function write32(view, offset, value) { view.setUint32(offset, value, true); }
async function makeModZip() {
  const encoder = new TextEncoder();
  const entries = [
    { name: "mod.json", data: encoder.encode(state.manifest) },
    { name: "extended.mod.json", data: encoder.encode(state.extended) }
  ];
  if (uploadedIcon) entries.push({ name: uploadedIcon.file.name, data: new Uint8Array(await uploadedIcon.file.arrayBuffer()) });
  const localParts = [], centralParts = [];
  let localOffset = 0;
  for (const entry of entries) {
    const name = encoder.encode(entry.name), crc = crc32(entry.data);
    const local = new Uint8Array(30 + name.length + entry.data.length), localView = new DataView(local.buffer);
    write32(localView, 0, 0x04034b50); write16(localView, 4, 20); write16(localView, 6, 0x0800); write16(localView, 8, 0); write16(localView, 12, 0x21);
    write32(localView, 14, crc); write32(localView, 18, entry.data.length); write32(localView, 22, entry.data.length); write16(localView, 26, name.length);
    local.set(name, 30); local.set(entry.data, 30 + name.length); localParts.push(local);
    const central = new Uint8Array(46 + name.length), centralView = new DataView(central.buffer);
    write32(centralView, 0, 0x02014b50); write16(centralView, 4, 20); write16(centralView, 6, 20); write16(centralView, 8, 0x0800); write16(centralView, 10, 0); write16(centralView, 14, 0x21);
    write32(centralView, 16, crc); write32(centralView, 20, entry.data.length); write32(centralView, 24, entry.data.length); write16(centralView, 28, name.length); write32(centralView, 42, localOffset);
    central.set(name, 46); centralParts.push(central); localOffset += local.length;
  }
  const centralSize = centralParts.reduce((sum, part) => sum + part.length, 0);
  const end = new Uint8Array(22), endView = new DataView(end.buffer);
  write32(endView, 0, 0x06054b50); write16(endView, 8, entries.length); write16(endView, 10, entries.length); write32(endView, 12, centralSize); write32(endView, 16, localOffset);
  return new Blob([...localParts, ...centralParts, end], { type: "application/zip" });
}
$("#download-package")?.addEventListener("click", async event => {
  const button = event.currentTarget;
  button.disabled = true;
  try {
    const manifest = JSON.parse(state.manifest), extended = JSON.parse(state.extended);
    const errors = validateManifests(manifest, extended);
    if (errors.length) { alert(errors[0]); return; }
    if (manifest.icon && (!uploadedIcon || manifest.icon !== uploadedIcon.file.name)) {
      alert(locale === "de" ? "Das eingetragene Icon ist nicht im Paket. Lade die Bilddatei hoch oder entferne den Icon-Pfad." : "The referenced icon is not included in the package. Upload that image or remove the icon path.");
      return;
    }
    const file = await makeModZip(), href = URL.createObjectURL(file), link = document.createElement("a");
    const base = (manifest.id || "mod").replace(/[^A-Za-z0-9._-]/g, "_");
    link.href = href; link.download = `${base}.zip`; link.click(); setTimeout(() => URL.revokeObjectURL(href), 1500);
  } catch { alert(locale === "de" ? "Bitte korrigiere zuerst beide JSON-Dateien." : "Please fix both JSON files first."); }
  finally { button.disabled = false; }
});
$("#add-dependency")?.addEventListener("click", () => {
  const id = $("#dependency-id").value.trim();
  const version = $("#dependency-version").value.trim();
  if (!/^[A-Za-z0-9._-]{1,80}$/.test(id) || !version) { alert(locale === "de" ? "Bitte trage die gültige Mod-Kennung aus deren mod.json und einen SemVer-Versionsbereich ein." : "Enter the dependency ID from its mod.json and a SemVer version requirement."); return; }
  let mod; try { mod = JSON.parse(state.manifest); } catch { return; }
  if (id === mod.id) { alert(locale === "de" ? "Ein Mod kann nicht von sich selbst abhängen." : "A mod cannot depend on itself."); return; }
  mod.dependencies ||= [];
  if (mod.dependencies.some(item => item.id === id)) { alert(locale === "de" ? "Diese Mod ist bereits als Abhängigkeit eingetragen." : "This mod is already listed as a dependency."); return; }
  const dependency = { id, version };
  if ($("#dependency-optional").checked) dependency.optional = true;
  mod.dependencies.push(dependency);
  state.manifest = JSON.stringify(mod, null, 2);
  $("#manifest-editor").value = state.manifest;
  $("#dependency-id").value = ""; $("#dependency-version").value = ""; $("#dependency-optional").checked = false;
  readManifestIntoBuilder();
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
  $("#manifest-editor").value = state.manifest;
  updatePreview();
});
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
  if (editingSettingKey && editingSettingKey !== key) {
    delete extended.settings[editingSettingKey];
    for (const group of extended.groups || []) group.settings = (group.settings || []).filter(value => value !== editingSettingKey);
  }
  extended.settings[key] = setting;
  if (!Array.isArray(extended.groups)) extended.groups = [];
  if (!extended.groups.length) extended.groups.push({ label: locale === "de" ? "Einstellungen" : "Settings", settings: [] });
  const groupName = $("#setting-group").value.trim();
  const targetGroup = groupName ? (extended.groups.find(group => group.label === groupName) || (extended.groups.push({ label: groupName, settings: [] }), extended.groups.at(-1))) : extended.groups[0];
  targetGroup.settings ||= [];
  if (!targetGroup.settings.includes(key)) targetGroup.settings.push(key);
  state.extended = JSON.stringify(extended, null, 2);
  $("#extended-editor").value = state.extended;
  editingSettingKey = null;
  $("#add-setting").textContent = locale === "de" ? "Einstellung hinzufügen +" : "Add setting +";
  $("#setting-key").value = "newSetting";
  $("#setting-label").value = locale === "de" ? "Neue Einstellung" : "New setting";
  $("#setting-value").value = "";
  $("#setting-description").value = "";
  $("#setting-options").value = "";
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
    $("#extended-editor").value = state.extended;
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
  const icon = uploadedIcon?.dataUrl || "";
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
  else if (control === "keybind") input = `<input class="mock-input mock-keybind" value="${esc(value || "")}" readonly aria-label="${title}">`;
  else if (control === "textarea") input = `<textarea minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" placeholder="${title}">${esc(value)}</textarea>`;
  else input = `<input class="mock-input" type="${type === "number" || type === "integer" ? "number" : "text"}" min="${esc(metadata.min ?? "")}" max="${esc(metadata.max ?? "")}" step="${esc(metadata.step ?? "")}" minlength="${esc(metadata.minLength ?? "")}" maxlength="${esc(metadata.maxLength ?? "")}" value="${esc(value)}" aria-label="${title}">`;
  return `<div class="mock-setting"><div class="mock-setting-copy"><b>${title}</b>${description ? `<small>${description}</small>` : ""}</div><div class="mock-control">${input}</div></div>`;
}

$("#manifest-editor").value = state.manifest;
$("#extended-editor").value = state.extended;
setBuilderTab("mod");
readManifestIntoBuilder();
renderBadgeBuilder();
$("#manifest-editor").addEventListener("input", () => { state.manifest = $("#manifest-editor").value; try { readManifestIntoBuilder(); const mod = JSON.parse(state.manifest); if (uploadedIcon && mod.icon !== uploadedIcon.file.name) { $("#mod-icon-file").value = ""; showUploadedIcon(null, ""); } } catch {} updatePreview(); });
$("#mod-builder").addEventListener("input", event => { if (event.target.matches("#mod-icon") && uploadedIcon && event.target.value !== uploadedIcon.file.name) { $("#mod-icon-file").value = ""; showUploadedIcon(null, ""); } if (event.target.matches("#mod-id, #mod-name, #mod-version, #mod-authors, #mod-description, #mod-license, #mod-icon, [data-capability]")) $("#apply-mod-info").click(); });
$("#mod-builder").addEventListener("change", event => { if (event.target.matches("[data-capability]")) $("#apply-mod-info").click(); });
$("#extended-enabled").addEventListener("change", () => $("#apply-extended-core").click());
$("#target-client")?.addEventListener("change", () => updateExtended(extended => {
  const targets = [$("#target-client").checked ? "client" : null, $("#target-server").checked ? "server" : null].filter(Boolean);
  if (targets.length) extended.targets = targets;
  else { extended.targets = ["client"]; syncTargetPicker(extended); }
}));
$("#target-server")?.addEventListener("change", () => updateExtended(extended => {
  const targets = [$("#target-client").checked ? "client" : null, $("#target-server").checked ? "server" : null].filter(Boolean);
  if (targets.length) extended.targets = targets;
  else { extended.targets = ["client"]; syncTargetPicker(extended); }
}));
$("#extended-editor").addEventListener("input", () => { updatePreview(); try { $("#extended-enabled").checked = Boolean(parseCurrent()?.enabled); renderBadgeBuilder(); } catch {} });
async function copyText(text, button) {
  try { await navigator.clipboard.writeText(text); }
  catch { const area = document.createElement('textarea'); area.value = text; document.body.append(area); area.select(); document.execCommand('copy'); area.remove(); }
  const original = button.textContent; button.textContent = locale === 'de' ? 'Kopiert ✓' : 'Copied ✓';
  setTimeout(() => button.textContent = original, 1400);
}
$('#copy-mod-json')?.addEventListener('click', event => copyText(state.manifest, event.currentTarget));
$('#copy-extended-json')?.addEventListener('click', event => copyText(state.extended, event.currentTarget));
function downloadJson(text, filename) {
  const file = new Blob([text], { type: 'application/json' }); const href = URL.createObjectURL(file);
  const link = document.createElement('a'); link.href = href; link.download = filename; link.click(); URL.revokeObjectURL(href);
}
$('#download-mod-json')?.addEventListener('click', () => downloadJson(state.manifest, 'mod.json'));
$('#download-extended-json')?.addEventListener('click', () => downloadJson(state.extended, 'extended.mod.json'));
$('#reset-example').addEventListener('click', () => {
  state.manifest = JSON.stringify(exampleManifest, null, 2); state.extended = JSON.stringify(exampleExtended, null, 2);
  $('#manifest-editor').value = state.manifest; $('#extended-editor').value = state.extended;
  editingSettingKey = null; $('#add-setting').textContent = locale === 'de' ? 'Einstellung hinzufügen +' : 'Add setting +';
  $('#mod-icon-file').value = ''; showUploadedIcon(null, '');
  readManifestIntoBuilder(); $('#extended-enabled').checked = Boolean(exampleExtended.enabled); renderBadgeBuilder(); updatePreview();
});
$("#api-search").addEventListener("input", event => renderApi(event.target.value));
$("#type-search").addEventListener("input", event => renderTypes(event.target.value));
$("#resource-search").addEventListener("input", event => renderResources(event.target.value));
function addCopyControls() {
  $$("pre").forEach(block => {
    if (block.parentElement.classList.contains("code-block")) return;
    const wrapper = document.createElement("div"); wrapper.className = "code-block";
    const bar = document.createElement("div"); bar.className = "code-block-bar";
    const label = document.createElement("span"); label.textContent = block.dataset.label || (locale === "de" ? "CODEBEISPIEL" : "CODE EXAMPLE");
    const button = document.createElement("button"); button.type = "button"; button.className = "reset-button copy-code"; button.textContent = block.dataset.copyLabel || (locale === "de" ? "Kopieren" : "Copy");
    button.addEventListener("click", () => copyText(block.innerText, button));
    bar.append(label, button); block.before(wrapper); wrapper.append(bar, block);
  });
  $$(".folder-tree").forEach(tree => {
    if (tree.querySelector(".copy-code")) return;
    const button = document.createElement("button"); button.type = "button"; button.className = "reset-button copy-code tree-copy";
    button.textContent = locale === "de" ? "Ordnerstruktur kopieren" : "Copy folder structure";
    button.addEventListener("click", () => { const copy = tree.cloneNode(true); copy.querySelector(".tree-copy")?.remove(); copyText(copy.innerText.trim(), button); });
    tree.append(button);
  });
  $$(".command").forEach(command => {
    if (command.parentElement.classList.contains("command-copy")) return;
    const wrapper = document.createElement("span"); wrapper.className = "command-copy";
    const button = document.createElement("button"); button.type = "button"; button.className = "reset-button copy-code"; button.textContent = locale === "de" ? "Befehl kopieren" : "Copy command";
    button.addEventListener("click", () => copyText(command.innerText, button));
    command.before(wrapper); wrapper.append(command, button);
  });
}
addCopyControls();
$("#mod-preview").addEventListener("input", event => {
  if (event.target.matches('input[type="range"]')) event.target.nextElementSibling.value = event.target.value;
});
updatePreview();

async function loadCatalog() {
  if (state.apiLoaded) { renderApi($("#api-search")?.value || ""); return; }
  if (apiLoadPromise) return apiLoadPromise;
  setCatalogStatus(locale === "de" ? "Lua-API wird geladen …" : "Loading Lua API …");
  apiLoadPromise = (async () => {
    try {
      const api = await fetch(new URL("data/api.json", siteRoot)).then(checkJson);
      if (!Array.isArray(api.symbols)) throw new Error("API response does not contain a symbols list");
      state.api = api.symbols;
      state.apiVersion = api.version || null;
      state.apiLoaded = true;
      updateCatalogVersion();
      setCatalogStatus(locale === "de" ? "Lua-API geladen" : "Lua API loaded", "loaded");
      renderApi($("#api-search").value);
    } catch (error) {
      apiLoadPromise = null;
      showCatalogFailure(copy.editor.loadingError, error, loadCatalog);
      console.error(error);
    }
  })();
  return apiLoadPromise;
}
async function checkJson(response) { if (!response.ok) throw new Error(`${response.status} ${response.url}`); return response.json(); }
function setCatalogStatus(message, stateName = "") {
  const status = $("#build-state");
  if (!status) return;
  status.textContent = message;
  const container = status.parentElement;
  container.classList.remove("loaded", "failed");
  if (stateName) container.classList.add(stateName);
  container.querySelector("#catalog-retry")?.remove();
}
function showCatalogFailure(message, error, retry) {
  setCatalogStatus(`${message} ${error.message}`, "failed");
  const button = document.createElement("button");
  button.id = "catalog-retry";
  button.type = "button";
  button.className = "catalog-retry";
  button.textContent = locale === "de" ? "Erneut versuchen" : "Retry";
  button.addEventListener("click", retry, { once: true });
  $("#build-state").parentElement.append(button);
}
function updateCatalogVersion() {
  const version = [
    state.apiVersion && `API ${state.apiVersion}`,
    state.profile?.game_version && state.profile.game_version.split("|")[0],
  ].filter(Boolean);
  $("#catalog-version").textContent = version.join(" · ");
}
async function loadGameProfile() {
  if (state.profile && state.snapshot) return state.snapshot;
  if (!gameProfilePromise) {
    gameProfilePromise = (async () => {
      const current = await fetch(new URL("data/current.json", siteRoot)).then(checkJson);
      const dataBase = new URL(`data/${current.snapshot}/`, siteRoot);
      state.profile = await fetch(new URL("profile.json", dataBase)).then(checkJson);
      state.snapshot = current.snapshot;
      updateCatalogVersion();
      return state.snapshot;
    })().catch(error => {
      gameProfilePromise = null;
      throw error;
    });
  }
  return gameProfilePromise;
}
async function loadGameData(name) {
  if (!["types", "resources"].includes(name)) return;
  const loadedKey = `${name}Loaded`;
  const label = name === "types"
    ? (locale === "de" ? "Spiel-Details" : "Game details")
    : (locale === "de" ? "Spieldateien" : "Game files");
  if (state[loadedKey]) {
    setCatalogStatus(`${label} ${locale === "de" ? "geladen" : "loaded"}`, "loaded");
    return;
  }
  if (gameDataPromises[name]) return gameDataPromises[name];
  setCatalogStatus(`${label} ${locale === "de" ? "werden geladen …" : "are loading …"}`);
  gameDataPromises[name] = (async () => {
    try {
      const snapshot = await loadGameProfile();
      const dataBase = new URL(`data/${snapshot}/`, siteRoot);
      state[name] = await fetch(new URL(`${name}.json`, dataBase)).then(checkJson);
      state[loadedKey] = true;
      if (name === "types") renderTypes($("#type-search").value);
      else renderResources($("#resource-search").value);
      setCatalogStatus(`${label} ${locale === "de" ? "geladen" : "loaded"}`, "loaded");
    } catch (error) {
      gameDataPromises[name] = null;
      showCatalogFailure(`${label} ${locale === "de" ? "konnten nicht geladen werden." : "could not be loaded."}`, error, () => loadGameData(name));
      console.error(error);
    }
  })();
  return gameDataPromises[name];
}
function setCatalog(name) {
  state.catalog = name;
  $$("[data-catalog]").forEach(button => button.classList.toggle("active", button.dataset.catalog === name));
  $$(".catalog-section").forEach(section => section.classList.toggle("hidden", section.id !== `catalog-${name}`));
  if (name === "api") loadCatalog();
  else loadGameData(name);
}
function renderApi(query = "") {
  if (!$("#api-results")) return;
  const needle = query.trim().toLocaleLowerCase();
  const count = $("#api-result-count");
  const terms = needle.split(/\s+/).filter(Boolean);
  const filtered = state.api.filter(item => {
    const isEml = item.source?.includes("eml/v1");
    const isSf = item.source?.includes("shroudforge/v1");
    const selected = state.apiSource === "all" || (state.apiSource === "eml" ? isEml : isSf);
    if (!selected) return false;
    const guide = apiGuideFor(item);
    const searchText = [
      item.name, item.signature, item.description, item.description_de, item.errors, item.example, item.source,
      ...(item.params || []).flatMap(param => [param.name, param.type, param.description, param.description_de]),
      ...(item.returns || []), ...(item.returnDetails || []).flatMap(result => [result.name, result.type, result.description]),
      guide?.title?.[locale], guide?.summary?.[locale],
    ].join(" ").toLocaleLowerCase();
    return terms.every(term => searchText.includes(term));
  });
  if (count) count.textContent = locale === "de"
    ? `${fmt(filtered.length)} ${filtered.length === 1 ? "Eintrag" : "Einträge"}`
    : `${fmt(filtered.length)} ${filtered.length === 1 ? "entry" : "entries"}`;
  $("#api-results").innerHTML = filtered.length
    ? filtered.map(renderApiResult).join("")
    : `<div class="empty-results">${locale === "de" ? "Keine Treffer. Prüfe die Schreibweise, wähle Alle oder suche nach einem kürzeren Begriff." : "No results. Check the spelling, choose All, or search for a shorter term."}</div>`;
  addCopyControls();
}

function renderApiResult(item) {
  const guide = apiGuideFor(item);
  const description = apiDescription(item, guide);
  const kind = item.kind === "field"
    ? (locale === "de" ? "FELD" : "FIELD")
    : (locale === "de" ? "FUNKTION" : "FUNCTION");
  const sourceLabel = item.source?.includes("eml/v1") ? "EML v1" : "ShroudForge v1";
  const params = item.params?.length
    ? `<h4>${locale === "de" ? "Eingaben" : "Inputs"}</h4><div class="api-mini-table">${item.params.map(param => {
      const explanation = (locale === "de" ? param.description_de : "") || param.description || (locale === "de" ? "Typ und Name zeigen, was diese Funktion erwartet." : "The type and name show what this function expects.");
      return `<div><code>${esc(param.name)}</code><span>${esc(param.type)}</span><small>${apiText(explanation)}</small></div>`;
    }).join("")}</div>`
    : "";
  const returns = item.kind === "field" ? "" : item.returnDetails?.length
    ? `<h4>${locale === "de" ? "Ergebnis" : "Result"}</h4><div class="api-return-list">${item.returnDetails.map(result => `<div><code>${esc(result.type)}${result.name ? ` ${esc(result.name)}` : ""}</code>${result.description ? `<span>${apiText(result.description)}</span>` : ""}</div>`).join("")}</div>`
    : item.returns?.length ? `<h4>${locale === "de" ? "Ergebnis" : "Result"}</h4><p><code>${esc(item.returns.join(", "))}</code></p>` : "";
  const errors = item.errors
    ? `<div class="api-error-note"><strong>${locale === "de" ? "Fehler und Grenzen" : "Errors and limits"}</strong><span>${apiText(item.errors)}</span></div>`
    : "";
  const example = item.example
    ? `<h4>${locale === "de" ? "Kurzes Beispiel" : "Quick example"}</h4><pre data-label="${locale === "de" ? "LUA-BEISPIEL" : "LUA EXAMPLE"}" data-copy-label="${locale === "de" ? "Beispiel kopieren" : "Copy example"}"><code>${esc(locale === "de" ? item.example_de || item.example : item.example)}</code></pre>`
    : "";
  const guideMarkup = guide
    ? `<div class="api-related-guide"><div><strong>${locale === "de" ? "Schritt-für-Schritt-Anleitung" : "Step-by-step guide"}</strong><span>${esc(guide.title[locale])}</span></div><button type="button" class="text-link" data-view="${esc(guide.route)}">${locale === "de" ? "Anleitung öffnen" : "Open guide"} →</button></div>`
    : "";
  const sourceUrl = `https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/api/src/${item.source}`;
  const sourceMarkup = `<small>${esc(copy.editor.source)}: <a href="${esc(sourceUrl)}" target="_blank" rel="noreferrer"><code>${esc(item.source)}</code></a></small>`;
  const signatureLabel = item.kind === "field"
    ? (locale === "de" ? "API-FELD" : "API FIELD")
    : (locale === "de" ? "API-SIGNATUR" : "API SIGNATURE");
  const copySignatureLabel = item.kind === "field"
    ? (locale === "de" ? "Feld kopieren" : "Copy field")
    : (locale === "de" ? "Signatur kopieren" : "Copy signature");
  return `<details class="api-result"><summary><div class="api-result-heading"><code>${esc(item.name)}</code><span class="api-source-badge">${esc(sourceLabel)}</span><span class="api-kind-badge">${esc(kind)}</span><i>⌄</i></div><span class="api-summary-text">${apiText(description)}</span></summary><div class="api-detail"><h4>${locale === "de" ? "Was macht das?" : "What does it do?"}</h4><p>${apiText(description)}</p><pre data-label="${signatureLabel}" data-copy-label="${copySignatureLabel}"><code>${esc(item.signature)}</code></pre>${params}${returns}${errors}${example}${guideMarkup}${sourceMarkup}</div></details>`;
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

