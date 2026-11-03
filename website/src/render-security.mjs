import { escapeHtml } from './html.mjs'
import { icon } from './icons.mjs'

const scannerStatuses = {
	clean: { label: 'No findings reported', icon: 'check', tone: 'clean' },
	findings: { label: 'Findings reported', icon: 'alert', tone: 'findings' },
	warning: { label: 'Warning', icon: 'alert', tone: 'warning' },
	error: { label: 'Scan error', icon: 'alert', tone: 'error' },
	skipped: { label: 'Not scanned', icon: 'minus', tone: 'unknown' },
	unknown: { label: 'Result unknown', icon: 'question', tone: 'unknown' },
}

const reportStatuses = {
	clean: { label: 'No findings reported', icon: 'check', tone: 'clean' },
	findings: { label: 'Findings reported', icon: 'alert', tone: 'findings' },
	warning: { label: 'Warnings reported', icon: 'alert', tone: 'warning' },
	incomplete: { label: 'Incomplete results', icon: 'question', tone: 'unknown' },
}

const reportStates = {
	missing: { label: 'No published security report', icon: 'minus', tone: 'unknown' },
	unavailable: { label: 'Report unavailable', icon: 'question', tone: 'warning' },
	invalid: { label: 'Report verification failed', icon: 'alert', tone: 'error' },
}

function number(value) {
	return Number.isSafeInteger(value) && value >= 0 ? value : null
}

function externalUrl(value) {
	try {
		const url = new URL(value)
		return url.protocol === 'https:' ? url.href : null
	} catch {
		return null
	}
}

function countLabel(value, singular, plural = `${singular}s`) {
	const count = number(value)
	return count === null ? null : `${count.toLocaleString('en')} ${count === 1 ? singular : plural}`
}

function virusTotal(report) {
	return report?.state === 'available' && externalUrl(report.virusTotal?.url)
		? report.virusTotal : null
}

function virusTotalVerdict(stats) {
	const malicious = number(stats?.malicious)
	const suspicious = number(stats?.suspicious)
	return `${malicious === null ? 'Malicious count not reported' : `${malicious} malicious`} · ${suspicious === null ? 'suspicious count not reported' : `${suspicious} suspicious`}`
}

export function renderVirusTotalLink(report, { card = false } = {}) {
	const result = virusTotal(report)
	const url = result ? externalUrl(result.url) : externalUrl(report?.lookupUrl)
	if (!url) return ''
	return `<a class="${card ? 'card-virus-total' : 'security-summary-link'}" href="${escapeHtml(url)}">${icon(result ? 'shield' : 'search')}<span>VirusTotal ${result ? 'report' : 'lookup'}${card && result ? `<span class="card-vt-counts">${escapeHtml(virusTotalVerdict(result.stats))}</span>` : ''}</span>${icon('external')}</a>`
}

function renderScanner(scanner) {
	const status = scannerStatuses[scanner.status] || scannerStatuses.unknown
	const findings = countLabel(scanner.findingCount, 'finding')
	const files = countLabel(scanner.scannedFileCount, 'file')
	const scopeLabels = { source: 'Source tree', package: 'Release package', archive: 'Release ZIP', release: 'Release package' }
	const jobLabels = { success: 'completed', failure: 'failed', cancelled: 'cancelled', skipped: 'skipped', unknown: 'unknown' }
	return `<li class="scanner-card security-tone-${status.tone}" data-security-scanner="${escapeHtml(scanner.id)}"><div class="scanner-heading">${icon(status.icon)}<h4>${escapeHtml(scanner.name || scanner.id)}</h4></div><p class="scanner-scope">${escapeHtml(scopeLabels[scanner.scope] || scanner.scope || 'Scope not reported')}</p><p class="scanner-status">${status.label}</p><div class="scanner-counts">${findings ? `<span>${findings}</span>` : '<span>Finding count not reported</span>'}${files ? `<span>${files} scanned</span>` : ''}</div><p class="scanner-job">Workflow job ${jobLabels[scanner.jobResult] || 'unknown'}</p></li>`
}

function renderVirusTotal(report) {
	const result = virusTotal(report)
	if (!result) return '<div class="security-vt-missing"><p>VirusTotal results are not included in this published report.</p></div>'
	const stats = result.stats || {}
	const errors = [stats.timeout, stats.confirmed_timeout, stats.failure].map(number)
	const groups = [
		{ id: 'malicious', label: 'Malicious', count: number(stats.malicious) },
		{ id: 'suspicious', label: 'Suspicious', count: number(stats.suspicious) },
		{ id: 'undetected', label: 'Undetected', count: number(stats.undetected) },
		{ id: 'harmless', label: 'Harmless', count: number(stats.harmless) },
		{ id: 'unsupported', label: 'Unsupported', count: number(stats.type_unsupported) },
		{ id: 'errors', label: 'Timeouts / errors', count: errors.every((value) => value !== null) ? errors.reduce((sum, value) => sum + value, 0) : null },
		{ id: 'other', label: 'Other', count: number(stats.other) },
	]
	const total = groups.reduce((sum, group) => sum + (group.count ?? 0), 0)
	const chartDescription = groups.map((group) => `${group.label}: ${group.count ?? 'not reported'}`).join(', ')
	let offset = 0
	const segments = groups.filter((group) => group.count > 0).map((group) => {
		const width = group.count / total * 1000
		const segment = `<rect class="vt-segment-${group.id}" x="${offset.toFixed(3)}" y="0" width="${width.toFixed(3)}" height="18"/>`
		offset += width
		return segment
	}).join('')
	const date = result.analysisDate && Number.isFinite(Date.parse(result.analysisDate))
		? new Intl.DateTimeFormat('en-GB', { day: '2-digit', month: 'short', year: 'numeric', timeZone: 'UTC' }).format(new Date(result.analysisDate)) : null
	return `<section class="vt-panel" aria-label="VirusTotal engine results"><div class="vt-heading"><div><h4>VirusTotal</h4><p class="vt-verdict">${escapeHtml(virusTotalVerdict(stats))}</p></div><a class="vt-report-link" href="${escapeHtml(result.url)}">Open report ${icon('external')}</a></div><svg class="vt-chart" viewBox="0 0 1000 18" preserveAspectRatio="none" role="img" aria-label="${escapeHtml(chartDescription)}"><rect class="vt-chart-track" x="0" y="0" width="1000" height="18" rx="8"/>${segments}</svg><ul class="vt-legend">${groups.map((group) => `<li><span class="vt-legend-dot vt-segment-${group.id}" aria-hidden="true"></span><span>${group.label}</span><strong>${group.count === null ? 'Not reported' : group.count.toLocaleString('en')}</strong></li>`).join('')}</ul><p class="vt-context">${total ? `${total.toLocaleString('en')} engine results reported.` : 'No engine results reported.'} Undetected means an engine reported no detection. Unsupported files, timeouts and errors are separate results.${date ? ` Analysis dated <time datetime="${escapeHtml(result.analysisDate)}">${date}</time>.` : ''}</p></section>`
}

function renderEvidence(release, report) {
	const reportUrl = externalUrl(release.securityReport?.url)
	const workflowUrl = externalUrl(report?.workflowUrl)
	const hashes = [
		['Release ZIP SHA-256', report?.archiveSha256],
		['Security report ZIP SHA-256', report?.bundleSha256],
	].filter(([, value]) => typeof value === 'string' && /^[a-f0-9]{64}$/i.test(value))
	return `<div class="security-evidence"><div class="security-evidence-links">${reportUrl ? `<a href="${escapeHtml(reportUrl)}">${icon('download')} Download report ZIP</a>` : ''}${workflowUrl ? `<a href="${escapeHtml(workflowUrl)}">${icon('github')} View scan workflow ${icon('external')}</a>` : ''}</div>${hashes.length ? `<details class="security-hashes"><summary>File fingerprints ${icon('chevron')}</summary><dl>${hashes.map(([label, value]) => `<div><dt>${label}</dt><dd><code>${escapeHtml(value)}</code></dd></div>`).join('')}</dl></details>` : ''}</div>`
}

export function renderSecurityRow(release, report, rowId) {
	const available = report?.state === 'available'
	const status = available ? (reportStatuses[report.status] || reportStatuses.incomplete) : (reportStates[report?.state] || reportStates.missing)
	const scanners = available && Array.isArray(report.scanners) ? report.scanners : []
	const vtLink = renderVirusTotalLink(report)
	return `<tr class="security-row" data-security-release="${escapeHtml(release.tag)}" data-security-for="${escapeHtml(rowId)}"><td colspan="3"><div class="security-line${vtLink ? ' has-virus-total' : ''}"><details class="security-details"><summary class="security-summary"><span class="security-summary-title">${icon('shield')} Security overview ${icon('chevron')}</span><span class="security-summary-status security-tone-${status.tone}">${icon(status.icon)} ${status.label}</span></summary><div class="security-content"><div class="security-content-heading"><h3>Published security results</h3><p>${escapeHtml(report?.message || (available ? 'Results from the security report published with this release.' : status.label))}</p></div>${available ? `${scanners.length ? `<ul class="scanner-grid">${scanners.map(renderScanner).join('')}</ul>` : '<p class="security-no-scanners">No scanner results are included in this report.</p>'}${renderVirusTotal(report)}` : `<p class="security-scope-note">No verified scan results are available to display for this release.${externalUrl(report?.lookupUrl) ? ' A VirusTotal lookup searches for this file and does not establish that it has been scanned.' : ''}</p>`}${renderEvidence(release, report)}</div></details>${vtLink}</div></td></tr>`
}
