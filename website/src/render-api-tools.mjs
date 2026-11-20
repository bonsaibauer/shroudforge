import { escapeHtml, renderSiteFooter, renderSiteHeader } from './render-page.mjs'
import { renderToolsSidebar } from './render-tool-page.mjs'

const referenceRoute = '/tools/modding/api-reference/'
const playgroundRoute = '/tools/modding/api-playground/'

function pageShell({ config, siteUrl, generatedAt, route, active, title, description, body }) {
	const canonicalUrl = new URL(route, siteUrl).href
	return `<!doctype html>
<html lang="en" data-theme="light">
<head>
	<meta charset="utf-8">
	<meta name="viewport" content="width=device-width, initial-scale=1">
	<title>${escapeHtml(title)} | ${escapeHtml(config.name)}</title>
	<meta name="description" content="${escapeHtml(description)}">
	<meta name="theme-color" content="#ffffff">
	<meta name="color-scheme" content="light">
	<meta name="robots" content="${new URL(siteUrl).origin === new URL(config.siteUrl).origin ? 'index, follow' : 'noindex, nofollow'}">
	<link rel="canonical" href="${escapeHtml(canonicalUrl)}">
	<link rel="icon" href="/assets/favicon.ico" type="image/x-icon">
	<link rel="stylesheet" href="/assets/styles.css">
	<link rel="stylesheet" href="/assets/mod-settings.css">
	<link rel="stylesheet" href="/assets/api-tools.css">
	<script src="/assets/theme.js"></script>
	<script src="/assets/api-tools.js" defer></script>
</head>
<body>
	<a class="skip-link" href="#main-content" data-i18n="skip">Skip to API tools</a>
	${renderSiteHeader(config, 'tools')}
	<div class="page-shell tool-shell">
		${renderToolsSidebar(active, config)}
		<main id="main-content" class="main-content mod-settings-main api-tool-main" data-api-tool="${active}">
			${body}
		</main>
	</div>
	${renderSiteFooter({ config, generatedAt })}
</body>
</html>\n`
}

function apiVersionControls() {
	return `<div class="api-version-controls">
		<label class="preview-version-picker"><span data-i18n="apiVersion">ShroudForge API version</span><select data-api-version aria-label="ShroudForge API version" data-i18n-aria-label="apiVersion" disabled><option>Loading…</option></select></label>
		<label class="preview-version-picker"><span data-i18n="gameBuildLabel">Game Build</span><select data-game-build aria-label="Game Build" data-i18n-aria-label="gameBuildLabel" disabled><option>Loading…</option></select></label>
	</div>`
}

function apiPagination(name) {
	return `<nav class="api-pagination" data-api-pagination="${name}" aria-label="Search result pages" data-i18n-aria-label="paginationLabel" hidden></nav>`
}

function apiLoadError() {
	return `<div class="api-load-error editor-status has-error" data-api-load-error role="alert" hidden><div class="api-load-error-copy"><span data-api-load-error-message></span><small data-api-load-error-detail></small></div><button class="button button-secondary" type="button" data-api-retry data-i18n="retryLoading">Retry</button></div>`
}

function apiIntro({ titleKey, titleText, leadKey, leadText, actionLabel, actionHref }) {
	return `<section class="page-intro tool-intro" aria-labelledby="page-title">
		<div class="eyebrow" data-i18n="toolsEyebrow">SHROUDFORGE TOOLS · MODDING</div>
		<div class="tool-intro-row"><div><h1 id="page-title" data-i18n="${titleKey}">${titleText}</h1><p data-i18n="${leadKey}">${leadText}</p></div><div class="tool-intro-actions" role="group" aria-label="API tool navigation"><a class="button button-primary" href="${actionHref}" data-api-route="${actionHref}" data-i18n="${actionLabel.key}">${actionLabel.text}</a></div></div>
	</section>`
}

function taskCards() {
	return `<section class="mod-preview-section panel api-task-section" aria-labelledby="task-heading">
		<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="startWithTask">START WITH A TASK</span><h2 id="task-heading" data-i18n="commonTasks">What do you want your mod to do?</h2><p data-i18n="taskLead">Choose a sample to build a Lua call and preview its response.</p></div></header>
		<div class="api-load-state">${apiLoadError()}</div>
		<div class="api-task-grid" data-api-tasks><div class="api-loading" role="status" data-i18n="loadingExamples">Loading examples…</div></div>
	</section>`
}

export function renderApiReferencePage({ config, siteUrl, generatedAt }) {
	const body = `${apiIntro({
		titleKey: 'apiReferenceTitle',
		titleText: 'Browse the API',
		leadKey: 'apiReferenceLead',
		leadText: 'Start with a modding task or search the Lua functions, game types, and resource IDs.',
		actionLabel: { key: 'tryCallsAction', text: 'API calls' },
		actionHref: playgroundRoute,
	})}
		<section class="editor-stage panel api-reference-panel" aria-labelledby="api-reference-heading">
			<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="referenceEyebrow">API REFERENCE</span><h2 id="api-reference-heading" data-i18n="allApiEntries">Find an API entry</h2><p data-i18n="searchLead">Search by what you want to do, a function name, or a parameter.</p></div><div class="api-reference-heading-controls">${apiVersionControls()}<span class="api-result-count" data-api-result-count role="status" aria-live="polite"></span></div></header>
			<div class="api-search-row"><label class="api-search-box"><span class="visually-hidden" data-i18n="searchLabel">Search the API</span><input type="search" data-api-search autocomplete="off" placeholder="Try “read a setting” or shroudforge.settings.get" data-i18n-placeholder="searchPlaceholder"></label><button class="button button-secondary api-clear-button" type="button" data-api-clear data-i18n="clearSearch">Clear</button></div>
			<div class="api-filter-row" role="group" aria-label="API categories" data-i18n-aria-label="apiCategories"><button type="button" class="button button-secondary api-filter is-active" data-api-filter="all" aria-pressed="true" data-i18n="filterAll">Everything</button><button type="button" class="button button-secondary api-filter" data-api-filter="eml" aria-pressed="false">EML</button><button type="button" class="button button-secondary api-filter" data-api-filter="shroudforge" aria-pressed="false">ShroudForge</button><button type="button" class="button button-secondary api-filter" data-api-filter="runtime" aria-pressed="false" data-i18n="filterRuntime">Runtime</button><button type="button" class="button button-secondary api-filter" data-api-filter="game" aria-pressed="false" data-i18n="filterGame">Game API</button></div>
			<div class="api-catalog-tabs" role="tablist" aria-label="API reference sections" data-i18n-aria-label="apiReferenceSections"><button type="button" role="tab" class="api-tab is-active" aria-selected="true" data-api-tab="functions" data-i18n="functionsTab">Lua functions</button><button type="button" role="tab" class="api-tab" aria-selected="false" data-api-tab="types" data-i18n="typesTab">Game types</button><button type="button" role="tab" class="api-tab" aria-selected="false" data-api-tab="resources" data-i18n="resourcesTab">Game resources</button></div>
			<section class="api-tab-panel" data-api-panel="functions" role="tabpanel"><div class="api-results" data-api-results><div class="api-loading" role="status" data-i18n="loadingCatalog">Loading the API catalogue…</div></div>${apiPagination('functions')}</section>
			<section class="api-tab-panel" data-api-panel="types" role="tabpanel" hidden><p class="api-data-note" data-i18n="typeDataNote">These details come from the captured Enshrouded build shown above.</p><label class="api-search-box api-data-search"><span class="visually-hidden" data-i18n="searchTypes">Search game types</span><input type="search" data-type-search placeholder="Type name or field" data-i18n-placeholder="typeSearchPlaceholder"></label><div class="api-results" data-type-results></div>${apiPagination('types')}</section>
			<section class="api-tab-panel" data-api-panel="resources" role="tabpanel" hidden><p class="api-data-note" data-i18n="resourceDataNote">Resource names and IDs come from the same captured game build.</p><label class="api-search-box api-data-search"><span class="visually-hidden" data-i18n="searchResources">Search game resources</span><input type="search" data-resource-search placeholder="Resource name or ID" data-i18n-placeholder="resourceSearchPlaceholder"></label><div class="api-results" data-resource-results></div>${apiPagination('resources')}</section>
		</section>`
	return pageShell({
		config,
		siteUrl,
		generatedAt,
		route: referenceRoute,
		active: 'api-reference',
		title: 'Browse the ShroudForge Lua API',
		description: 'Search ShroudForge and EML Lua functions, captured Enshrouded game types, and resource IDs.',
		body,
	})
}

export function renderApiPlaygroundPage({ config, siteUrl, generatedAt }) {
	const body = `${apiIntro({
		titleKey: 'apiPlaygroundTitle',
		titleText: 'API calls',
		leadKey: 'apiPlaygroundLead',
		leadText: 'Build a Lua call from a modding example and preview a sample response.',
		actionLabel: { key: 'browseApiAction', text: 'Browse API' },
		actionHref: referenceRoute,
	})}
		${taskCards()}
		<section class="editor-stage panel api-playground-panel" aria-labelledby="playground-heading">
			<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="exampleEditor">API CALL EDITOR</span><h2 id="playground-heading" data-i18n="editRunExample">Build an API call</h2><p data-i18n="playgroundLead">Choose a task, edit its inputs, and preview an example response. This does not run Lua in the game.</p></div></header>
			${apiLoadError()}
			<div class="api-request-column">
				<label class="api-field-label"><span data-i18n="chooseExample">Choose a task</span><select data-api-scenario aria-label="Choose an API example"><option data-i18n="loadingExamplesOption">Loading examples…</option></select></label>
				<p class="api-scenario-summary" data-scenario-summary></p>
				<form data-api-request-form>
					<div class="api-request-fields" data-request-fields></div>
					<div class="api-code-heading"><strong data-i18n="generatedLua">Lua code for your mod</strong></div>
					<div class="api-code-row"><pre class="api-code-panel"><code data-generated-call></code></pre><button class="button button-secondary api-copy-button" type="button" data-copy-call data-i18n="copyCode">Copy</button></div>
					<p class="api-copy-status" data-copy-status role="status" aria-live="polite"></p>
					<button class="button button-primary api-run-button" type="submit" data-i18n="runExample">Show sample response</button>
				</form>
			</div>
		</section>
		<section class="editor-stage panel api-response-section" aria-labelledby="response-heading">
			<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="exampleResponse">SAMPLE RESPONSE</span><h2 id="response-heading" data-i18n="responseHeading">What the call may return</h2><p data-response-note data-i18n="runToSeeResponse">Choose a task and show its sample response.</p></div><span class="api-sample-badge" data-i18n="sampleOnly">Browser sample</span></header>
			<pre class="api-response-output" data-response-output aria-live="polite"><code data-i18n="noResponseYet">No response yet.</code></pre>
			<div class="api-data-source"><strong data-i18n="responseSource">Sample source</strong><p data-response-source data-i18n="sourceChanges">The source is shown after you preview a response.</p></div>
			<a class="api-reference-link" data-reference-link href="${referenceRoute}" data-i18n="viewApiEntry">View API entry →</a>
		</section>
		<footer class="tool-footer-note"><span data-i18n="playgroundFooter">Copy the Lua call into your mod. Check live availability with runtime.has or runtime.status in game.</span></footer>`
	return pageShell({
		config,
		siteUrl,
		generatedAt,
		route: playgroundRoute,
		active: 'api-playground',
		title: 'ShroudForge API calls',
		description: 'Build a Lua API example, edit its inputs, and preview a sample response from bundled ShroudForge or Enshrouded data.',
		body,
	})
}
