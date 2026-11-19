import { escapeHtml, renderSiteFooter, renderSiteHeader } from './render-page.mjs'
import { icon } from './icons.mjs'

const route = '/tools/modding/mod-settings/'
const schemasRoute = '/schemas/'
const apiPlaygroundRoute = '/tools/modding/api-playground/'
const apiReferenceRoute = '/tools/modding/api-reference/'

export function renderToolsSidebar(active, config) {
	return `<aside class="sidebar tools-sidebar" aria-label="Tools navigation">
		<div class="tools-sidebar-panel panel">
			<h2 data-i18n="toolsNavLabel">Tools</h2>
			<div class="tools-nav-group">
				<div class="tools-category-label" data-i18n="moddingNavLabel">Modding</div>
				<a class="tool-nav-item${active === 'mod-settings' ? ' is-active' : ''}" href="${route}"${active === 'mod-settings' ? ' aria-current="page"' : ''} data-i18n="modSettingsNav">Mod-Settings</a>
				<a class="tool-nav-item${active === 'api-playground' ? ' is-active' : ''}" href="${apiPlaygroundRoute}"${active === 'api-playground' ? ' aria-current="page"' : ''} data-i18n="apiCallsNav">API calls</a>
				<a class="tool-nav-item${active === 'api-reference' ? ' is-active' : ''}" href="${apiReferenceRoute}"${active === 'api-reference' ? ' aria-current="page"' : ''} data-i18n="browseApiNav">Browse API</a>
			</div>
			<div class="tools-nav-group">
				<div class="tools-category-label" data-i18n="standardsNavLabel">Standards</div>
				<a class="tool-nav-item${active === 'schemas' ? ' is-active' : ''}" href="${schemasRoute}"${active === 'schemas' ? ' aria-current="page"' : ''}>Schemas</a>
				<a class="tool-nav-item" href="${escapeHtml(config.dinput8ChainStandardUrl)}">${icon('external')} DInput8 Standard</a>
			</div>
		</div>
	</aside>`
}

function jsonTemplate(id, value) {
	const text = JSON.stringify(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
	return `<template id="${id}">${text}</template>`
}

export function renderModSettingsPage({ config, siteUrl, generatedAt, version, manifest, extended, links }) {
	const canonicalUrl = new URL(route, siteUrl).href
	const toolTitle = 'Mod Settings editor and live preview'
	const description = 'Build a mod settings page in your browser. Preview the current ShroudForge Modloader UI and edit mod.json and extended.mod.json together.'
	const example = { manifest, extended, links, version, schemaBaseUrl: new URL('/schemas/', siteUrl).href }
	const exampleSourceUrl = config.repository ? `https://github.com/${config.repository}/tree/${version}/docs/examples/starter-mod` : '#'
	const exampleSourceUrlTemplate = config.repository ? `https://github.com/${config.repository}/tree/{version}/docs/examples/starter-mod` : '#'
	return `<!doctype html>
<html lang="en" data-theme="light">
<head>
	<meta charset="utf-8">
	<meta name="viewport" content="width=device-width, initial-scale=1">
	<title>${escapeHtml(toolTitle)} | ${escapeHtml(config.name)}</title>
	<meta name="description" content="${escapeHtml(description)}">
	<meta name="theme-color" content="#ffffff">
	<meta name="color-scheme" content="light">
	<meta name="robots" content="${new URL(siteUrl).origin === new URL(config.siteUrl).origin ? 'index, follow' : 'noindex, nofollow'}">
	<link rel="canonical" href="${escapeHtml(canonicalUrl)}">
	<link rel="icon" href="/assets/favicon.ico" type="image/x-icon">
	<link rel="stylesheet" href="/assets/styles.css">
	<link rel="stylesheet" href="/assets/mod-settings.css">
	<script src="/assets/theme.js"></script>
	<script src="/assets/mod-settings.js" defer></script>
</head>
<body>
	<a class="skip-link" href="#main-content">Skip to Mod Settings</a>
	${renderSiteHeader(config, 'tools')}
	<div class="page-shell tool-shell">
		${renderToolsSidebar('mod-settings', config)}
		<main id="main-content" class="main-content mod-settings-main" data-mod-settings-tool>
			<section class="page-intro tool-intro" aria-labelledby="page-title">
				<div class="eyebrow">SHROUDFORGE TOOLS · MODDING</div>
				<div class="tool-intro-row"><div><h1 id="page-title">Mod-Settings</h1><p>Shape the mod page, then refine the JSON. The preview uses the current Modloader frontend and updates as you edit.</p></div><div class="tool-intro-actions" role="group" aria-label="Example and JSON downloads"><button class="button button-secondary reset-example-button" type="button" data-reset-example>Reset example</button><div class="tool-download-actions"><button class="button button-primary download-json-button" type="button" data-download-json="manifest">Download mod.json</button><button class="button button-primary download-json-button" type="button" data-download-json="extended">Download extended-mod.json</button></div></div></div>
			</section>
			<section class="mod-preview-section panel" aria-labelledby="preview-heading">
				<div class="tool-section-heading">
					<div><span class="eyebrow">LIVE MODLOADER UI</span><h2 id="preview-heading">Your mod preview</h2><p>One example mod is loaded. Preview controls work locally and never send data to a server.</p></div>
					<div class="preview-version-controls"><label class="preview-version-picker"><span>Modloader version</span><select data-modloader-version aria-label="Modloader version"><option value="${escapeHtml(version)}">${escapeHtml(version)}</option></select></label><a class="preview-example-source-link" data-example-source-link data-version-url-template="${escapeHtml(exampleSourceUrlTemplate)}" href="${escapeHtml(exampleSourceUrl)}" target="_blank" rel="noopener" aria-label="Open the Starter Mod example on GitHub" title="Open the Starter Mod example on GitHub">${icon('github')}</a></div>
				</div>
				<iframe class="modloader-preview-frame" data-modloader-preview src="${route}preview.html" title="Live ShroudForge Modloader preview" loading="eager" scrolling="no"></iframe>
			</section>
			<section class="editor-stage panel" aria-labelledby="manifest-heading">
				<header class="tool-section-heading"><div><span class="eyebrow">MOD.JSON</span><h2 id="manifest-heading">Mod details</h2><p>Use the quick fields or edit the JSON directly. Both stay in sync.</p></div></header>
				<div class="quick-fields mod-quick-fields" data-manifest-fields>
					<label>Mod ID<input data-manifest-field="id" autocomplete="off" spellcheck="false"></label>
					<label>Name<input data-manifest-field="name" autocomplete="off"></label>
					<label>Mod version<input data-manifest-field="version" autocomplete="off" placeholder="1.0.0"></label>
					<label class="field-wide">Description<input data-manifest-field="description" autocomplete="off"></label>
					<label class="field-wide">Authors<input data-manifest-field="authors" autocomplete="off" placeholder="Separate names with commas"></label>
					<label>License<input data-manifest-field="license" autocomplete="off" placeholder="MIT"><small>Optional. Leave empty to remove it.</small></label>
					<label>Icon path<input data-manifest-field="icon" autocomplete="off" placeholder="icon.svg"><small>File path inside the mod package.</small></label>
				</div>
				<section class="manifest-list-editor" aria-labelledby="manifest-dependencies-heading">
					<div class="manifest-editor-heading"><div><h3 id="manifest-dependencies-heading">Dependencies</h3><p>Add required or optional mods and their version ranges.</p></div><button class="manifest-add-button" type="button" data-add-manifest-dependency>Add dependency</button></div>
					<div class="manifest-dependencies" data-manifest-dependencies></div>
				</section>
				<section class="manifest-list-editor" aria-labelledby="manifest-capabilities-heading">
					<div class="manifest-editor-heading"><div><h3 id="manifest-capabilities-heading">Capabilities</h3><p>Select every capability declared by this mod.</p></div></div>
					<div class="manifest-capabilities" data-manifest-capabilities aria-label="Manifest capabilities"></div>
				</section>
				<label class="json-editor-label" for="manifest-editor"><span>mod.json</span><span class="json-editor-label-actions"><a data-schema-link="manifest" href="${new URL(`/schemas/${version}/manifest.schema.json`, siteUrl).href}">Schema ${escapeHtml(version)}</a><small>JSON</small></span></label>
				<div class="json-editor-frame"><div class="json-line-numbers" aria-hidden="true"><div data-line-number-list="manifest"></div></div><textarea id="manifest-editor" class="json-editor" data-json-editor="manifest" spellcheck="false" autocapitalize="off" autocomplete="off" wrap="off" aria-label="mod.json"></textarea></div>
				<p class="editor-status" data-json-status="manifest" role="status" aria-live="polite"><span class="json-warning-icon" data-json-warning hidden aria-hidden="true">!</span><span data-json-message></span></p>
			</section>
			<section class="editor-stage panel" aria-labelledby="extended-heading">
				<header class="tool-section-heading"><div><span class="eyebrow">EXTENDED.MOD.JSON</span><h2 id="extended-heading">Settings and badges</h2><p>Choose a setting template or a link badge. The tool adds the fields and updates the JSON for you.</p></div></header>
				<section class="builder-block badge-builder-block" aria-labelledby="badge-builder-heading">
					<div class="builder-block-heading"><div><h3 id="badge-builder-heading">Add link badges</h3><p>Click a badge to add it or edit its link. Selected badges are marked in the catalog.</p></div></div>
					<div class="template-catalog badge-template-catalog" data-badge-catalog aria-label="Link badge templates"></div>
					<div class="template-fields badge-fields" data-badge-fields></div>
				</section>
				<section class="builder-block" aria-labelledby="settings-builder-heading">
					<div class="builder-block-heading"><div><h3 id="settings-builder-heading">Add a setting</h3><p>Choose how players will control this setting.</p></div></div>
					<div class="template-catalog setting-template-catalog" data-setting-catalog aria-label="Setting templates"></div>
					<div class="template-fields" data-setting-fields hidden></div>
					<div class="builder-items" data-setting-items></div>
				</section>
				<label class="json-editor-label" for="extended-editor"><span>extended.mod.json</span><span class="json-editor-label-actions"><a data-schema-link="extended" href="${new URL(`/schemas/${version}/extended.mod.schema.json`, siteUrl).href}">Schema ${escapeHtml(version)}</a><small>JSON</small></span></label>
				<div class="json-editor-frame"><div class="json-line-numbers" aria-hidden="true"><div data-line-number-list="extended"></div></div><textarea id="extended-editor" class="json-editor" data-json-editor="extended" spellcheck="false" autocapitalize="off" autocomplete="off" wrap="off" aria-label="extended.mod.json"></textarea></div>
				<p class="editor-status" data-json-status="extended" role="status" aria-live="polite"><span class="json-warning-icon" data-json-warning hidden aria-hidden="true">!</span><span data-json-message></span></p>
			</section>
			<footer class="tool-footer-note"><span>Browser-only tool. Changes stay on this page until you copy or save the JSON.</span></footer>
		</main>
	</div>
	${jsonTemplate('mod-settings-example', example)}
	${renderSiteFooter({ config, generatedAt })}
</body>
</html>\n`
}
