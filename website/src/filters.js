(() => {
	const rows = [...document.querySelectorAll('[data-release]')]
	const securityRows = new Map([...document.querySelectorAll('[data-security-for]')].map((row) => [row.dataset.securityFor, row]))
	const search = document.querySelector('#release-search')
	const channelButtons = [...document.querySelectorAll('[data-channel-filter]')]
	const versionLinks = [...document.querySelectorAll('[data-version-link]')]
	const resultCount = document.querySelector('#filter-result-count')
	const clearButton = document.querySelector('#clear-filters')
	const noResults = document.querySelector('#no-results')
	if (!search || !resultCount || !clearButton || !noResults || !rows.length) return

	let channel = 'all'
	const defaultVersion = versionLinks.find((link) => link.dataset.versionLink)?.dataset.versionLink || ''
	let version = defaultVersion

	function applyFilters() {
		const terms = search.value.trim().toLowerCase().split(/\s+/).filter(Boolean)
		let visible = 0
		for (const row of rows) {
			const matches = (channel === 'all' || row.dataset.channel === channel)
				&& (!version || row.dataset.version === version)
				&& terms.every((term) => row.dataset.search.includes(term))
			row.hidden = !matches
			const securityRow = securityRows.get(row.id)
			if (securityRow) securityRow.hidden = !matches
			if (matches) visible += 1
		}
		for (const button of channelButtons) {
			const active = button.dataset.channelFilter === channel
			button.classList.toggle('is-active', active)
			button.setAttribute('aria-pressed', String(active))
		}
		for (const link of versionLinks) {
			const active = link.dataset.versionLink === version
			link.classList.toggle('is-active', active)
			if (active) link.setAttribute('aria-current', 'true')
			else link.removeAttribute('aria-current')
		}
		resultCount.textContent = `Showing ${visible} of ${rows.length} releases${version ? ` · Version ${version}` : ''}`
		clearButton.hidden = !terms.length && channel === 'all' && version === defaultVersion
		noResults.hidden = visible !== 0
	}

	search.addEventListener('input', applyFilters)
	for (const button of channelButtons) {
		button.addEventListener('click', () => {
			channel = button.dataset.channelFilter
			applyFilters()
		})
	}
	for (const link of versionLinks) {
		link.addEventListener('click', (event) => {
			if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return
			event.preventDefault()
			version = link.dataset.versionLink
			applyFilters()
			document.querySelector('#all-versions').scrollIntoView({ block: 'start' })
		})
	}
	clearButton.addEventListener('click', () => {
		channel = 'all'
		version = defaultVersion
		search.value = ''
		applyFilters()
	})
	for (const control of document.querySelectorAll('.enhanced-filters, .filter-status')) control.hidden = false
	applyFilters()
})()
