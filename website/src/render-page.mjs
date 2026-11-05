import { icon } from './icons.mjs'
import { escapeHtml } from './html.mjs'
import { renderSecurityRow, renderVirusTotalLink } from './render-security.mjs'

export { escapeHtml } from './html.mjs'

function releaseId(release) {
	return `release-${encodeURIComponent(release.tag).replace(/%/g, '-')}`
}

function formatDate(value) {
	return new Intl.DateTimeFormat('en-GB', {
		day: '2-digit', month: 'short', year: 'numeric', timeZone: 'UTC',
	}).format(new Date(value))
}

function formatSize(bytes) {
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function badge(prerelease) {
	return `<span class="badge ${prerelease ? 'badge-development' : 'badge-stable'}"><span class="status-dot"></span>${prerelease ? 'Development' : 'Stable'}</span>`
}

function downloadCard(release, { title, description, kind }, securityReport) {
	const symbol = kind === 'stable' ? 'shield' : 'branch'
	if (!release) {
		return `<section class="download-card download-card-empty panel" aria-label="${title}">
			<div class="card-label">${icon(symbol)} ${title}</div>
			<h2>No ${kind === 'stable' ? 'stable' : 'development'} release yet</h2>
			<p>${kind === 'stable' ? 'A recommended download will appear here when a stable release is published.' : 'Development builds will appear here when a prerelease is published on GitHub.'}</p>
		</section>`
	}
	return `<section class="download-card panel ${kind === 'stable' ? 'recommended-card' : ''}" aria-label="${title}">
		<div class="card-label">${icon(symbol)} ${title}${kind === 'stable' ? '<span class="recommended-label">Recommended</span>' : ''}</div>
		<div class="card-version"><h2>${escapeHtml(release.version)}</h2>${badge(release.prerelease)}</div>
		<p class="card-description">${description}</p>
		<div class="card-metadata"><span>Build ${escapeHtml(release.build)}</span><span aria-hidden="true">·</span><time datetime="${escapeHtml(release.publishedAt)}">${formatDate(release.publishedAt)}</time><span aria-hidden="true">·</span><span>${formatSize(release.asset.size)}</span></div>
		<a class="button ${kind === 'stable' ? 'button-primary' : 'button-secondary'} download" href="${escapeHtml(release.asset.url)}" aria-label="Download ${escapeHtml(release.asset.name)}">${icon('download')} Download ZIP</a>
		<a class="card-changelog" href="${escapeHtml(release.releaseUrl)}">View release notes ${icon('external')}</a>
		${renderVirusTotalLink(securityReport, { card: true })}
	</section>`
}

function releaseRow(release, securityReport) {
	const escapedVersion = escapeHtml(release.version)
	return `<tr id="${releaseId(release)}" data-release data-version="${escapedVersion}" data-channel="${release.prerelease ? 'development' : 'stable'}" data-search="${escapeHtml(`${release.tag} ${release.version} ${release.build} ${release.asset.name}`.toLowerCase())}">
		<td class="release-version-cell"><div class="release-version"><a class="version-link" href="${escapeHtml(release.releaseUrl)}">${escapedVersion}</a>${badge(release.prerelease)}</div><span class="release-build">Build ${escapeHtml(release.build)}</span></td>
		<td class="release-date"><time datetime="${escapeHtml(release.publishedAt)}">${formatDate(release.publishedAt)}</time></td>
		<td class="release-downloads"><div class="release-actions"><a class="button button-small" download href="${escapeHtml(release.asset.url)}" aria-label="Download ${escapeHtml(release.asset.name)}">${icon('download')} Download</a><a class="notes-link" href="${escapeHtml(release.releaseUrl)}">Release notes ${icon('external')}</a></div>
			<details class="file-details"><summary>Files &amp; verification ${icon('chevron')}</summary><div class="file-details-content"><span class="asset-name">${escapeHtml(release.asset.name)}</span><span>${formatSize(release.asset.size)}${Number.isFinite(release.asset.downloadCount) ? ` · ${new Intl.NumberFormat('en').format(release.asset.downloadCount)} downloads` : ''}</span><div class="verification-links">${release.checksum ? `<a href="${escapeHtml(release.checksum.url)}">${icon('check')} SHA-256 checksums</a>` : ''}${release.securityReport ? `<a href="${escapeHtml(release.securityReport.url)}">${icon('shield')} Security report</a>` : ''}</div></div></details>
		</td>
	</tr>${renderSecurityRow(release, securityReport, releaseId(release))}`
}

function renderSidebar(releases, repositoryUrl) {
	const versions = new Map()
	for (const release of releases) {
		const entry = versions.get(release.version)
		if (entry) entry.count += 1
		else versions.set(release.version, { release, count: 1 })
	}
	const latestVersion = versions.keys().next().value
	const allVersionsActive = latestVersion === undefined
	const versionLinks = [...versions].map(([version, { release, count }]) => {
		const active = version === latestVersion
		return `<a class="version-option${active ? ' is-active' : ''}" href="#${releaseId(release)}" data-version-link="${escapeHtml(version)}"${active ? ' aria-current="true"' : ''}><span>${escapeHtml(version)}</span><span class="version-count">${count}</span></a>`
	}).join('')
	return `<aside class="sidebar" aria-label="Download navigation"><div class="version-panel panel"><h2>ShroudForge version</h2><nav aria-label="Release versions"><a class="version-option${allVersionsActive ? ' is-active' : ''}" href="#all-versions" data-version-link=""${allVersionsActive ? ' aria-current="true"' : ''}><span>All versions</span><span class="version-count">${releases.length}</span></a>${versionLinks}</nav></div><div class="sidebar-note">${icon('github')}<p>Official releases.<br>Downloaded directly from <a href="${repositoryUrl}">GitHub ${icon('external')}</a>.</p></div></aside>`
}

export function renderPage({ releases, generatedAt, siteUrl, config, securityReports = {} }) {
	const canonicalUrl = new URL('/', siteUrl).href
	const repositoryUrl = `https://github.com/${config.repository}`
	const stable = releases.find((release) => !release.prerelease)
	const development = releases.find((release) => release.prerelease)
	const title = `${config.name} Downloads | Enshrouded Modloader`
	const indexable = new URL(siteUrl).origin === new URL(config.siteUrl).origin
	const structuredData = {
		'@context': 'https://schema.org',
		'@graph': [{
			'@type': 'WebSite',
			'@id': `${canonicalUrl}#website`,
			name: config.name,
			url: canonicalUrl,
			inLanguage: 'en',
		}, {
			'@type': 'SoftwareApplication',
			'@id': `${canonicalUrl}#software`,
			name: config.name,
			url: canonicalUrl,
			description: config.description,
			applicationCategory: 'GameApplication',
			operatingSystem: 'Windows',
			image: new URL('/assets/hammer.png', siteUrl).href,
			...(stable ? { softwareVersion: stable.version, downloadUrl: stable.asset.url, datePublished: stable.publishedAt } : {}),
			isAccessibleForFree: true,
			offers: { '@type': 'Offer', price: 0 },
			sameAs: repositoryUrl,
		}],
	}
	return `<!doctype html>
<html lang="en" data-theme="light">
<head>
	<meta charset="utf-8">
	<meta name="viewport" content="width=device-width, initial-scale=1">
	<title>${escapeHtml(title)}</title>
	<meta name="description" content="${escapeHtml(config.description)}">
	<meta name="theme-color" content="#ffffff">
	<meta name="color-scheme" content="light">
	<meta name="robots" content="${indexable ? 'index, follow, max-image-preview:large' : 'noindex, nofollow'}">
	<link rel="canonical" href="${escapeHtml(canonicalUrl)}">
	<link rel="icon" href="/assets/favicon.ico" type="image/x-icon" sizes="16x16 32x32 48x48">
	<link rel="icon" href="/assets/hammer.png" type="image/png" sizes="256x256">
	<link rel="apple-touch-icon" href="/assets/hammer.png">
	<link rel="preload" href="/assets/inter-latin.woff2" as="font" type="font/woff2" crossorigin>
	<script src="/assets/theme.js"></script>
	<link rel="stylesheet" href="/assets/styles.css">
	<meta property="og:type" content="website">
	<meta property="og:site_name" content="${escapeHtml(config.name)}">
	<meta property="og:title" content="${escapeHtml(title)}">
	<meta property="og:description" content="${escapeHtml(config.description)}">
	<meta property="og:url" content="${escapeHtml(canonicalUrl)}">
	<meta property="og:image" content="${escapeHtml(new URL('/assets/hammer.png', siteUrl).href)}">
	<meta property="og:image:alt" content="ShroudForge hammer emblem">
	<meta property="og:image:type" content="image/png">
	<meta property="og:image:width" content="256">
	<meta property="og:image:height" content="256">
	<meta name="twitter:card" content="summary">
	<meta name="twitter:title" content="${escapeHtml(title)}">
	<meta name="twitter:description" content="${escapeHtml(config.description)}">
	<meta name="twitter:image" content="${escapeHtml(new URL('/assets/hammer.png', siteUrl).href)}">
	<script type="application/ld+json">${JSON.stringify(structuredData).replace(/</g, '\\u003c')}</script>
	<script src="/assets/filters.js" defer></script>
</head>
<body>
	<a class="skip-link" href="#main-content">Skip to downloads</a>
	<header class="site-header"><div class="header-inner"><a class="brand" href="/" aria-label="ShroudForge home"><img src="/assets/hammer.png" alt="" width="48" height="48" fetchpriority="high"><span>ShroudForge</span></a><nav class="primary-navigation" aria-label="Main navigation"><a class="nav-link nav-link-active" href="/" aria-current="page">${icon('file')} Files</a><a class="nav-link" href="${repositoryUrl}">${icon('github')} GitHub ${icon('external', 'nav-external')}</a><a class="nav-link" href="${escapeHtml(config.discordUrl)}">${icon('discord')} Discord ${icon('external', 'nav-external')}</a><a class="nav-link" href="${escapeHtml(config.youtubeUrl)}">${icon('youtube')} YouTube ${icon('external', 'nav-external')}</a></nav></div></header>
	<div class="page-shell">
		${renderSidebar(releases, repositoryUrl)}
		<main id="main-content" class="main-content">
			<section class="page-intro" aria-labelledby="page-title"><div class="eyebrow">FOR ENSHROUDED</div><h1 id="page-title">ShroudForge downloads</h1><p>The free, open-source modloader for Enshrouded. Download the latest release and start making the game your own.</p></section>
			<div class="download-cards">${downloadCard(stable, { title: 'Latest stable', description: 'The recommended release for your next adventure.', kind: 'stable' }, securityReports[stable?.tag])}${downloadCard(development, { title: 'Latest development', description: 'Try the newest changes. Development builds may be less stable.', kind: 'development' }, securityReports[development?.tag])}</div>
			<div class="download-notice">${icon('file')}<p>New to ShroudForge? Download a ZIP above to get started. <a href="https://bonsaibauer.github.io/shroudforge/en/#play">Installation guide ${icon('external')}</a></p></div>
			<section class="archive-section" aria-labelledby="all-versions">
				<div class="archive-heading"><div><h2 id="all-versions">All versions</h2><p>Find a specific release or explore the latest builds.</p></div><span class="total-count">${releases.length} ${releases.length === 1 ? 'release' : 'releases'}</span></div>
				<div class="enhanced-filters" hidden><div class="search-field">${icon('search')}<label class="visually-hidden" for="release-search">Search releases</label><input id="release-search" type="search" placeholder="Search version or build…" autocomplete="off" spellcheck="false"></div><div class="channel-filters" role="group" aria-label="Release channel"><button type="button" class="filter-button is-active" data-channel-filter="all" aria-pressed="true">All</button><button type="button" class="filter-button" data-channel-filter="stable" aria-pressed="false">Stable</button><button type="button" class="filter-button" data-channel-filter="development" aria-pressed="false">Development</button></div></div>
				<div class="filter-status" hidden><p id="filter-result-count" role="status" aria-live="polite" aria-atomic="true"></p><button class="text-button" id="clear-filters" type="button" hidden>Clear filters</button></div>
				<div class="release-table-container panel"><table class="release-table"><caption class="visually-hidden">ShroudForge releases, publication dates, downloads and verification files</caption><thead><tr><th scope="col">Version</th><th scope="col">Released</th><th scope="col">Downloads</th></tr></thead><tbody>${releases.map((release) => releaseRow(release, securityReports[release.tag])).join('')}</tbody></table><div id="no-results" class="empty-state" ${releases.length ? 'hidden' : ''}>${icon('search')}<h3>${releases.length ? 'No matching releases' : 'Releases are on their way'}</h3><p>${releases.length ? 'Try another version, build number or release channel.' : 'Published downloads will appear here automatically.'}</p></div></div>
				<p class="archive-footnote">${icon('github')} Release files and release notes are hosted on <a href="${repositoryUrl}/releases">GitHub ${icon('external')}</a>.</p>
			</section>
		</main>
	</div>
	<footer class="site-footer"><div class="footer-inner"><span class="footer-brand">ShroudForge <span>Modding starts here.</span></span><div class="footer-links"><button class="theme-switch" id="theme-toggle" type="button" role="switch" aria-checked="true" aria-label="Light background" hidden><span>Light background</span><span class="theme-switch-track" aria-hidden="true"><span class="theme-switch-thumb"></span></span></button><a href="${repositoryUrl}">Source code ${icon('external')}</a><a href="${escapeHtml(config.statusUrl)}">Status ${icon('external')}</a><a href="${escapeHtml(config.discordUrl)}">Discord ${icon('external')}</a></div><p class="footer-meta">Release index updated <time datetime="${escapeHtml(generatedAt)}">${formatDate(generatedAt)}</time>. ShroudForge is a community project for Enshrouded.</p></div></footer>
</body>
</html>`
}
