import { escapeHtml, renderSiteFooter, renderSiteHeader } from './render-page.mjs'
import { renderToolsSidebar } from './render-tool-page.mjs'

const route = '/schemas/'

const schemaDefinitions = [
	{
		file: 'manifest.schema.json',
		title: 'mod.json',
		description: 'Package identity, authors, dependencies, capabilities and other EML manifest fields.',
	},
	{
		file: 'extended.mod.schema.json',
		title: 'extended.mod.json',
		description: 'ShroudForge targets, settings, groups, presets, actions and mod page links.',
	},
	{
		file: 'mod-locale.schema.json',
		title: 'Mod locale files',
		description: 'Localized names, descriptions, settings, options and action text for a mod.',
	},
]

function renderSchemaCard(version, schema, siteUrl) {
	const url = new URL(`/schemas/${version}/${schema.file}`, siteUrl).href
	return `<article class="schema-card panel">
		<div class="schema-card-heading"><span class="schema-file-mark" aria-hidden="true">{ }</span><div><h3>${escapeHtml(schema.title)}</h3><code>${escapeHtml(schema.file)}</code></div></div>
		<p>${escapeHtml(schema.description)}</p>
		<div class="schema-card-actions"><a class="schema-action-link" href="${escapeHtml(url)}" target="_blank" rel="noopener">Open JSON</a><a class="schema-action-link" href="${escapeHtml(url)}" download>Download</a><button class="schema-copy-button" type="button" data-copy-schema="${escapeHtml(url)}">Copy URL</button></div>
		<output class="schema-copy-status" aria-live="polite"></output>
	</article>`
}

export function renderSchemasPage({ config, siteUrl, generatedAt, schemaVersions, currentVersion }) {
	const canonicalUrl = new URL(route, siteUrl).href
	const indexable = new URL(siteUrl).origin === new URL(config.siteUrl).origin
	const versions = [...schemaVersions].sort((left, right) => {
		const a = left.split('.').map(Number)
		const b = right.split('.').map(Number)
		return b[0] - a[0] || b[1] - a[1] || b[2] - a[2]
	})
	const versionOptions = versions.map(version => {
		const manifestSchemaUrl = new URL(`/schemas/${version}/manifest.schema.json`, siteUrl).href
		const sourceUrl = config.repository ? `https://github.com/${config.repository}/tree/${version}/modules/modloader/manifest` : ''
		return `<option value="${escapeHtml(version)}" data-manifest-schema-url="${escapeHtml(manifestSchemaUrl)}" data-source-url="${escapeHtml(sourceUrl)}" ${version === currentVersion ? 'selected' : ''}>${escapeHtml(version)}</option>`
	}).join('')
	const versionSections = versions.map(version => `
		<section class="schema-version-section" data-schema-version-section="${escapeHtml(version)}" aria-label="Schemas for Modloader ${escapeHtml(version)}" ${version === currentVersion ? '' : 'hidden'}>
			<div class="schema-grid">${schemaDefinitions.map(schema => renderSchemaCard(version, schema, siteUrl)).join('')}</div>
		</section>`).join('')
	const manifestSchemaUrl = new URL(`/schemas/${currentVersion}/manifest.schema.json`, siteUrl).href
	const sourceUrl = config.repository ? `https://github.com/${config.repository}/tree/${currentVersion}/modules/modloader/manifest` : '#'
	return `<!doctype html>
<html lang="en" data-theme="light">
<head>
	<meta charset="utf-8">
	<meta name="viewport" content="width=device-width, initial-scale=1">
	<title>Schema | ${escapeHtml(config.name)}</title>
	<meta name="description" content="Versioned ShroudForge Modloader schemas for mod manifests, settings extensions and localized mod text.">
	<meta name="theme-color" content="#ffffff">
	<meta name="color-scheme" content="light">
	<meta name="robots" content="${indexable ? 'index, follow' : 'noindex, nofollow'}">
	<link rel="canonical" href="${escapeHtml(canonicalUrl)}">
	<link rel="icon" href="/assets/favicon.ico" type="image/x-icon">
	<link rel="stylesheet" href="/assets/styles.css">
	<link rel="stylesheet" href="/assets/mod-settings.css">
	<link rel="stylesheet" href="/assets/schemas.css">
	<script src="/assets/theme.js"></script>
	<script src="/assets/schemas.js" defer></script>
</head>
<body>
	<a class="skip-link" href="#main-content">Skip to Schema</a>
	${renderSiteHeader(config, 'tools')}
	<div class="page-shell tool-shell">
		${renderToolsSidebar('schemas', config)}
		<main id="main-content" class="main-content schemas-main">
			<section class="page-intro schema-intro" aria-labelledby="page-title">
				<div class="eyebrow">SHROUDFORGE TOOLS · STANDARDS</div>
				<h1 id="page-title">Schema</h1>
				<p>Versioned contracts for ShroudForge mod packages. Pin a schema URL to the Modloader version your mod supports.</p>
			</section>
			<section class="schema-reference panel" aria-labelledby="schema-reference-title">
				<header class="tool-section-heading"><div><span class="eyebrow">MACHINE-READABLE REFERENCES</span><h2 id="schema-reference-title">Mod package schemas</h2><p>Open a schema in your editor or copy its version-specific URL into the <code>$schema</code> field.</p></div><label class="preview-version-picker schema-version-picker"><span>Modloader version</span><select data-schema-version aria-label="Modloader version">${versionOptions}</select></label></header>
				<div class="schema-manifest-example"><span>Example for <code>mod.json</code></span><code class="schema-url-value" data-schema-manifest-example>"$schema": "${escapeHtml(manifestSchemaUrl)}"</code></div>
				${versionSections}
			</section>
			<footer class="schema-footer-note"><span>These files are static, versioned downloads. A published version keeps its schema URL.</span><a data-schema-source-link href="${escapeHtml(sourceUrl)}">Schema source in the repository</a></footer>
		</main>
	</div>
	${renderSiteFooter({ config, generatedAt })}
</body>
</html>\n`
}
