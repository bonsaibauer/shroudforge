const state = { profile: null, api: [], types: {}, resources: {}, view: "overview", apiNamespace: "all", apiQuery: "" };
const $ = (selector) => document.querySelector(selector);
const format = (value) => Number(value || 0).toLocaleString("de-DE");
const escape = (value) => String(value ?? "").replace(/[&<>"']/g, character => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"})[character]);

async function load() {
  const current = await fetch("./data/current.json").then(response => response.json());
  const base = `./data/${current.snapshot}`;
  const [profile, api, types, resources] = await Promise.all([
    fetch(`${base}/profile.json`).then(response => response.json()),
    fetch("./data/api.json").then(response => response.json()),
    fetch(`${base}/types.json`).then(response => response.json()),
    fetch(`${base}/resources.json`).then(response => response.json()),
  ]);
  Object.assign(state, { profile, api: api.symbols, types, resources });
  renderMeta(api.version);
  renderApi(""); renderTypes(""); renderResources("");
  $("#build-state").textContent = "Spielkatalog geladen";
}

function renderMeta(apiVersion) {
  const profile = state.profile;
  const [build, branch, timestamp] = profile.game_version.split("|");
  $("#api-count").textContent = format(state.api.length);
  $("#type-count").textContent = format(profile.type_count);
  $("#resource-count").textContent = format(profile.resource_type_count);
  $("#metric-types").textContent = format(profile.type_count);
  $("#metric-fields").textContent = format(profile.field_count);
  $("#metric-resources").textContent = format(profile.resource_type_count);
  $("#metric-symbols").textContent = format(state.api.length);
  $("#footer-api").textContent = apiVersion;
  $("#footer-game").textContent = `${build} · ${branch.replace(/^\^\//, "")}`;
  $("#footer-date").textContent = new Intl.DateTimeFormat("de-DE", {dateStyle:"medium",timeStyle:"short",timeZone:"UTC"}).format(new Date(timestamp)) + " UTC";
  $("#footer-parser").textContent = profile.parser;
}

function renderApi(query) {
  state.apiQuery = query;
  const needle = query.trim().toLocaleLowerCase();
  const matches = state.api.filter(item => (state.apiNamespace === "all" || item.namespace === state.apiNamespace) && `${item.name} ${item.signature} ${item.description}`.toLocaleLowerCase().includes(needle)).slice(0, 300);
  $("#api-results").innerHTML = matches.length ? matches.map(item => `<details class="result"><summary><h3>${escape(item.name)}</h3><span class="chips"><span class="chip">${item.kind === "field" ? "Feld" : "Funktion"}</span><span class="chip">${escape(item.source)}</span></span></summary><div class="detail"><pre class="signature"><code>${escape(item.signature)}</code></pre>${item.description ? `<p>${escape(item.description)}</p>` : ""}${item.params.length ? `<table class="field-table"><thead><tr><th>Parameter</th><th>Typ</th><th>Beschreibung</th></tr></thead><tbody>${item.params.map(value => `<tr><td>${escape(value.name)}</td><td>${escape(value.type)}</td><td>${escape(value.description)}</td></tr>`).join("")}</tbody></table>` : ""}${item.returns.length ? `<p class="hint">${item.kind === "field" ? "Typ" : "Rückgabe"}: <code>${escape(item.returns.join(", "))}</code></p>` : ""}</div></details>`).join("") : empty("Kein API-Symbol gefunden.");
}

function renderTypes(query) {
  const needle = query.trim().toLocaleLowerCase();
  const matches = Object.values(state.types).filter(type => {
    if (!needle) return true;
    return type.qualified_name.toLocaleLowerCase().includes(needle) || Object.values(type.fields || {}).some(field => `${field.name} ${field.type_name}`.toLocaleLowerCase().includes(needle));
  }).slice(0, 200);
  $("#type-results").innerHTML = matches.length ? matches.map(type => `<details class="result"><summary><h3>${escape(type.qualified_name)}</h3><span class="chips"><span class="chip">${escape(type.primitive)}</span><span class="chip">${format(type.field_count)} Felder</span></span></summary><div class="detail"><div class="detail-grid"><div class="datum"><span>Größe</span><b>${format(type.size)} Bytes</b></div><div class="datum"><span>Ausrichtung</span><b>${format(type.alignment)}</b></div><div class="datum"><span>Spielname</span><b>${escape(type.impact_name)}</b></div><div class="datum"><span>Namespace</span><b>${escape((type.namespace || []).join("::"))}</b></div></div>${fieldTable(type.fields)}${enumTable(type.enum_values)}</div></details>`).join("") : empty("Kein Spieltyp gefunden.");
}

function fieldTable(fields) {
  const values = Object.values(fields || {});
  if (!values.length) return "";
  return `<table class="field-table"><thead><tr><th>Feld</th><th>Exakter Typ</th><th>Offset</th></tr></thead><tbody>${values.map(field => `<tr><td>${escape(field.name)}</td><td>${escape(field.type_name)}</td><td>${format(field.data_offset)}</td></tr>`).join("")}</tbody></table>`;
}
function enumTable(values) {
  const entries = Object.entries(values || {});
  if (!entries.length) return "";
  return `<table class="field-table"><thead><tr><th>Enum-Wert</th><th>Wert</th></tr></thead><tbody>${entries.map(([name,value]) => `<tr><td>${escape(name)}</td><td>${escape(value)}</td></tr>`).join("")}</tbody></table>`;
}

function renderResources(query) {
  const needle = query.trim().toLocaleLowerCase();
  const matches = Object.entries(state.resources).filter(([name, values]) => !needle || name.toLocaleLowerCase().includes(needle) || values.some(value => value.guid.toLocaleLowerCase().includes(needle))).slice(0, 200);
  $("#resource-results").innerHTML = matches.length ? matches.map(([name, values]) => `<details class="result"><summary><h3>${escape(name)}</h3><span class="chips"><span class="chip">${format(values.length)} Werte</span></span></summary><div class="detail"><table class="field-table"><thead><tr><th>GUID</th><th>Part</th></tr></thead><tbody>${values.slice(0,1000).map(value => `<tr><td>${escape(value.guid)}</td><td>${escape(value.part)}</td></tr>`).join("")}</tbody></table></div></details>`).join("") : empty("Keine Ressource gefunden.");
}

function empty(message) { return `<div class="empty">${escape(message)}</div>`; }
document.querySelectorAll(".nav-item").forEach(button => button.addEventListener("click", () => {
  state.view = button.dataset.view;
  document.querySelectorAll(".nav-item").forEach(item => item.classList.toggle("active", item === button));
  document.querySelectorAll(".view").forEach(view => view.classList.toggle("active", view.id === state.view));
  location.hash = state.view;
}));
document.querySelectorAll("[data-search]").forEach(input => input.addEventListener("input", () => ({api:renderApi,types:renderTypes,resources:renderResources})[input.dataset.search](input.value)));
document.querySelectorAll("[data-namespace]").forEach(button => button.addEventListener("click", () => {
  state.apiNamespace = button.dataset.namespace;
  document.querySelectorAll("[data-namespace]").forEach(item => item.classList.toggle("active", item === button));
  renderApi(state.apiQuery);
}));
const initial = location.hash.slice(1); if (initial && $(`[data-view="${initial}"]`)) $(`[data-view="${initial}"]`).click();
load().catch(error => { $("#build-state").textContent = "Katalog konnte nicht geladen werden"; $("#build-state").parentElement.querySelector("i").style.background = "var(--danger)"; console.error(error); });
