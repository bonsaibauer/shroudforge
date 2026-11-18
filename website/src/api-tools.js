(() => {
	const root = document.querySelector('[data-api-tool]')
	if (!root) return

	const text = {
		en: {
			skip: 'Skip to API tools', toolsNavLabel: 'Tools', moddingNavLabel: 'Modding', modSettingsNav: 'Mod-Settings', apiCallsNav: 'API calls', browseApiNav: 'Browse API', standardsNavLabel: 'Standards',
			toolsEyebrow: 'SHROUDFORGE TOOLS · MODDING', apiReferenceTitle: 'Browse the API', apiReferenceLead: 'Start with a task or search the published API, game types, and resource IDs.', apiPlaygroundTitle: 'Try API calls', apiPlaygroundLead: 'Choose a task, edit its inputs, and preview sample output.', tryCallsAction: 'Try API calls', browseApiAction: 'Browse API', apiVersion: 'ShroudForge API version', gameBuildLabel: 'Game Build', apiCategories: 'API categories', apiReferenceSections: 'API reference sections',
			referenceNotesTitle: 'Status and evidence', referenceNotesAction: 'Show details', referenceNotesLua: 'Lua API means the entry is part of the shared Lua interface used by mods. It is generated from the published API declarations. This badge does not confirm availability in the current game process.', referenceNotesGame: 'Mod permission shows the capability checked by the Modloader. Runtime declared means the operation ID is published for runtime checks. Neither badge confirms live availability.', referenceNotesRuntime: 'Only runtime.status or runtime.has inside the running game can report live availability. No per-feature gameplay-effect verification is published here. Game types and resource IDs come from the captured game build, not live game objects.',
			startWithTask: 'START WITH A TASK', commonTasks: 'What do you want your mod to do?', taskLead: 'Choose an example to see its Lua call and what the response means.', loadingExamples: 'Loading examples…',
		referenceEyebrow: 'SEARCHABLE REFERENCE', allApiEntries: 'Find an API entry', searchLead: 'Search by task, function, or parameter. Badges describe published evidence, not live availability.', searchLabel: 'Search the API', searchPlaceholder: 'Try “read a setting” or shroudforge.settings.get', clearSearch: 'Clear', filterAll: 'Everything', filterRuntime: 'Runtime', filterGame: 'Game data', functionsTab: 'Lua functions', typesTab: 'Game types', resourcesTab: 'Game resources', loadingCatalog: 'Loading the API catalogue…', typeDataNote: 'These details come from a captured Enshrouded build. They do not describe a live game process.', searchTypes: 'Search game types', typeSearchPlaceholder: 'Type name or field', resourceDataNote: 'Resource names and IDs come from the same captured game snapshot.', searchResources: 'Search game resources', resourceSearchPlaceholder: 'Resource name or ID', referenceFooter: 'The catalogue documents the Lua interface. Check runtime.has or runtime.status inside your mod to learn what is available in the current game process.',
			apiCount: '{count} API entries', apiCountFiltered: '{count} matching API entries', noApiResults: 'No API entries match this search. Try a shorter task or function name.', itemsPerPage: 'Items per page', previousPage: 'Previous page', nextPage: 'Next page', pageIndicator: 'Page {current} of {total}', paginationLabel: 'Search result pages', apiBadge: 'Lua API', operationBadge: 'Runtime declared', capabilityBadge: 'Requires {capability} capability', signatureLabel: 'Lua call', descriptionLabel: 'What it does', parametersLabel: 'Inputs', returnsLabel: 'Returns', errorsLabel: 'Errors', exampleLabel: 'Example', sourceLabel: 'Declaration source', noParameters: 'No named inputs are listed.', noReturn: 'No return value is listed.', noExample: 'No example is included in this declaration.', tryExample: 'Open a matching example',
		typesLoading: 'Loading captured game types…', resourcesLoading: 'Loading captured game resources…', typeSearchPrompt: 'Enter a type name or field to search the captured catalogue.', resourceSearchPrompt: 'Enter a resource type, GUID, or part number to search.', noTypeResults: 'No captured types match that search.', noResourceResults: 'No captured resources match that search.', typeCount: '{count} game types', resourceCount: '{count} resource types', fieldsCount: '{count} fields', primitiveLabel: 'Kind', sizeLabel: 'Size', alignmentLabel: 'Alignment', fieldsLabel: 'Fields', moreFields: '{count} more fields', enumValues: 'Enum values', resourceEntries: '{count} captured entries', partLabel: 'Part', snapshotLabel: 'Captured snapshot',
		exampleEditor: 'API CALL EDITOR', editRunExample: 'Build a Lua API call', playgroundLead: 'Choose a task, edit its inputs, and preview sample output. Lua never runs in this browser.', chooseExample: 'Choose a task', loadingExamplesOption: 'Loading examples…', generatedLua: 'Lua code for your mod', copyCode: 'Copy', runExample: 'Show sample response', exampleResponse: 'SAMPLE RESPONSE', responseHeading: 'What the call may return', sampleOnly: 'Browser sample', runToSeeResponse: 'Choose a task and show its sample response.', noResponseYet: 'No response yet.', responseSource: 'Sample source', sourceChanges: 'The sample source will appear here after you request a response.', viewApiEntry: 'View API entry →', simulationHeading: 'How sample responses work', simulationBody: 'The browser generates a Lua example and reads bundled sample data. It does not execute Lua, query a server, or connect to your running game.', playgroundFooter: 'Copy the generated Lua into your mod. Check live runtime availability in game with runtime.has or runtime.status.',
			fieldKey: 'Setting key', fieldFallback: 'Fallback value', fieldFeature: 'Runtime feature', fieldCapability: 'Capability', fieldComponent: 'Component', fieldField: 'Player field', fieldSetting: 'Setting key', fieldTypeName: 'Qualified type name', booleanTrue: 'true', booleanFalse: 'false', scenarioReadSurvivalSetting: 'Read an SF Survival setting',
			settingSource: 'Starter Mod example: extended.mod.json settings', runtimeSource: 'Simulated status fixture. The browser cannot inspect the current game process.', typeSource: 'Captured Enshrouded game-data snapshot', responseSetting: 'This value is read from the example mod settings included with ShroudForge.', responseRuntime: 'This is a sample response shape only. The website did not query a running game.', responseType: 'This result is a view of captured game metadata, not a live object from the game.', settingNotFound: 'The example settings do not contain this key, so the fallback value is returned.', typeNotFound: 'This type name does not exist in the captured snapshot.', runtimeReason: 'This website has no connection to the current game process.', inputError: 'Enter a valid value for each input.',
			copySuccess: 'Lua call copied.', copyFailure: 'Copy is unavailable in this browser.', loadFailure: 'The published API files could not be loaded.', loadFailureHelp: 'Check the request details below. If you opened this page as a local file, use the website or its local preview server.', retryLoading: 'Retry', versionsUnavailableOption: 'API versions unavailable', examplesUnavailableOption: 'Examples unavailable', gameBuildUnavailable: 'Game build information is unavailable.', runtimeCapability: 'runtime', patchCapability: 'patch', exportCapability: 'export',
		},
	}

	const escapeHtml = value => String(value ?? '').replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char])
	const locale = 'en'
	const state = { index: null, catalog: null, examples: null, operations: null, gameBuildIndex: null, selectedGameBuild: null, types: null, resources: null, filter: 'all', pageSize: 30, pages: { functions: 1, types: 1, resources: 1 }, versionRequest: 0 }
	const apiPageSizeOptions = [10, 20, 30, 50]
	const gameDataCache = { types: new Map(), resources: new Map() }
	const gameDataLoads = { types: new Map(), resources: new Map() }
	let gameBuildLoadPromise
	const translate = (key, values = {}) => {
		if (!key) return ''
		let value = text.en[key] ?? key
		for (const [name, replacement] of Object.entries(values)) value = value.replaceAll(`{${name}}`, String(replacement))
		return value
	}
	const $ = selector => root.querySelector(selector)
	const $$ = selector => [...root.querySelectorAll(selector)]
	const mainRoute = root.dataset.apiTool === 'api-playground' ? 'playground' : 'reference'

	function updateQuery(values) {
		const params = new URLSearchParams(location.search)
		for (const [key, value] of Object.entries(values)) {
			if (value === null || value === undefined || value === '') params.delete(key)
			else params.set(key, value)
		}
		const next = `${location.pathname}${params.size ? `?${params}` : ''}${location.hash}`
		history.replaceState(null, '', next)
	}

	function applyLocale() {
		document.documentElement.lang = locale
		for (const node of $$('[data-i18n]')) node.textContent = translate(node.dataset.i18n)
		for (const node of $$('[data-i18n-placeholder]')) node.placeholder = translate(node.dataset.i18nPlaceholder)
		for (const node of $$('[data-i18n-aria-label]')) node.setAttribute('aria-label', translate(node.dataset.i18nAriaLabel))
	}

	async function fetchJson(url) {
		const requestUrl = new URL(url, document.baseURI)
		if (requestUrl.protocol === 'file:') throw new Error(`Cannot load published API data from a file URL. Open this page through the website or its local preview server: ${requestUrl.href}`)
		let response
		try { response = await fetch(requestUrl, { headers: { Accept: 'application/json' } }) }
		catch (error) { throw new Error(`Network request failed for ${requestUrl.href}${error?.message ? ` (${error.message})` : ''}`, { cause: error }) }
		if (!response.ok) throw new Error(`HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ''} while loading ${requestUrl.href}`)
		const contentType = response.headers.get('content-type') || 'unknown content type'
		try { return await response.json() }
		catch (error) { throw new Error(`Invalid JSON from ${requestUrl.href} (${contentType})`, { cause: error }) }
	}

	function clearApiLoadError() {
		for (const container of $$('[data-api-load-error]')) {
			container.hidden = true
			const detail = container.querySelector('[data-api-load-error-detail]')
			if (detail) detail.textContent = ''
		}
	}

	function showApiLoadError(error) {
		console.error('[ShroudForge API tools] Could not load the published API data.', error)
		for (const container of $$('[data-api-load-error]')) {
			const message = container.querySelector('[data-api-load-error-message]')
			if (message) message.textContent = translate('loadFailure')
			const detail = container.querySelector('[data-api-load-error-detail]')
			if (detail) detail.textContent = `${translate('loadFailureHelp')} ${error?.message || 'Unknown loading error.'}`
			container.hidden = false
		}
		if (!state.catalog) {
			const results = $('[data-api-results]')
			if (results) results.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
			const tasks = $('[data-api-tasks]')
			if (tasks) tasks.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
		}
	}

	function versionRoute(route, extra = {}) {
		const params = new URLSearchParams({ apiVersion: state.catalog?.version || state.index?.current || '' })
		if (state.selectedGameBuild?.gameBuild) params.set('gameBuild', state.selectedGameBuild.gameBuild)
		for (const [key, value] of Object.entries(extra)) if (value) params.set(key, value)
		return `${route}?${params}`
	}

	function updateNavigationRoutes() {
		for (const link of $$('[data-api-route]')) link.href = versionRoute(link.dataset.apiRoute)
	}

	function updatePlaygroundReferenceLink() {
		const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
		const reference = $('[data-reference-link]')
		if (scenario && reference) reference.href = versionRoute('/tools/modding/api-reference/', { q: scenario.method })
	}

	function renderVersionPicker() {
		const picker = $('[data-api-version]')
		if (!picker || !state.index) return
		picker.innerHTML = state.index.versions.map(entry => {
			return `<option value="${escapeHtml(entry.version)}">${escapeHtml(entry.version)}</option>`
		}).join('')
		picker.disabled = false
		picker.value = state.catalog?.version || state.index.current
	}

	async function selectVersion(version) {
		const request = ++state.versionRequest
		const selected = state.index.versions.find(item => item.version === version) || state.index.versions.find(item => item.version === state.index.current) || state.index.versions[0]
		if (!selected) throw new Error('API version index is empty')
		const base = `/data/api/${encodeURIComponent(selected.version)}`
		let catalog, examples, operations
		try {
			[catalog, examples, operations] = await Promise.all([
				fetchJson(`${base}/catalog.json`), fetchJson(`${base}/examples.json`), fetchJson(`${base}/runtime-operations.json`),
			])
		} catch (error) {
			if (request !== state.versionRequest) return
			throw error
		}
		if (request !== state.versionRequest) return
		if (catalog.version !== selected.version || !Array.isArray(catalog.symbols)) throw new Error('Invalid API catalogue')
		if (!Array.isArray(examples.scenarios) || !Array.isArray(operations.runtimeOperations)) throw new Error('Invalid API example data')
		state.catalog = catalog
		state.examples = examples
		state.operations = new Set(operations.runtimeOperations)
		state.pages.functions = 1
		updateQuery({ apiVersion: selected.version })
		renderVersionPicker()
		updateNavigationRoutes()
		renderTasks()
		renderApiResults()
		if (mainRoute === 'playground') renderPlayground(getScenarioFromQuery())
		clearApiLoadError()
	}

	function renderGameBuildPicker() {
		const picker = $('[data-game-build]')
		if (!picker || !state.gameBuildIndex) return
		picker.innerHTML = state.gameBuildIndex.builds.map(entry => `<option value="${escapeHtml(entry.gameBuild)}">${escapeHtml(entry.gameBuild)}</option>`).join('')
		picker.disabled = false
		picker.value = state.selectedGameBuild?.gameBuild || ''
	}

	function selectGameBuild(gameBuild) {
		const selected = state.gameBuildIndex.builds.find(entry => entry.gameBuild === gameBuild)
			|| state.gameBuildIndex.builds.find(entry => entry.snapshot === state.gameBuildIndex.current)
			|| state.gameBuildIndex.builds[0]
		if (!selected) throw new Error('Game build index is empty')
		state.selectedGameBuild = selected
		state.types = gameDataCache.types.get(selected.snapshot) || null
		state.resources = gameDataCache.resources.get(selected.snapshot) || null
		state.pages.types = 1
		state.pages.resources = 1
		renderPagination('types', 0)
		renderPagination('resources', 0)
		renderGameBuildPicker()
		updateQuery({ gameBuild: selected.gameBuild })
		updateNavigationRoutes()
		renderTasks()
		updatePlaygroundReferenceLink()
		const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
		if (mainRoute === 'playground' && scenario?.handler === 'game-type') {
			setResponse(translate('noResponseYet'), translate('sourceChanges'), translate('runToSeeResponse'))
		}
		for (const name of ['types', 'resources']) {
			const panel = $(`[data-api-panel="${name}"]`)
			const target = $(`[data-${name === 'types' ? 'type' : 'resource'}-results]`)
			if (!panel || panel.hidden || !target) continue
			if (state[name]) {
				if (name === 'types') renderTypes()
				else renderResources()
			} else {
				target.innerHTML = `<p class="api-loading" role="status">${escapeHtml(translate(name === 'types' ? 'typesLoading' : 'resourcesLoading'))}</p>`
				loadGameData(name)
			}
		}
	}

	async function loadGameBuilds() {
		const index = await fetchJson('/data/game/builds.json')
		if (!Array.isArray(index.builds) || !index.builds.length || !index.builds.every(entry => typeof entry.snapshot === 'string' && typeof entry.gameBuild === 'string')) {
			throw new Error('Invalid game build index')
		}
		if (!index.builds.some(entry => entry.snapshot === index.current)) throw new Error('Game build index does not contain its current snapshot')
		state.gameBuildIndex = index
		const requested = new URLSearchParams(location.search).get('gameBuild')
		const selected = index.builds.find(entry => entry.gameBuild === requested)
		selectGameBuild(selected?.gameBuild || index.builds.find(entry => entry.snapshot === index.current).gameBuild)
	}

	function scenarioTitle(scenario) {
		const names = { 'read-mod-setting': 'scenarioReadSurvivalSetting' }
		return names[scenario.id] ? translate(names[scenario.id]) : (scenario.title?.[locale] || scenario.title?.en || scenario.method)
	}

	function renderTasks() {
		const target = $('[data-api-tasks]')
		if (!target || !state.examples) return
		target.innerHTML = state.examples.scenarios.map(scenario => `<a class="api-task-card" href="${escapeHtml(`${versionRoute('/tools/modding/api-playground/', { scenario: scenario.id })}#playground-heading`)}"><span class="api-task-mark" aria-hidden="true">↗</span><span class="api-task-copy"><strong>${escapeHtml(scenarioTitle(scenario))}</strong><small>${escapeHtml(scenario.summary?.[locale] || scenario.summary?.en || '')}</small><code>${escapeHtml(scenario.method)}</code></span></a>`).join('')
	}

	function htmlInline(value) {
		const codeParts = []
		let html = escapeHtml(value).replace(/`([^`]+)`/g, (_, content) => {
			const token = `\u0000INLINE_CODE_${codeParts.length}\u0000`
			codeParts.push(`<code>${content}</code>`)
			return token
		})
		html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/\n/g, '<br>')
		return html.replace(/\u0000INLINE_CODE_(\d+)\u0000/g, (_, index) => codeParts[Number(index)])
	}

	function sourceCategory(symbol) {
		return /^shroudforge\//i.test(symbol.source || '') ? 'shroudforge' : 'eml'
	}

	function apiKind(symbol) {
		if (/^runtime(?:\.|$)/.test(symbol.name)) return 'runtime'
		if (/^game(?:\.|$)/.test(symbol.name)) return 'game'
		return sourceCategory(symbol)
	}

	function capabilityFor(symbol) {
		if (/^runtime(?:\.|$)/.test(symbol.name)) return 'runtime'
		if (symbol.name === 'game.assets.write') return 'patch'
		if (symbol.name === 'export') return 'export'
		return null
	}

	function statusBadges(symbol) {
		const badges = [`<span class="api-entry-badge is-api">${escapeHtml(translate('apiBadge'))}</span>`]
		const capability = capabilityFor(symbol)
		if (capability) badges.push(`<span class="api-entry-badge is-capability">${escapeHtml(translate('capabilityBadge', { capability: translate(`${capability}Capability`) }))}</span>`)
		if (state.operations?.has(symbol.name)) badges.push(`<span class="api-entry-badge is-declared">${escapeHtml(translate('operationBadge'))}</span>`)
		return badges.join('')
	}

	function renderParamList(params, key, emptyKey) {
		if (!Array.isArray(params) || !params.length) return `<p class="api-detail-muted">${escapeHtml(translate(emptyKey))}</p>`
		return `<dl class="api-param-list">${params.map(param => `<div><dt><code>${escapeHtml(param.name || '')}</code><span>${escapeHtml(param.type || '')}</span></dt><dd>${htmlInline(param.description || '')}</dd></div>`).join('')}</dl>`
	}

	function renderSymbol(symbol) {
		const signature = symbol.signature || symbol.name
		const category = sourceCategory(symbol)
		const tryLink = state.examples?.scenarios.find(scenario => scenario.method === symbol.name)
		const returns = Array.isArray(symbol.returnDetails) && symbol.returnDetails.length
			? renderParamList(symbol.returnDetails.map(item => ({ ...item, name: item.name || 'value' })), 'returns', 'noReturn')
			: `<p class="api-return-items">${(symbol.returns || []).map(item => `<code>${escapeHtml(item)}</code>`).join(', ') || escapeHtml(translate('noReturn'))}</p>`
		return `<details class="api-entry-card" data-api-entry data-category="${category}">
			<summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(signature)}</code><span class="api-entry-summary-description">${htmlInline(symbol.description || '')}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary>
			<div class="api-entry-detail">
			<div class="api-entry-badges">${statusBadges(symbol)}<span class="api-entry-badge is-source">${category === 'shroudforge' ? 'ShroudForge' : 'EML'}</span></div>
				<div class="api-entry-block"><h3>${escapeHtml(translate('descriptionLabel'))}</h3><p>${htmlInline(symbol.description || '')}</p></div>
				<div class="api-entry-block"><h3>${escapeHtml(translate('signatureLabel'))}</h3><pre class="api-code-panel"><code>${escapeHtml(signature)}</code></pre></div>
				<div class="api-entry-columns"><section class="api-entry-block"><h3>${escapeHtml(translate('parametersLabel'))}</h3>${renderParamList(symbol.params, 'params', 'noParameters')}</section><section class="api-entry-block"><h3>${escapeHtml(translate('returnsLabel'))}</h3>${returns}</section></div>
				${symbol.errors ? `<div class="api-entry-block"><h3>${escapeHtml(translate('errorsLabel'))}</h3><p>${htmlInline(symbol.errors)}</p></div>` : ''}
				${symbol.example ? `<div class="api-entry-block"><h3>${escapeHtml(translate('exampleLabel'))}</h3><pre class="api-code-panel"><code>${escapeHtml(symbol.example)}</code></pre></div>` : `<p class="api-detail-muted">${escapeHtml(translate('noExample'))}</p>`}
				<p class="api-source-path"><span>${escapeHtml(translate('sourceLabel'))}:</span> <code>${escapeHtml(symbol.source || '—')}</code></p>
				${tryLink ? `<a class="api-reference-link" href="${escapeHtml(versionRoute('/tools/modding/api-playground/', { scenario: tryLink.id }))}">${escapeHtml(translate('tryExample'))} →</a>` : ''}
			</div>
		</details>`
	}

	function renderPagination(name, total) {
		const target = $(`[data-api-pagination="${name}"]`)
		if (!target) return
		if (!total) {
			target.hidden = true
			target.innerHTML = ''
			state.pages[name] = 1
			return
		}
		const totalPages = Math.max(1, Math.ceil(total / state.pageSize))
		const currentPage = Math.min(state.pages[name], totalPages)
		state.pages[name] = currentPage
		target.hidden = false
		target.dataset.totalPages = String(totalPages)
		target.innerHTML = `<label class="api-page-size"><span>${escapeHtml(translate('itemsPerPage'))}</span><select data-api-page-size aria-label="${escapeHtml(translate('itemsPerPage'))}">${apiPageSizeOptions.map(option => `<option value="${option}" ${option === state.pageSize ? 'selected' : ''}>${option}</option>`).join('')}</select></label><div class="api-page-controls"><button type="button" class="button button-secondary" data-api-page-list="${name}" data-api-page-step="-1" aria-label="${escapeHtml(translate('previousPage'))}"${currentPage <= 1 ? ' disabled' : ''}>‹</button><span aria-live="polite">${escapeHtml(translate('pageIndicator', { current: currentPage, total: totalPages }))}</span><button type="button" class="button button-secondary" data-api-page-list="${name}" data-api-page-step="1" aria-label="${escapeHtml(translate('nextPage'))}"${currentPage >= totalPages ? ' disabled' : ''}>›</button></div>`
	}

	function pageResults(name, matches) {
		renderPagination(name, matches.length)
		const start = (state.pages[name] - 1) * state.pageSize
		return matches.slice(start, start + state.pageSize)
	}

	function renderApiResults() {
		const target = $('[data-api-results]')
		if (!target || !state.catalog) return
		const search = $('[data-api-search]')
		const needle = (search?.value || '').trim().toLocaleLowerCase(locale)
		const filter = state.filter
		const matches = state.catalog.symbols.filter(symbol => {
			const source = sourceCategory(symbol)
			const kind = apiKind(symbol)
			if (filter !== 'all' && source !== filter && kind !== filter) return false
			if (!needle) return true
			const relatedTasks = (state.examples?.scenarios || []).filter(scenario => scenario.method === symbol.name).flatMap(scenario => [scenario.title?.en, scenario.summary?.en])
			const searchText = [symbol.name, symbol.signature, symbol.description, symbol.source, ...(symbol.params || []).map(param => `${param.name} ${param.type} ${param.description}`), ...(symbol.returns || []), ...relatedTasks].join(' ').toLocaleLowerCase(locale)
			return needle.split(/\s+/).filter(Boolean).every(term => searchText.includes(term))
		})
		const count = $('[data-api-result-count]')
		if (count) count.textContent = translate(needle || filter !== 'all' ? 'apiCountFiltered' : 'apiCount', { count: matches.length })
		const shown = pageResults('functions', matches)
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noApiResults'))}</p>`
			return
		}
		target.innerHTML = shown.map(renderSymbol).join('')
	}

	async function loadGameData(name) {
		if (!state.selectedGameBuild && gameBuildLoadPromise) await gameBuildLoadPromise
		const snapshot = state.selectedGameBuild?.snapshot
		if (!snapshot) return
		const target = $(`[data-${name === 'types' ? 'type' : 'resource'}-results]`)
		if (gameDataCache[name].has(snapshot)) {
			state[name] = gameDataCache[name].get(snapshot)
			if (name === 'types') renderTypes()
			else renderResources()
			return
		}
		let load = gameDataLoads[name].get(snapshot)
		if (!load) {
			load = fetchJson(`/data/game/${encodeURIComponent(snapshot)}/${name}.json`)
				.then(data => { gameDataCache[name].set(snapshot, data); return data })
				.finally(() => gameDataLoads[name].delete(snapshot))
			gameDataLoads[name].set(snapshot, load)
		}
		try {
			const data = await load
			if (state.selectedGameBuild?.snapshot !== snapshot) return loadGameData(name)
			state[name] = data
			if (name === 'types') renderTypes()
			else renderResources()
		} catch {
			if (state.selectedGameBuild?.snapshot !== snapshot) return loadGameData(name)
			if (state.selectedGameBuild?.snapshot === snapshot && target) target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
		}
	}

	function loadTypes() {
		return loadGameData('types')
	}

	function loadResources() {
		return loadGameData('resources')
	}

	function renderTypes() {
		const target = $('[data-type-results]')
		if (!target || !state.types) return
		const queryText = ($('[data-type-search]')?.value || '').trim().toLocaleLowerCase(locale)
		if (!queryText) {
			renderPagination('types', 0)
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('typeSearchPrompt'))}</p>`
			return
		}
		const keys = Object.keys(state.types)
		const matches = keys.filter(key => {
			const item = state.types[key]
			const fields = Object.values(item.fields || {}).map(field => `${field.name} ${field.type_name}`)
			return `${key} ${item.primitive || ''} ${item.name || ''} ${fields.join(' ')}`.toLocaleLowerCase(locale).includes(queryText)
		}).sort((a, b) => (a.toLocaleLowerCase().startsWith(queryText) ? -1 : 0) - (b.toLocaleLowerCase().startsWith(queryText) ? -1 : 0) || a.localeCompare(b))
		const shown = pageResults('types', matches)
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noTypeResults'))}</p>`
			return
		}
		target.innerHTML = shown.map(key => {
			const item = state.types[key]
			const fields = Object.values(item.fields || {}).sort((a, b) => a.name.localeCompare(b.name))
			const visibleFields = fields.slice(0, 35)
			const enumValues = Object.entries(item.enum_values || {})
			return `<details class="api-entry-card api-type-card"><summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(key)}</code><span class="api-entry-summary-description">${escapeHtml(item.primitive || 'Type')} · ${escapeHtml(translate('fieldsCount', { count: item.field_count ?? fields.length }))}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary><div class="api-entry-detail"><div class="api-entry-badges"><span class="api-entry-badge is-snapshot">${escapeHtml(translate('snapshotLabel'))}</span><span class="api-entry-badge is-source">${escapeHtml(item.primitive || 'Type')}</span></div><dl class="api-type-meta"><div><dt>${escapeHtml(translate('primitiveLabel'))}</dt><dd>${escapeHtml(item.primitive || '—')}</dd></div><div><dt>${escapeHtml(translate('sizeLabel'))}</dt><dd>${escapeHtml(item.size ?? '—')} bytes</dd></div><div><dt>${escapeHtml(translate('alignmentLabel'))}</dt><dd>${escapeHtml(item.alignment ?? '—')}</dd></div></dl>${fields.length ? `<h3>${escapeHtml(translate('fieldsLabel'))}</h3><div class="api-field-list">${visibleFields.map(field => `<div><code>${escapeHtml(field.name)}</code><code>${escapeHtml(field.type_name)}</code><span>${escapeHtml(field.data_offset ?? '—')}</span></div>`).join('')}</div>${fields.length > visibleFields.length ? `<p class="api-detail-muted">${escapeHtml(translate('moreFields', { count: fields.length - visibleFields.length }))}</p>` : ''}` : ''}${enumValues.length ? `<h3>${escapeHtml(translate('enumValues'))}</h3><div class="api-field-list">${enumValues.slice(0, 40).map(([name, value]) => `<div><code>${escapeHtml(name)}</code><code>${escapeHtml(value)}</code></div>`).join('')}</div>` : ''}</div></details>`
		}).join('')
	}

	function renderResources() {
		const target = $('[data-resource-results]')
		if (!target || !state.resources) return
		const queryText = ($('[data-resource-search]')?.value || '').trim().toLocaleLowerCase(locale)
		if (!queryText) {
			renderPagination('resources', 0)
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('resourceSearchPrompt'))}</p>`
			return
		}
		const matches = Object.entries(state.resources).filter(([name, entries]) => `${name} ${entries.map(entry => `${entry.guid} ${entry.part}`).join(' ')}`.toLocaleLowerCase(locale).includes(queryText)).sort((a, b) => a[0].localeCompare(b[0]))
		const shown = pageResults('resources', matches)
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noResourceResults'))}</p>`
			return
		}
		target.innerHTML = shown.map(([name, entries]) => `<details class="api-entry-card"><summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(name)}</code><span class="api-entry-summary-description">${escapeHtml(translate('resourceEntries', { count: entries.length }))}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary><div class="api-entry-detail"><div class="api-entry-badges"><span class="api-entry-badge is-snapshot">${escapeHtml(translate('snapshotLabel'))}</span></div><div class="api-resource-list">${entries.slice(0, 25).map(entry => `<div><code>${escapeHtml(entry.guid)}</code><span>${escapeHtml(translate('partLabel'))} ${escapeHtml(entry.part ?? 0)}</span></div>`).join('')}</div>${entries.length > 25 ? `<p class="api-detail-muted">${escapeHtml(translate('moreFields', { count: entries.length - 25 }))}</p>` : ''}</div></details>`).join('')
	}

	function renderPlayground(scenarioId) {
		const picker = $('[data-api-scenario]')
		if (!picker || !state.examples) return
		const scenarios = state.examples.scenarios
		picker.innerHTML = scenarios.map(scenario => `<option value="${escapeHtml(scenario.id)}">${escapeHtml(scenarioTitle(scenario))} · ${escapeHtml(scenario.method)}</option>`).join('')
		const chosen = scenarios.find(scenario => scenario.id === scenarioId) || scenarios[0]
		if (!chosen) return
		picker.value = chosen.id
		updateQuery({ scenario: chosen.id })
		const summary = $('[data-scenario-summary]')
		if (summary) summary.textContent = chosen.summary?.[locale] || chosen.summary?.en || ''
		const fields = $('[data-request-fields]')
		if (fields) fields.innerHTML = chosen.inputs.map(input => {
			const nameLabel = input.name === 'key' ? translate('fieldKey') : input.name === 'fallback' ? translate('fieldFallback') : input.name === 'feature' ? translate('fieldFeature') : input.name === 'capability' ? translate('fieldCapability') : input.name === 'component' ? translate('fieldComponent') : input.name === 'field' ? translate('fieldField') : input.name === 'setting' ? translate('fieldSetting') : input.name === 'qualified_name' ? translate('fieldTypeName') : input.name
			if (input.type === 'select' && Array.isArray(input.options)) return `<label class="api-field-label">${escapeHtml(nameLabel)}<select name="${escapeHtml(input.name)}" data-input-type="string">${input.options.map(option => `<option value="${escapeHtml(option)}" ${option === input.value ? 'selected' : ''}>${escapeHtml(option)}</option>`).join('')}</select></label>`
			if (input.type === 'boolean') return `<label class="api-field-label">${escapeHtml(nameLabel)}<select name="${escapeHtml(input.name)}" data-input-type="boolean"><option value="true" ${input.value === true ? 'selected' : ''}>${escapeHtml(translate('booleanTrue'))}</option><option value="false" ${input.value === false ? 'selected' : ''}>${escapeHtml(translate('booleanFalse'))}</option></select></label>`
			return `<label class="api-field-label">${escapeHtml(nameLabel)}<input name="${escapeHtml(input.name)}" data-input-type="${input.type === 'number' ? 'number' : 'string'}" value="${escapeHtml(input.value)}" autocomplete="off"></label>`
		}).join('')
		updateGeneratedCall(chosen)
		const note = $('[data-response-note]')
		if (note) note.textContent = translate('runToSeeResponse')
		const output = $('[data-response-output] code')
		if (output) output.textContent = translate('noResponseYet')
		const source = $('[data-response-source]')
		if (source) source.textContent = translate('sourceChanges')
		const reference = $('[data-reference-link]')
		if (reference) reference.href = versionRoute('/tools/modding/api-reference/', { q: chosen.method })
	}

	function readInputs(scenario) {
		const values = {}
		for (const input of scenario.inputs) {
			const field = $(`[data-request-fields] [name="${CSS.escape(input.name)}"]`)
			if (!field) throw new Error(translate('inputError'))
			let value = field.value
			if (input.type === 'boolean') value = value === 'true'
			if (input.type === 'number') {
				value = Number(value)
				if (!Number.isFinite(value)) throw new Error(translate('inputError'))
			}
			values[input.name] = value
		}
		return values
	}

	function luaArgument(value) {
		if (typeof value === 'boolean' || typeof value === 'number') return String(value)
		if (value === null) return 'nil'
		return `"${String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\r/g, '\\r').replace(/\n/g, '\\n').replace(/\t/g, '\\t')}"`
	}

	function updateGeneratedCall(scenario) {
		const fields = $('[data-request-fields]')
		const values = Object.fromEntries(scenario.inputs.map(input => {
			const value = fields?.querySelector(`[name="${CSS.escape(input.name)}"]`)?.value ?? input.value
			if (input.type === 'boolean') return [input.name, value === 'true' ? 'true' : 'false']
			if (input.type === 'number') return [input.name, Number.isFinite(Number(value)) ? String(Number(value)) : '0']
			return [input.name, luaArgument(value)]
		}))
		const output = $('[data-generated-call]')
		if (!output) return
		if (scenario.codeTemplate) {
			output.textContent = scenario.codeTemplate.replace(/\{\{([A-Za-z0-9_]+)\}\}/g, (match, name) => values[name] ?? match)
			return
		}
		const args = scenario.inputs.map(input => values[input.name] ?? luaArgument(input.value))
		output.textContent = `${scenario.method}(${args.join(', ')})`
	}

	function getScenarioFromQuery() {
		return new URLSearchParams(location.search).get('scenario')
	}

	function simulate(scenario, values) {
		const localized = (value, fallback) => typeof value === 'string' ? value : value?.[locale] || value?.en || fallback
		if (scenario.handler === 'captured-calls') {
			const fixture = state.examples.fixtures?.[scenario.fixture]
			const inputName = scenario.captureInput || scenario.inputs?.[0]?.name
			const callNames = scenario.captureCallsByInput
				? scenario.captureCallsByInput[values[inputName]]
				: scenario.captureCalls
			const calls = Array.isArray(callNames) && Array.isArray(fixture?.calls)
				? callNames.map(name => fixture.calls.find(call => call.call === name))
				: []
			if (!calls.length || calls.some(call => !call)) {
				return { result: null, note: translate('loadFailure'), source: localized(scenario.source, '') }
			}
			return {
				result: calls.length === 1 ? calls[0] : calls,
				note: localized(scenario.responseNote, translate('responseRuntime')),
				source: localized(scenario.source, translate('runtimeSource')),
			}
		}
		if (scenario.handler === 'mod-setting') {
			const settings = state.examples.fixtures?.[scenario.fixture || 'settings'] || {}
			const exists = Object.hasOwn(settings, values.key)
			return {
				result: exists ? settings[values.key] : values.fallback,
				note: exists ? localized(scenario.responseNote, translate('responseSetting')) : translate('settingNotFound'),
				source: localized(scenario.source, translate('settingSource')),
			}
		}
		if (scenario.handler === 'runtime-status') {
			return { result: { feature: values.feature, available: false, reason: translate('runtimeReason') }, note: translate('responseRuntime'), source: translate('runtimeSource') }
		}
		if (scenario.handler === 'game-type') {
			const type = state.types?.[values.qualified_name]
			if (!type) return { result: null, note: translate('typeNotFound'), source: translate('typeSource') }
			const fields = Object.values(type.fields || {}).slice(0, 40).map(field => ({ name: field.name, type_name: field.type_name, data_offset: field.data_offset }))
			return { result: { qualified_name: type.qualified_name, name: type.name, primitive: type.primitive, size: type.size, field_count: type.field_count, fields }, note: translate('responseType'), source: translate('typeSource') }
		}
		throw new Error(translate('loadFailure'))
	}

	function setResponse(result, source, note) {
		const output = $('[data-response-output] code')
		const noteNode = $('[data-response-note]')
		const sourceNode = $('[data-response-source]')
		if (output) output.textContent = JSON.stringify(result, null, 2)
		if (noteNode) noteNode.textContent = note
		if (sourceNode) sourceNode.textContent = source
	}

	function setTab(name) {
		for (const tab of $$('[data-api-tab]')) {
			const selected = tab.dataset.apiTab === name
			tab.classList.toggle('is-active', selected)
			tab.setAttribute('aria-selected', String(selected))
			tab.tabIndex = selected ? 0 : -1
		}
		for (const panel of $$('[data-api-panel]')) panel.hidden = panel.dataset.apiPanel !== name
		if (name === 'types' || name === 'resources') {
			const target = $(`[data-${name === 'types' ? 'type' : 'resource'}-results]`)
			if (target && !state[name]) target.innerHTML = `<p class="api-loading" role="status">${escapeHtml(translate(name === 'types' ? 'typesLoading' : 'resourcesLoading'))}</p>`
			if (name === 'types') loadTypes()
			else loadResources()
		}
	}

	root.addEventListener('click', async event => {
		const retry = event.target.closest('[data-api-retry]')
		if (retry) {
			retry.disabled = true
			try { await initialize() }
			finally { retry.disabled = false }
			return
		}
		const pageButton = event.target.closest('[data-api-page-step]')
		if (pageButton) {
			const name = pageButton.dataset.apiPageList
			if (!Object.hasOwn(state.pages, name)) return
			const pagination = $(`[data-api-pagination="${name}"]`)
			const totalPages = Number(pagination?.dataset.totalPages) || 1
			state.pages[name] = Math.max(1, Math.min(totalPages, state.pages[name] + Number(pageButton.dataset.apiPageStep)))
			if (name === 'functions') renderApiResults()
			else if (name === 'types') renderTypes()
			else renderResources()
			return
		}
		const filterButton = event.target.closest('[data-api-filter]')
		if (filterButton) {
			state.filter = filterButton.dataset.apiFilter
			state.pages.functions = 1
			for (const button of $$('[data-api-filter]')) {
				const active = button === filterButton
				button.classList.toggle('is-active', active)
				button.setAttribute('aria-pressed', String(active))
			}
			renderApiResults()
		}
		const tab = event.target.closest('[data-api-tab]')
		if (tab) setTab(tab.dataset.apiTab)
		if (event.target.closest('[data-api-clear]')) {
			const search = $('[data-api-search]')
			if (search) search.value = ''
			state.pages.functions = 1
			updateQuery({ q: null })
			renderApiResults()
			search?.focus()
		}
		if (event.target.closest('[data-copy-call]')) {
			const call = $('[data-generated-call]')?.textContent || ''
			const status = $('[data-copy-status]')
			try {
				await navigator.clipboard.writeText(call)
				if (status) status.textContent = translate('copySuccess')
			} catch { if (status) status.textContent = translate('copyFailure') }
		}
	})

	root.addEventListener('input', event => {
		if (event.target.matches('[data-api-search]')) {
			state.pages.functions = 1
			updateQuery({ q: event.target.value.trim() || null })
			renderApiResults()
		}
		if (event.target.matches('[data-type-search]')) {
			state.pages.types = 1
			renderTypes()
		}
		if (event.target.matches('[data-resource-search]')) {
			state.pages.resources = 1
			renderResources()
		}
		if (event.target.closest('[data-request-fields]')) {
			const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
			if (scenario) updateGeneratedCall(scenario)
		}
	})

	root.addEventListener('change', async event => {
		if (event.target.matches('[data-api-version]')) {
			try { await selectVersion(event.target.value) }
			catch (error) {
				renderVersionPicker()
				showApiLoadError(error)
			}
		}
		if (event.target.matches('[data-game-build]')) selectGameBuild(event.target.value)
		if (event.target.matches('[data-api-scenario]')) renderPlayground(event.target.value)
		if (event.target.matches('[data-api-page-size]')) {
			const pageSize = Number(event.target.value)
			if (!apiPageSizeOptions.includes(pageSize)) return
			state.pageSize = pageSize
			state.pages = { functions: 1, types: 1, resources: 1 }
			renderApiResults()
			renderTypes()
			renderResources()
		}
		if (event.target.closest('[data-request-fields]')) {
			const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
			if (scenario) updateGeneratedCall(scenario)
		}
	})

	root.addEventListener('submit', event => {
		if (!event.target.matches('[data-api-request-form]')) return
		event.preventDefault()
		const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
		if (!scenario) return
		try {
			const values = readInputs(scenario)
			if (scenario.handler === 'game-type' && !state.types) {
				setResponse(translate('typesLoading'), translate('typeSource'), translate('runToSeeResponse'))
				loadTypes().then(() => {
					try { const fresh = readInputs(scenario); const answer = simulate(scenario, fresh); setResponse(answer.result, answer.source, answer.note) }
					catch { setResponse(translate('loadFailure'), translate('typeSource'), translate('loadFailure')) }
				})
				return
			}
			const answer = simulate(scenario, values)
			setResponse(answer.result, answer.source, answer.note)
		} catch { setResponse(translate('inputError'), '', translate('inputError')) }
	})

	root.addEventListener('keydown', event => {
		const tab = event.target.closest('[data-api-tab]')
		if (!tab || !['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(event.key)) return
		event.preventDefault()
		const tabs = $$('[data-api-tab]')
		const index = tabs.indexOf(tab)
		const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length
		tabs[next]?.focus()
		setTab(tabs[next]?.dataset.apiTab)
	})

	async function initialize() {
		applyLocale()
		setTab('functions')
		clearApiLoadError()
		const search = $('[data-api-search]')
		if (search) search.value = new URLSearchParams(location.search).get('q') || ''
		gameBuildLoadPromise = loadGameBuilds().catch(error => {
			console.warn('[ShroudForge API tools] Game build information is unavailable.', error)
			const picker = $('[data-game-build]')
			if (picker) {
				picker.innerHTML = `<option>${escapeHtml(translate('gameBuildUnavailable'))}</option>`
				picker.disabled = true
			}
		})
		try {
			state.index = await fetchJson('/data/api/versions.json')
			if (!Array.isArray(state.index.versions) || !state.index.versions.length) throw new Error('No API versions')
			await selectVersion(new URLSearchParams(location.search).get('apiVersion') || state.index.current)
		} catch (error) {
			if (state.index) renderVersionPicker()
			else {
				const versionPicker = $('[data-api-version]')
				if (versionPicker) {
					versionPicker.innerHTML = `<option>${escapeHtml(translate('versionsUnavailableOption'))}</option>`
					versionPicker.disabled = true
				}
			}
			if (!state.examples) {
				const examplePicker = $('[data-api-scenario]')
				if (examplePicker) {
					examplePicker.innerHTML = `<option>${escapeHtml(translate('examplesUnavailableOption'))}</option>`
					examplePicker.disabled = true
				}
			}
			showApiLoadError(error)
		}
	}

	initialize()
})()
