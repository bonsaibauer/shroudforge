(() => {
	const root = document.querySelector('[data-api-tool]')
	if (!root) return

	const text = {
		en: {
			skip: 'Skip to API tools', toolsNavLabel: 'Tools', moddingNavLabel: 'Modding', modSettingsNav: 'Mod-Settings', apiCallsNav: 'API calls', browseApiNav: 'Browse API', standardsNavLabel: 'Standards',
			toolsEyebrow: 'SHROUDFORGE TOOLS · MODDING', apiReferenceTitle: 'Browse the API', apiReferenceLead: 'Find an example by task or search every documented function, type, and game resource.', apiPlaygroundTitle: 'Try API calls', apiPlaygroundLead: 'Choose a real modding task, edit its inputs, and see the Lua call and a sample response.', tryCallsAction: 'Try API calls', browseApiAction: 'Browse API', apiVersion: 'ShroudForge API version', apiCategories: 'API categories', apiReferenceSections: 'API reference sections',
			catalogStatusTitle: 'Published API and game-data snapshot', catalogStatusBody: 'This page reads published files. It cannot inspect or run inside your current game.', noLiveConnection: 'No live game connection', statusGuideTitle: 'What do these API status badges mean?', statusDocumented: 'Documented', statusDocumentedBody: 'This entry is in the published API catalogue. The catalogue is generated from the Lua API declarations.', statusCapability: 'Mod permission', statusCapabilityBody: 'For runtime entries, the required capability is derived from the Modloader checks already used by the runtime.', statusDeclared: 'Runtime declared', statusDeclaredBody: 'The operation identifier is listed in runtime-operations.json. That does not mean it is available in every game process.', statusLive: 'Live availability', statusLiveBody: 'Only runtime.status or runtime.has inside the running game can report current availability for this mod.', liveUnknownBadge: 'Not checked live', statusEffect: 'Gameplay effect verification', statusEffectBody: 'No central per-feature gameplay verification result is published here. Do not infer an in-game effect from an API declaration or live availability.', effectUnknownBadge: 'No published effect result', apiRelease: 'API {api} · ShroudForge {release}', gameBuild: 'Captured game build {build}',
			startWithTask: 'START WITH A TASK', commonTasks: 'What do you want your mod to do?', taskLead: 'Choose an example to see its Lua call and what the response means.', loadingExamples: 'Loading examples…',
		referenceEyebrow: 'SEARCHABLE REFERENCE', allApiEntries: 'Find an API entry', searchLead: 'Search by the task you have in mind, a function name, or a parameter.', searchLabel: 'Search the API', searchPlaceholder: 'Try “read a setting” or shroudforge.settings.get', clearSearch: 'Clear', filterAll: 'Everything', filterRuntime: 'Runtime', filterGame: 'Game data', functionsTab: 'Lua functions', typesTab: 'Game types', resourcesTab: 'Game resources', loadingCatalog: 'Loading the API catalogue…', typeDataNote: 'These details come from a captured Enshrouded build. They do not describe a live game process.', searchTypes: 'Search game types', typeSearchPlaceholder: 'Type name or field', resourceDataNote: 'Resource names and IDs come from the same captured game snapshot.', searchResources: 'Search game resources', resourceSearchPlaceholder: 'Resource name or ID', referenceFooter: 'The catalogue documents the Lua interface. Check runtime.has or runtime.status inside your mod to learn what is available in the current game process.',
			apiCount: '{count} API entries', apiCountFiltered: '{count} matching API entries', noApiResults: 'No API entries match this search. Try a shorter task or function name.', resultLimit: 'Showing {shown} of {total} matches. Add a search term to narrow the list.', documentedBadge: 'Documented', operationBadge: 'Runtime declared', capabilityBadge: 'Requires {capability} capability', runtimeApiBadge: 'Runtime API', gameApiBadge: 'Game API', staticAvailability: 'Live availability is checked only inside the game with runtime.has or runtime.status.', effectNotProven: 'This API entry does not by itself prove that a gameplay effect was tested.', signatureLabel: 'Lua call', descriptionLabel: 'What it does', parametersLabel: 'Inputs', returnsLabel: 'Returns', errorsLabel: 'Errors', exampleLabel: 'Example', sourceLabel: 'Declaration source', noParameters: 'No named inputs are listed.', noReturn: 'No return value is listed.', noExample: 'No example is included in this declaration.', tryExample: 'Open a matching example',
		typesLoading: 'Loading captured game types…', resourcesLoading: 'Loading captured game resources…', typeSearchPrompt: 'Enter a type name or field to search the captured catalogue.', resourceSearchPrompt: 'Enter a resource type, GUID, or part number to search.', noTypeResults: 'No captured types match that search.', noResourceResults: 'No captured resources match that search.', showFirst: 'Showing the first {shown} of {total} matches. Add more detail to the search.', typeCount: '{count} game types', resourceCount: '{count} resource types', fieldsCount: '{count} fields', primitiveLabel: 'Kind', sizeLabel: 'Size', alignmentLabel: 'Alignment', fieldsLabel: 'Fields', moreFields: '{count} more fields', enumValues: 'Enum values', resourceEntries: '{count} captured entries', partLabel: 'Part', snapshotLabel: 'Captured snapshot',
		exampleEditor: 'EXAMPLE CALL EDITOR', editRunExample: 'Edit and run an example', playgroundLead: 'Change the inputs below. The browser answers from included sample data or a clearly marked runtime fixture.', chooseExample: 'Choose an example', loadingExamplesOption: 'Loading examples…', generatedLua: 'Generated Lua call', copyCode: 'Copy code', runExample: 'Run example', exampleResponse: 'EXAMPLE RESPONSE', responseHeading: 'What the call returns', sampleOnly: 'Sample only', runToSeeResponse: 'Run the example to see a response.', noResponseYet: 'No response yet.', responseSource: 'Response source', sourceChanges: 'The source is shown after you run an example.', viewApiEntry: 'View API entry →', simulationHeading: 'What does “Run example” do?', simulationBody: 'It runs a small, allowlisted browser simulation. Settings and game-type examples read the included sample files. Runtime examples show a fixture. No Lua script is sent to a server or to your game.', playgroundFooter: 'Use the generated Lua in your mod, then check live availability with runtime.has or runtime.status in game.',
			fieldKey: 'Setting key', fieldFallback: 'Fallback value', fieldFeature: 'Runtime feature', fieldTypeName: 'Qualified type name', booleanTrue: 'true', booleanFalse: 'false', scenarioReadSetting: 'Read a mod setting', scenarioRuntimeStatus: 'Check a runtime requirement', scenarioGameType: 'Look up a game type',
			settingSource: 'Starter Mod example: extended.mod.json settings', runtimeSource: 'Simulated status fixture. The browser cannot inspect the current game process.', typeSource: 'Captured Enshrouded game-data snapshot', responseSetting: 'This value is read from the example mod settings included with ShroudForge.', responseRuntime: 'This is a sample response shape only. The website did not query a running game.', responseType: 'This result is a view of captured game metadata, not a live object from the game.', settingNotFound: 'The example settings do not contain this key, so the fallback value is returned.', typeNotFound: 'This type name does not exist in the captured snapshot.', runtimeReason: 'This website has no connection to the current game process.', inputError: 'Enter a valid value for each input.',
			copySuccess: 'Lua call copied.', copyFailure: 'Copy is unavailable in this browser.', loadFailure: 'The API data could not be loaded. Reload the page or choose another API version.', runtimeCapability: 'runtime', patchCapability: 'patch', exportCapability: 'export',
		},
	}

	const escapeHtml = value => String(value ?? '').replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char])
	const locale = 'en'
	const state = { index: null, catalog: null, examples: null, operations: null, gameProfile: null, types: null, resources: null, filter: 'all', typeLoading: false, resourceLoading: false }
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
		const response = await fetch(url, { headers: { Accept: 'application/json' } })
		if (!response.ok) throw new Error(`${response.status} ${url}`)
		return response.json()
	}

	function versionRoute(route, extra = {}) {
		const params = new URLSearchParams({ apiVersion: state.catalog?.version || state.index?.current || '' })
		for (const [key, value] of Object.entries(extra)) if (value) params.set(key, value)
		return `${route}?${params}`
	}

	function setVersionFacts() {
		const release = $('[data-api-release]')
		if (release && state.catalog) release.textContent = translate('apiRelease', { api: state.catalog.version, release: state.catalog.shroudforge_version || '—' })
		const build = $('[data-game-build]')
		const gameVersion = state.gameProfile?.game_version || ''
		const gameBuild = gameVersion.split('|')[0] || '—'
		if (build) build.textContent = translate('gameBuild', { build: gameBuild })
	}

	function renderVersionPicker() {
		const picker = $('[data-api-version]')
		if (!picker || !state.index) return
		picker.innerHTML = state.index.versions.map(entry => {
			const label = `API ${entry.version}${entry.shroudforgeVersion ? ` · ShroudForge ${entry.shroudforgeVersion}` : ''}${entry.current ? ' · current' : ''}`
			return `<option value="${escapeHtml(entry.version)}">${escapeHtml(label)}</option>`
		}).join('')
		picker.disabled = false
		picker.value = state.catalog?.version || state.index.current
		const versionNote = $('[data-api-version-note]')
		if (versionNote) versionNote.textContent = state.index.versions.length < 2
			? 'One API version has been published so far.'
			: 'Choose the version you are developing for.'
	}

	async function selectVersion(version) {
		const selected = state.index.versions.find(item => item.version === version) || state.index.versions.find(item => item.version === state.index.current) || state.index.versions[0]
		if (!selected) throw new Error('API version index is empty')
		const base = `/data/api/${encodeURIComponent(selected.version)}`
		const [catalog, examples, operations] = await Promise.all([
			fetchJson(`${base}/catalog.json`), fetchJson(`${base}/examples.json`), fetchJson(`${base}/runtime-operations.json`),
		])
		if (catalog.version !== selected.version || !Array.isArray(catalog.symbols)) throw new Error('Invalid API catalogue')
		if (!Array.isArray(examples.scenarios) || !Array.isArray(operations.runtimeOperations)) throw new Error('Invalid API example data')
		state.catalog = catalog
		state.examples = examples
		state.operations = new Set(operations.runtimeOperations)
		updateQuery({ apiVersion: selected.version })
		renderVersionPicker()
		setVersionFacts()
		renderTasks()
		renderApiResults()
		if (mainRoute === 'playground') renderPlayground(getScenarioFromQuery())
	}

	async function loadGameProfile() {
		const current = await fetchJson('/data/game/current.json')
		const profile = await fetchJson(`/data/game/${encodeURIComponent(current.snapshot)}/profile.json`)
		state.gameProfile = { ...profile, snapshot: current.snapshot }
		setVersionFacts()
	}

	function scenarioTitle(scenario) {
		const names = { 'read-mod-setting': 'scenarioReadSetting', 'check-runtime-availability': 'scenarioRuntimeStatus', 'look-up-game-type': 'scenarioGameType' }
		return names[scenario.id] ? translate(names[scenario.id]) : (scenario.title?.[locale] || scenario.title?.en || scenario.method)
	}

	function renderTasks() {
		const target = $('[data-api-tasks]')
		if (!target || !state.examples) return
		target.innerHTML = state.examples.scenarios.map(scenario => `<a class="api-task-card" href="${escapeHtml(versionRoute('/tools/modding/api-playground/', { scenario: scenario.id }))}"><span class="api-task-mark" aria-hidden="true">↗</span><span class="api-task-copy"><strong>${escapeHtml(scenarioTitle(scenario))}</strong><small>${escapeHtml(scenario.summary?.[locale] || scenario.summary?.en || '')}</small><code>${escapeHtml(scenario.method)}</code></span></a>`).join('')
	}

	function htmlInline(value) {
		return escapeHtml(value).replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\n/g, '<br>')
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
		const badges = [`<span class="api-entry-badge is-documented">${escapeHtml(translate('documentedBadge'))}</span>`]
		const capability = capabilityFor(symbol)
		if (capability) badges.push(`<span class="api-entry-badge is-capability">${escapeHtml(translate('capabilityBadge', { capability: translate(`${capability}Capability`) }))}</span>`)
		if (state.operations?.has(symbol.name)) badges.push(`<span class="api-entry-badge is-declared">${escapeHtml(translate('operationBadge'))}</span>`)
		if (apiKind(symbol) === 'runtime') badges.push(`<span class="api-entry-badge is-live">${escapeHtml(translate('liveUnknownBadge'))}</span><span class="api-entry-badge is-effect">${escapeHtml(translate('effectUnknownBadge'))}</span>`)
		return badges.join('')
	}

	function renderParamList(params, key, emptyKey) {
		if (!Array.isArray(params) || !params.length) return `<p class="api-detail-muted">${escapeHtml(translate(emptyKey))}</p>`
		return `<dl class="api-param-list">${params.map(param => `<div><dt><code>${escapeHtml(param.name || '')}</code><span>${escapeHtml(param.type || '')}</span></dt><dd>${htmlInline(param.description || '')}</dd></div>`).join('')}</dl>`
	}

	function renderSymbol(symbol) {
		const signature = symbol.signature || symbol.name
		const category = sourceCategory(symbol)
		const kind = apiKind(symbol)
		const tryLink = state.examples?.scenarios.find(scenario => scenario.method === symbol.name)
		const returns = Array.isArray(symbol.returnDetails) && symbol.returnDetails.length
			? renderParamList(symbol.returnDetails.map(item => ({ ...item, name: item.name || 'value' })), 'returns', 'noReturn')
			: `<p class="api-return-items">${(symbol.returns || []).map(item => `<code>${escapeHtml(item)}</code>`).join(', ') || escapeHtml(translate('noReturn'))}</p>`
		const declared = state.operations?.has(symbol.name)
		return `<details class="api-entry-card" data-api-entry data-category="${category}">
			<summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(signature)}</code><span class="api-entry-summary-description">${htmlInline(symbol.description || '')}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary>
			<div class="api-entry-detail">
			<div class="api-entry-badges">${statusBadges(symbol)}<span class="api-entry-badge is-source">${category === 'shroudforge' ? 'ShroudForge' : 'EML'}</span>${kind === 'runtime' ? `<span class="api-entry-badge is-source">${escapeHtml(translate('runtimeApiBadge'))}</span>` : kind === 'game' ? `<span class="api-entry-badge is-source">${escapeHtml(translate('gameApiBadge'))}</span>` : ''}</div>
				<div class="api-entry-block"><h3>${escapeHtml(translate('descriptionLabel'))}</h3><p>${htmlInline(symbol.description || '')}</p></div>
				<div class="api-entry-block"><h3>${escapeHtml(translate('signatureLabel'))}</h3><pre class="api-code-panel"><code>${escapeHtml(signature)}</code></pre></div>
				<div class="api-entry-columns"><section class="api-entry-block"><h3>${escapeHtml(translate('parametersLabel'))}</h3>${renderParamList(symbol.params, 'params', 'noParameters')}</section><section class="api-entry-block"><h3>${escapeHtml(translate('returnsLabel'))}</h3>${returns}</section></div>
				${symbol.errors ? `<div class="api-entry-block"><h3>${escapeHtml(translate('errorsLabel'))}</h3><p>${htmlInline(symbol.errors)}</p></div>` : ''}
				${symbol.example ? `<div class="api-entry-block"><h3>${escapeHtml(translate('exampleLabel'))}</h3><pre class="api-code-panel"><code>${escapeHtml(symbol.example)}</code></pre></div>` : `<p class="api-detail-muted">${escapeHtml(translate('noExample'))}</p>`}
				${declared ? `<p class="api-evidence-note">${escapeHtml(translate('statusDeclaredBody'))}</p>` : ''}
				${kind === 'runtime' ? `<p class="api-evidence-note">${escapeHtml(translate('staticAvailability'))}</p>` : ''}
				${kind === 'runtime' ? '' : `<p class="api-evidence-note">${escapeHtml(translate('effectNotProven'))}</p>`}
				<p class="api-source-path"><span>${escapeHtml(translate('sourceLabel'))}:</span> <code>${escapeHtml(symbol.source || '—')}</code></p>
				${tryLink ? `<a class="api-reference-link" href="${escapeHtml(versionRoute('/tools/modding/api-playground/', { scenario: tryLink.id }))}">${escapeHtml(translate('tryExample'))} →</a>` : ''}
			</div>
		</details>`
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
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noApiResults'))}</p>`
			return
		}
		const shown = matches.slice(0, 60)
		target.innerHTML = `${matches.length > shown.length ? `<p class="api-result-limit">${escapeHtml(translate('resultLimit', { shown: shown.length, total: matches.length }))}</p>` : ''}${shown.map(renderSymbol).join('')}`
	}

	async function gameDataBase() {
		const current = await fetchJson('/data/game/current.json')
		return `/data/game/${encodeURIComponent(current.snapshot)}`
	}

	async function loadTypes() {
		if (state.types || state.typeLoading) return
		state.typeLoading = true
		try {
			const base = await gameDataBase()
			state.types = await fetchJson(`${base}/types.json`)
			state.gameProfile ||= await fetchJson(`${base}/profile.json`)
			setVersionFacts()
			renderTypes()
		} catch {
			const target = $('[data-type-results]')
			if (target) target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
		} finally { state.typeLoading = false }
	}

	async function loadResources() {
		if (state.resources || state.resourceLoading) return
		state.resourceLoading = true
		try {
			const base = await gameDataBase()
			state.resources = await fetchJson(`${base}/resources.json`)
			state.gameProfile ||= await fetchJson(`${base}/profile.json`)
			renderResources()
		} catch {
			const target = $('[data-resource-results]')
			if (target) target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
		} finally { state.resourceLoading = false }
	}

	function renderTypes() {
		const target = $('[data-type-results]')
		if (!target || !state.types) return
		const queryText = ($('[data-type-search]')?.value || '').trim().toLocaleLowerCase(locale)
		if (!queryText) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('typeSearchPrompt'))}</p>`
			return
		}
		const keys = Object.keys(state.types)
		const matches = keys.filter(key => {
			const item = state.types[key]
			const fields = Object.values(item.fields || {}).map(field => `${field.name} ${field.type_name}`)
			return `${key} ${item.primitive || ''} ${item.name || ''} ${fields.join(' ')}`.toLocaleLowerCase(locale).includes(queryText)
		}).sort((a, b) => (a.toLocaleLowerCase().startsWith(queryText) ? -1 : 0) - (b.toLocaleLowerCase().startsWith(queryText) ? -1 : 0) || a.localeCompare(b))
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noTypeResults'))}</p>`
			return
		}
		const shown = matches.slice(0, 40)
		target.innerHTML = `${matches.length > shown.length ? `<p class="api-result-limit">${escapeHtml(translate('showFirst', { shown: shown.length, total: matches.length }))}</p>` : ''}${shown.map(key => {
			const item = state.types[key]
			const fields = Object.values(item.fields || {}).sort((a, b) => a.name.localeCompare(b.name))
			const visibleFields = fields.slice(0, 35)
			const enumValues = Object.entries(item.enum_values || {})
			return `<details class="api-entry-card api-type-card"><summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(key)}</code><span class="api-entry-summary-description">${escapeHtml(item.primitive || 'Type')} · ${escapeHtml(translate('fieldsCount', { count: item.field_count ?? fields.length }))}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary><div class="api-entry-detail"><div class="api-entry-badges"><span class="api-entry-badge is-documented">${escapeHtml(translate('snapshotLabel'))}</span><span class="api-entry-badge is-source">${escapeHtml(item.primitive || 'Type')}</span></div><dl class="api-type-meta"><div><dt>${escapeHtml(translate('primitiveLabel'))}</dt><dd>${escapeHtml(item.primitive || '—')}</dd></div><div><dt>${escapeHtml(translate('sizeLabel'))}</dt><dd>${escapeHtml(item.size ?? '—')} bytes</dd></div><div><dt>${escapeHtml(translate('alignmentLabel'))}</dt><dd>${escapeHtml(item.alignment ?? '—')}</dd></div></dl>${fields.length ? `<h3>${escapeHtml(translate('fieldsLabel'))}</h3><div class="api-field-list">${visibleFields.map(field => `<div><code>${escapeHtml(field.name)}</code><code>${escapeHtml(field.type_name)}</code><span>${escapeHtml(field.data_offset ?? '—')}</span></div>`).join('')}</div>${fields.length > visibleFields.length ? `<p class="api-detail-muted">${escapeHtml(translate('moreFields', { count: fields.length - visibleFields.length }))}</p>` : ''}` : ''}${enumValues.length ? `<h3>${escapeHtml(translate('enumValues'))}</h3><div class="api-field-list">${enumValues.slice(0, 40).map(([name, value]) => `<div><code>${escapeHtml(name)}</code><code>${escapeHtml(value)}</code></div>`).join('')}</div>` : ''}</div></details>`
		}).join('')}`
	}

	function renderResources() {
		const target = $('[data-resource-results]')
		if (!target || !state.resources) return
		const queryText = ($('[data-resource-search]')?.value || '').trim().toLocaleLowerCase(locale)
		if (!queryText) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('resourceSearchPrompt'))}</p>`
			return
		}
		const matches = Object.entries(state.resources).filter(([name, entries]) => `${name} ${entries.map(entry => `${entry.guid} ${entry.part}`).join(' ')}`.toLocaleLowerCase(locale).includes(queryText)).sort((a, b) => a[0].localeCompare(b[0]))
		if (!matches.length) {
			target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('noResourceResults'))}</p>`
			return
		}
		const shown = matches.slice(0, 40)
		target.innerHTML = `${matches.length > shown.length ? `<p class="api-result-limit">${escapeHtml(translate('showFirst', { shown: shown.length, total: matches.length }))}</p>` : ''}${shown.map(([name, entries]) => `<details class="api-entry-card"><summary><span class="api-entry-summary-main"><code class="api-entry-signature">${escapeHtml(name)}</code><span class="api-entry-summary-description">${escapeHtml(translate('resourceEntries', { count: entries.length }))}</span></span><span class="api-entry-chevron" aria-hidden="true">⌄</span></summary><div class="api-entry-detail"><div class="api-entry-badges"><span class="api-entry-badge is-documented">${escapeHtml(translate('snapshotLabel'))}</span></div><div class="api-resource-list">${entries.slice(0, 25).map(entry => `<div><code>${escapeHtml(entry.guid)}</code><span>${escapeHtml(translate('partLabel'))} ${escapeHtml(entry.part ?? 0)}</span></div>`).join('')}</div>${entries.length > 25 ? `<p class="api-detail-muted">${escapeHtml(translate('moreFields', { count: entries.length - 25 }))}</p>` : ''}</div></details>`).join('')}`
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
			const nameLabel = input.name === 'key' ? translate('fieldKey') : input.name === 'fallback' ? translate('fieldFallback') : input.name === 'feature' ? translate('fieldFeature') : input.name === 'qualified_name' ? translate('fieldTypeName') : input.name
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
		const args = scenario.inputs.map(input => {
			const value = fields?.querySelector(`[name="${CSS.escape(input.name)}"]`)?.value ?? input.value
			if (input.type === 'boolean') return value === 'true' ? 'true' : 'false'
			if (input.type === 'number') return Number.isFinite(Number(value)) ? String(Number(value)) : '0'
			return luaArgument(value)
		})
		const output = $('[data-generated-call]')
		if (output) output.textContent = `${scenario.method}(${args.join(', ')})`
	}

	function getScenarioFromQuery() {
		return new URLSearchParams(location.search).get('scenario')
	}

	function simulate(scenario, values) {
		if (scenario.handler === 'mod-setting') {
			const settings = state.examples.fixtures?.settings || {}
			const exists = Object.hasOwn(settings, values.key)
			return { result: exists ? settings[values.key] : values.fallback, note: exists ? translate('responseSetting') : translate('settingNotFound'), source: translate('settingSource') }
		}
		if (scenario.handler === 'runtime-status') {
			return { result: { available: false, reason: translate('runtimeReason') }, note: translate('responseRuntime'), source: translate('runtimeSource') }
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
		const filterButton = event.target.closest('[data-api-filter]')
		if (filterButton) {
			state.filter = filterButton.dataset.apiFilter
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
			updateQuery({ q: event.target.value.trim() || null })
			renderApiResults()
		}
		if (event.target.matches('[data-type-search]')) renderTypes()
		if (event.target.matches('[data-resource-search]')) renderResources()
		if (event.target.closest('[data-request-fields]')) {
			const scenario = state.examples?.scenarios.find(item => item.id === getScenarioFromQuery())
			if (scenario) updateGeneratedCall(scenario)
		}
	})

	root.addEventListener('change', async event => {
		if (event.target.matches('[data-api-version]')) {
			try { await selectVersion(event.target.value) }
			catch { const target = $('[data-api-results]'); if (target) target.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>` }
		}
		if (event.target.matches('[data-api-scenario]')) renderPlayground(event.target.value)
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
		const search = $('[data-api-search]')
		if (search) search.value = new URLSearchParams(location.search).get('q') || ''
		try {
			state.index = await fetchJson('/data/api/versions.json')
			if (!Array.isArray(state.index.versions) || !state.index.versions.length) throw new Error('No API versions')
			await Promise.all([selectVersion(new URLSearchParams(location.search).get('apiVersion') || state.index.current), loadGameProfile()])
			renderVersionPicker()
			if (mainRoute === 'playground') renderPlayground(getScenarioFromQuery())
			else renderApiResults()
		} catch {
			const catalog = $('[data-api-results]')
			if (catalog) catalog.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
			const tasks = $('[data-api-tasks]')
			if (tasks) tasks.innerHTML = `<p class="api-empty-state">${escapeHtml(translate('loadFailure'))}</p>`
		}
	}

	initialize()
})()
