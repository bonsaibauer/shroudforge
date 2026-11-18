import { escapeHtml, renderSiteHeader } from './render-page.mjs'
import { renderToolsSidebar } from './render-tool-page.mjs'

const referenceRoute = '/tools/modding/api-reference/'
const playgroundRoute = '/tools/modding/api-playground/'

function pageShell({ config, siteUrl, route, active, title, description, body }) {
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
		<main id="main-content" class="main-content api-tool-main" data-api-tool="${active}">
			${body}
		</main>
	</div>
</body>
</html>\n`
}

function versionControls() {
	return `<div class="api-version-controls">
		<label class="preview-version-picker"><span data-i18n="apiVersion">ShroudForge API version</span><select data-api-version aria-label="ShroudForge API version" data-i18n-aria-label="apiVersion" disabled><option>Loading…</option></select><small data-api-version-note></small></label>
	</div>`
}

function statusPanel() {
	return `<section class="api-status-panel panel" aria-label="API catalogue status" data-i18n-aria-label="catalogStatusTitle">
		<div class="api-status-heading"><span class="api-status-icon" aria-hidden="true">i</span><div><strong data-i18n="catalogStatusTitle">Static API catalogue</strong><p data-i18n="catalogStatusBody">This page shows the published API contract and captured game data. It cannot see or run inside your current game.</p></div></div>
		<div class="api-status-facts"><span data-api-release></span><span data-game-build></span><span class="api-live-state" data-i18n="noLiveConnection">No live game connection</span></div>
		<details class="api-status-guide" open><summary data-i18n="statusGuideTitle">What do these API status badges mean?</summary><div class="api-status-guide-grid">
			<div><span class="api-status-mark is-documented" data-i18n="statusDocumented">Documented</span><p data-i18n="statusDocumentedBody">The API catalogue contains this entry and its published contract.</p></div>
			<div><span class="api-status-mark is-capability" data-i18n="statusCapability">Mod permission</span><p data-i18n="statusCapabilityBody">Runtime permission is derived from the capability checks already used by the Modloader.</p></div>
			<div><span class="api-status-mark is-declared" data-i18n="statusDeclared">Runtime declared</span><p data-i18n="statusDeclaredBody">The operation identifier is in the runtime operation catalogue. This does not mean it is available in every game process.</p></div>
			<div><span class="api-status-mark is-live" data-i18n="statusLive">Live availability</span><p data-i18n="statusLiveBody">Only runtime.status or runtime.has inside the running game can report current availability.</p></div>
			<div><span class="api-status-mark is-effect" data-i18n="statusEffect">Gameplay effect verification</span><p data-i18n="statusEffectBody">No central per-feature gameplay verification result is published here. Do not infer an in-game effect from an API declaration or live availability.</p></div>
		</div></details>
	</section>`
}

function apiIntro({ titleKey, titleText, leadKey, leadText, actionLabel, actionHref }) {
	return `<section class="page-intro tool-intro api-tool-intro" aria-labelledby="page-title">
		<div class="eyebrow" data-i18n="toolsEyebrow">SHROUDFORGE TOOLS · MODDING</div>
		<div class="api-tool-intro-row"><div><h1 id="page-title" data-i18n="${titleKey}">${titleText}</h1><p data-i18n="${leadKey}">${leadText}</p></div><div class="api-tool-intro-actions"><a class="button button-primary" href="${actionHref}" data-i18n="${actionLabel.key}">${actionLabel.text}</a>${versionControls()}</div></div>
	</section>`
}

function taskCards() {
	return `<section class="api-task-section panel" aria-labelledby="task-heading">
		<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="startWithTask">START WITH A TASK</span><h2 id="task-heading" data-i18n="commonTasks">What do you want your mod to do?</h2><p data-i18n="taskLead">Choose an example to see a working Lua call and what its response means.</p></div></header>
		<div class="api-task-grid" data-api-tasks><div class="api-loading" role="status" data-i18n="loadingExamples">Loading examples…</div></div>
	</section>`
}

export function renderApiReferencePage({ config, siteUrl }) {
	const body = `${apiIntro({
		titleKey: 'apiReferenceTitle',
		titleText: 'Browse the API',
		leadKey: 'apiReferenceLead',
		leadText: 'Find an example by task or search every documented function, type, and game resource.',
		actionLabel: { key: 'tryCallsAction', text: 'Try API calls' },
		actionHref: playgroundRoute,
	})}
		${statusPanel()}
		${taskCards()}
		<section class="api-reference-panel panel" aria-labelledby="api-reference-heading">
			<header class="tool-section-heading api-reference-heading"><div><span class="eyebrow" data-i18n="referenceEyebrow">SEARCHABLE REFERENCE</span><h2 id="api-reference-heading" data-i18n="allApiEntries">Find an API entry</h2><p data-i18n="searchLead">Search by the task you have in mind, a function name, or a parameter.</p></div><span class="api-result-count" data-api-result-count role="status" aria-live="polite"></span></header>
			<div class="api-search-row"><label class="api-search-box"><span class="visually-hidden" data-i18n="searchLabel">Search the API</span><span class="api-search-icon" aria-hidden="true">⌕</span><input type="search" data-api-search autocomplete="off" placeholder="Try “read a setting” or shroudforge.settings.get" data-i18n-placeholder="searchPlaceholder"></label><button class="button button-secondary api-clear-button" type="button" data-api-clear data-i18n="clearSearch">Clear</button></div>
			<div class="api-filter-row" role="group" aria-label="API categories" data-i18n-aria-label="apiCategories"><button type="button" class="api-filter is-active" data-api-filter="all" aria-pressed="true" data-i18n="filterAll">Everything</button><button type="button" class="api-filter" data-api-filter="eml" aria-pressed="false">EML</button><button type="button" class="api-filter" data-api-filter="shroudforge" aria-pressed="false">ShroudForge</button><button type="button" class="api-filter" data-api-filter="runtime" aria-pressed="false" data-i18n="filterRuntime">Runtime</button><button type="button" class="api-filter" data-api-filter="game" aria-pressed="false" data-i18n="filterGame">Game data</button></div>
			<div class="api-catalog-tabs" role="tablist" aria-label="API reference sections" data-i18n-aria-label="apiReferenceSections"><button type="button" role="tab" class="is-active" aria-selected="true" data-api-tab="functions" data-i18n="functionsTab">Lua functions</button><button type="button" role="tab" aria-selected="false" data-api-tab="types" data-i18n="typesTab">Game types</button><button type="button" role="tab" aria-selected="false" data-api-tab="resources" data-i18n="resourcesTab">Game resources</button></div>
			<section class="api-tab-panel" data-api-panel="functions" role="tabpanel"><div class="api-results" data-api-results><div class="api-loading" role="status" data-i18n="loadingCatalog">Loading the API catalogue…</div></div></section>
			<section class="api-tab-panel" data-api-panel="types" role="tabpanel" hidden><p class="api-data-note" data-i18n="typeDataNote">These details come from a captured Enshrouded build. They do not reflect a live game process.</p><label class="api-search-box api-data-search"><span class="visually-hidden" data-i18n="searchTypes">Search game types</span><span class="api-search-icon" aria-hidden="true">⌕</span><input type="search" data-type-search placeholder="Type name or field" data-i18n-placeholder="typeSearchPlaceholder"></label><div class="api-results" data-type-results></div></section>
			<section class="api-tab-panel" data-api-panel="resources" role="tabpanel" hidden><p class="api-data-note" data-i18n="resourceDataNote">Resource names and IDs come from the same captured game data snapshot.</p><label class="api-search-box api-data-search"><span class="visually-hidden" data-i18n="searchResources">Search game resources</span><span class="api-search-icon" aria-hidden="true">⌕</span><input type="search" data-resource-search placeholder="Resource name or ID" data-i18n-placeholder="resourceSearchPlaceholder"></label><div class="api-results" data-resource-results></div></section>
		</section>
		<footer class="api-tool-footer"><span data-i18n="referenceFooter">The catalogue documents the Lua interface. In-game availability depends on runtime conditions and is checked from a mod with runtime.has or runtime.status.</span></footer>`
	return pageShell({
		config,
		siteUrl,
		route: referenceRoute,
		active: 'api-reference',
		title: 'Browse the ShroudForge Lua API',
		description: 'Search ShroudForge and EML Lua functions, captured Enshrouded game types, and resource IDs. Inspect examples and runtime requirements.',
		body,
	})
}

export function renderApiPlaygroundPage({ config, siteUrl }) {
	const body = `${apiIntro({
		titleKey: 'apiPlaygroundTitle',
		titleText: 'Try API calls',
		leadKey: 'apiPlaygroundLead',
		leadText: 'Choose a real modding task, edit its inputs, and see the Lua call and a sample response.',
		actionLabel: { key: 'browseApiAction', text: 'Browse API' },
		actionHref: referenceRoute,
	})}
		${statusPanel()}
		<section class="api-playground-panel panel" aria-labelledby="playground-heading">
			<header class="tool-section-heading"><div><span class="eyebrow" data-i18n="exampleEditor">EXAMPLE CALL EDITOR</span><h2 id="playground-heading" data-i18n="editRunExample">Edit and run an example</h2><p data-i18n="playgroundLead">Change the inputs below. The browser answers from the selected example data or a clearly marked runtime fixture.</p></div></header>
			<div class="api-playground-grid">
				<div class="api-request-column">
					<label class="api-field-label"><span data-i18n="chooseExample">Choose an example</span><select data-api-scenario aria-label="Choose an API example"><option data-i18n="loadingExamplesOption">Loading examples…</option></select></label>
					<p class="api-scenario-summary" data-scenario-summary></p>
					<form data-api-request-form>
						<div class="api-request-fields" data-request-fields></div>
					<div class="api-code-heading"><strong data-i18n="generatedLua">Generated Lua call</strong><button class="text-link" type="button" data-copy-call data-i18n="copyCode">Copy code</button></div>
						<pre class="api-code-panel"><code data-generated-call></code></pre>
					<p class="api-copy-status" data-copy-status role="status" aria-live="polite"></p>
						<button class="button button-primary api-run-button" type="submit" data-i18n="runExample">Run example</button>
					</form>
				</div>
				<div class="api-response-column">
					<div class="api-response-heading"><div><span class="eyebrow" data-i18n="exampleResponse">EXAMPLE RESPONSE</span><h3 data-i18n="responseHeading">What the call returns</h3></div><span class="api-sample-badge" data-i18n="sampleOnly">Sample only</span></div>
					<p class="api-response-note" data-response-note data-i18n="runToSeeResponse">Run the example to see a response.</p>
					<pre class="api-response-output" data-response-output aria-live="polite"><code data-i18n="noResponseYet">No response yet.</code></pre>
					<div class="api-data-source"><strong data-i18n="responseSource">Response source</strong><p data-response-source data-i18n="sourceChanges">The source is shown after you run an example.</p></div>
					<a class="api-reference-link" data-reference-link href="${referenceRoute}" data-i18n="viewApiEntry">View API entry →</a>
				</div>
			</div>
		</section>
		<section class="api-meaning-panel panel" aria-labelledby="meaning-heading"><div class="api-meaning-mark" aria-hidden="true">!</div><div><h2 id="meaning-heading" data-i18n="simulationHeading">What does “Run example” do?</h2><p data-i18n="simulationBody">It runs a small, allowlisted browser simulation. Settings and game-type examples read the included sample files. Runtime examples show a fixture. No Lua script is sent to a server or to your game.</p></div></section>
		<footer class="api-tool-footer"><span data-i18n="playgroundFooter">Use the generated Lua in your mod, then check live availability with runtime.has or runtime.status in game.</span></footer>`
	return pageShell({
		config,
		siteUrl,
		route: playgroundRoute,
		active: 'api-playground',
		title: 'Try ShroudForge API calls',
		description: 'Build a Lua API example, edit its inputs, and see a sample response from bundled ShroudForge or Enshrouded data.',
		body,
	})
}
