export function createReferenceSearch({ locale, esc, fmt, apiText, copy, state, siteRoot, apiGuides, $, $$, addCopyControls }) {
  let apiLoadPromise;
  let gameProfilePromise;
  const gameDataPromises = {};

  function apiGuideFor(item) {
    const exact = apiGuides.find(page => page.apiSymbols?.includes(item.name));
    if (exact) return exact;
    return apiGuides.flatMap(page => (page.apiPrefixes || [])
      .filter(prefix => item.name.startsWith(prefix))
      .map(prefix => ({ page, length: prefix.length })))
      .sort((left, right) => right.length - left.length)[0]?.page;
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

  async function loadCatalog() {
    if (!$("#api-results")) return;
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
    const sourceUrl = `https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/mod-runtime/lua/${item.source}`;
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

  return { loadCatalog, setCatalog, renderApi, renderTypes, renderResources };
}
