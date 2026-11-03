import de from "./content/de.js";
import en from "./content/en.js";
import docPages from "./pages/index.js";
import siteViews from "./pages/views.js";
import { createNavigation } from "./page-navigation.js";
import { createMarkdown } from "./markdown-rendering.js";
import { createReferenceSearch } from "./reference-search.js";
import { createPageIntegrationLoader } from "./page-integration-loader.js";

const locale = document.documentElement.dataset.locale === "de" ? "de" : "en";
const copy = locale === "de" ? de : en;
const localizedSiteViews = siteViews[locale];
const siteRoot = new URL("./", import.meta.url);
const staticPages = docPages.filter(page => page.view);
const staticPageIds = staticPages.map(page => page.view);
const docRoutes = docPages.filter(page => !page.view).map(page => ({ ...page, route: `doc-${page.id}` }));
const navigationPages = [
  ...staticPages.map(page => ({ ...page, route: page.view })),
  ...docRoutes,
];
const docPageByRoute = new Map(docRoutes.map(page => [page.route, page]));
const integrationPageByRoute = new Map(navigationPages.filter(page => page.integration).map(page => [page.route, page]));
const docPageIds = docRoutes.map(page => page.route);
const apiGuides = docRoutes.filter(page => page.apiSymbols?.length || page.apiPrefixes?.length);
const pageIds = [...staticPageIds, ...docPageIds];

const state = { page: "home", catalog: "api", apiSource: "all", api: [], apiVersion: null, apiLoaded: false, profile: null, snapshot: null, types: {}, typesLoaded: false, resources: {}, resourcesLoaded: false, manifest: null, extended: null, editor: "mod" };

const esc = value => String(value ?? "").replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
const fmt = value => Number(value || 0).toLocaleString(locale === "de" ? "de-DE" : "en-US");
const apiText = value => esc(value)
  .replace(/;\s*/g, ", ")
  .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
  .replace(/`([^`]+)`/g, "<code>$1</code>")
  .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const { loadMarkdownPage, copyText, addCopyControls } = createMarkdown({ locale, esc, pageIds, $$ });
const { loadCatalog, setCatalog, renderApi, renderTypes, renderResources } = createReferenceSearch({ locale, esc, fmt, apiText, copy, state, siteRoot, apiGuides, $, $$, addCopyControls });
const { loadPageIntegration } = createPageIntegrationLoader({ locale, copy, esc, state, copyText });
const { groupDocsForNavigation, setPage } = createNavigation({ locale, state, pageIds, docPageByRoute, integrationPageByRoute, loadPageIntegration, $, $$, loadCatalog, loadMarkdownPage });
const navGroups = groupDocsForNavigation(navigationPages);

document.documentElement.lang = locale;
document.title = locale === "de" ? "ShroudForge — Spiel-, Server- und Modding-Guide" : "ShroudForge — Player, server and modding guide";
document.querySelector('meta[name="description"]')?.setAttribute("content", locale === "de"
  ? "ShroudForge für PC und Server einrichten, Mods installieren und verstehen, wie Loader, Lua Runtime und KFC Runtime arbeiten."
  : "Set up ShroudForge for PC and servers, install mods, and learn how the loader, Lua runtime, and KFC Runtime work.");
localStorage.setItem("sf-language", locale);

function renderNavigationEntry(entry, depth = 0) {
  if (entry.type === "page") {
    return `<button class="nav-item${depth ? " nav-child" : ""} ${entry.id === "home" ? "active" : ""}" style="--nav-depth:${depth}" data-view="${entry.id}"><span>${esc(entry.label)}</span>${entry.id === "home" ? '<i class="nav-glow"></i>' : ""}</button>`;
  }

  const contents = entry.entries.map(child => renderNavigationEntry(child, depth + 1)).join("");
  if (entry.hasOverview || entry.pageCount < 2) {
    return `<div class="nav-subgroup nav-subgroup-section" style="--nav-depth:${depth}"><div class="nav-subgroup-title">${esc(entry.label)}</div>${contents}</div>`;
  }

  const countLabel = locale === "de" ? "Einträge" : "items";
  return `<details class="nav-subgroup" style="--nav-depth:${depth}"><summary><span>${esc(entry.label)}</span><small>${fmt(entry.pageCount)} ${countLabel}</small></summary><div class="nav-subgroup-items">${contents}</div></details>`;
}

const siteNavigation = navGroups.map(group =>
  `<div class="nav-group"><div class="nav-group-title">${esc(group.label)}</div>${group.entries.map(entry => renderNavigationEntry(entry)).join("")}</div>`,
).join("");
const pageMarkup = pageIds.map(id => docPageByRoute.has(id)
  ? `<section id="view-${id}" class="view doc-view"><article class="markdown-page" data-doc-page="${id}"><div class="doc-loading">${locale === "de" ? "Artikel wird geladen …" : "Loading article …"}</div></article></section>`
  : `<section id="view-${id}" class="view ${id === "home" ? "active" : ""}">${localizedSiteViews[id] || ""}</section>`).join("");

document.querySelector("#app").innerHTML = `
  <header class="topbar"><a href="#home" class="brand" data-view="home" aria-label="ShroudForge home"><img src="../media/shroudedit-hammer.png" alt=""><span><b>SHROUDFORGE</b><small>${copy.brand}</small></span></a><div class="top-actions"><a class="top-link" href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><a class="top-link release-link" href="https://github.com/bonsaibauer/shroudforge/releases/latest" target="_blank" rel="noreferrer">${copy.download} ↗</a><nav class="language-switch" aria-label="${locale === "de" ? "Sprache wählen" : "Choose language"}"><a data-language="de" href="../de/#${location.hash.slice(1) || "home"}" lang="de" title="Deutsch" aria-label="Deutsch" ${locale === "de" ? 'aria-current="page"' : ""}><img src="../media/flag-de.svg" alt=""></a><a data-language="en" href="../en/#${location.hash.slice(1) || "home"}" lang="en" title="English" aria-label="English" ${locale === "en" ? 'aria-current="page"' : ""}><img src="../media/flag-en.svg" alt=""></a></nav><button class="mobile-menu" id="mobile-menu" aria-label="${copy.menu}">☰</button></div></header>
  <aside class="sidebar"><div class="sidebar-label">${copy.menu}</div><nav>${siteNavigation}</nav></aside>
  <main class="main-content"><div class="content-wrap">${pageMarkup}<footer class="site-footer"><span>© SHROUDFORGE · ${copy.footer}</span><span><a href="https://github.com/bonsaibauer/shroudforge" target="_blank" rel="noreferrer">${copy.github} ↗</a><b>·</b><a href="#api" data-view="api">${copy.apiLabel}</a></span></footer></div></main>`;

document.addEventListener("click", event => {
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
});
$("#mobile-menu").addEventListener("click", () => $(".sidebar").classList.toggle("open"));
document.querySelectorAll("[data-language]").forEach(link => link.addEventListener("click", () => localStorage.setItem("sf-language", link.dataset.language)));
window.addEventListener("hashchange", () => setPage(location.hash.slice(1), false));
setPage(pageIds.includes(location.hash.slice(1)) ? location.hash.slice(1) : "home", false);

$("#api-search").addEventListener("input", event => renderApi(event.target.value));
$("#type-search").addEventListener("input", event => renderTypes(event.target.value));
$("#resource-search").addEventListener("input", event => renderResources(event.target.value));

addCopyControls();

