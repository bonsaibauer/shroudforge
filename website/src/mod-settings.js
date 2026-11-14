const tool = document.querySelector('[data-mod-settings-tool]')

if (tool) {
  const exampleNode = document.querySelector('#mod-settings-example')
  const example = JSON.parse(exampleNode?.content.textContent || '{}')
  const plusIcon = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M8 3.5v9M3.5 8h9"/></svg>'
  const checkIcon = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m3.5 8.5 3 3 6-7"/></svg>'
  const manifestEditor = tool.querySelector('[data-json-editor="manifest"]')
  const extendedEditor = tool.querySelector('[data-json-editor="extended"]')
  const preview = tool.querySelector('[data-modloader-preview]')
  const versionSelect = tool.querySelector('[data-modloader-version]')
  const exampleSourceLink = tool.querySelector('[data-example-source-link]')
  const settingCatalog = tool.querySelector('[data-setting-catalog]')
  const settingFields = tool.querySelector('[data-setting-fields]')
  const settingItems = tool.querySelector('[data-setting-items]')
  const manifestDependencies = tool.querySelector('[data-manifest-dependencies]')
  const manifestCapabilities = tool.querySelector('[data-manifest-capabilities]')
  const badgeCatalog = tool.querySelector('[data-badge-catalog]')
  const badgeFields = tool.querySelector('[data-badge-fields]')
  const lineNumberLists = Object.fromEntries(
    [...tool.querySelectorAll('[data-line-number-list]')].map(node => [node.dataset.lineNumberList, node]),
  )
  const statusNodes = Object.fromEntries(
    [...tool.querySelectorAll('[data-json-status]')].map(node => [node.dataset.jsonStatus, node]),
  )
  const templates = [
    { id: 'toggle', label: 'Switch', description: 'A compact on or off control.' },
    { id: 'checkbox', label: 'Checkbox', description: 'An on or off checkbox.' },
    { id: 'slider', label: 'Slider', description: 'A number the player can adjust.' },
    { id: 'number', label: 'Number', description: 'A number field.' },
    { id: 'select', label: 'Dropdown', description: 'Choose one option from a list.' },
    { id: 'radio', label: 'Radio choices', description: 'Show all single-choice options.' },
    { id: 'segmented', label: 'Button choices', description: 'Choose one option from buttons.' },
    { id: 'multiselect', label: 'Multiple choices', description: 'Choose any number of options.' },
    { id: 'text', label: 'Short text', description: 'A single-line text field.' },
    { id: 'textarea', label: 'Long text', description: 'A multi-line text field.' },
    { id: 'color', label: 'Color', description: 'Choose a color.' },
    { id: 'keybind', label: 'Key binding', description: 'Set a keyboard or mouse binding.' },
  ]
  const manifestCapabilityOptions = ['patch', 'export', 'runtime', 'runtime-register-dll']
  let currentVersion = versionSelect?.value || example.version || '1.8.0'
  let selectedSettingTemplate = ''
  let settingsExpanded = false
  let badgesExpanded = false
  const expandedSettingKeys = new Set()
  const expandedBadgeIds = new Set()
  let manifest = clone(example.manifest || {})
  let extended = clone(example.extended || {})
  let renderTimer = 0
  const errorLines = { manifest: 0, extended: 0 }

  function clone(value) {
    return JSON.parse(JSON.stringify(value))
  }

  function escapeHtml(value) {
    return String(value ?? '').replace(/[&<>"']/g, character => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    })[character])
  }

  function renderLineNumbers(editor, key) {
    const lineCount = Math.max(1, editor.value.split('\n').length)
    lineNumberLists[key].innerHTML = Array.from({ length: lineCount }, (_, index) => {
      const line = index + 1
      return `<span class="json-line-number${errorLines[key] === line ? ' is-error' : ''}">${line}</span>`
    }).join('')
    lineNumberLists[key].style.transform = `translateY(-${editor.scrollTop}px)`
  }

  function setEditorStatus(editor, key, error) {
    const status = statusNodes[key]
    const message = status.querySelector('[data-json-message]')
    const warning = status.querySelector('[data-json-warning]')
    if (!error) {
      errorLines[key] = 0
      message.textContent = ''
      warning.hidden = true
      status.classList.remove('has-error')
      renderLineNumbers(editor, key)
      return
    }

    const details = error.message.match(/\bline (\d+) column (\d+)/i)
    const position = error.message.match(/\bposition (\d+)/i)
    let line = details ? Number(details[1]) : 1
    let column = details ? Number(details[2]) : 1
    if (!details && position) {
      const beforeError = editor.value.slice(0, Number(position[1]))
      line = beforeError.split('\n').length
      column = beforeError.length - beforeError.lastIndexOf('\n')
    }
    errorLines[key] = line
    const reason = error.message.replace(/\s+at position \d+(?: \(line \d+ column \d+\))?/i, '')
    message.textContent = `Line ${line}, column ${column}: ${reason}`
    warning.hidden = false
    status.classList.add('has-error')
    renderLineNumbers(editor, key)
  }

  function parseEditor(editor, key) {
    try {
      const value = JSON.parse(editor.value)
      if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Use a JSON object at the top level.')
      setEditorStatus(editor, key, null)
      return value
    } catch (error) {
      setEditorStatus(editor, key, error)
      return null
    }
  }

  function readEditors() {
    const nextManifest = parseEditor(manifestEditor, 'manifest')
    const nextExtended = parseEditor(extendedEditor, 'extended')
    if (!nextManifest || !nextExtended) return null
    manifest = nextManifest
    extended = nextExtended
    return { manifest, extended }
  }

  function publishPreview() {
    const current = readEditors()
    if (!current || !preview.contentWindow) return
    preview.contentWindow.postMessage({
      source: 'shroudforge-mod-settings-tool',
      type: 'render',
      version: currentVersion,
      manifest: current.manifest,
      extended: current.extended,
    }, location.origin)
  }

  function updatePreviewSoon() {
    window.clearTimeout(renderTimer)
    renderTimer = window.setTimeout(publishPreview, 100)
  }

  function updateEditor(editor, value) {
    editor.value = `${JSON.stringify(value, null, 2)}\n`
    renderLineNumbers(editor, editor.dataset.jsonEditor)
  }

  function versionedSchemaUrl(kind) {
    const file = kind === 'manifest' ? 'manifest.schema.json' : 'extended.mod.schema.json'
    return new URL(`${currentVersion}/${file}`, example.schemaBaseUrl).href
  }

  function updateSchemaReferences() {
    for (const kind of ['manifest', 'extended']) {
      const link = tool.querySelector(`[data-schema-link="${kind}"]`)
      if (!link) continue
      link.href = versionedSchemaUrl(kind)
      link.textContent = `Schema ${currentVersion}`

      const editor = kind === 'manifest' ? manifestEditor : extendedEditor
      const value = parseEditor(editor, kind)
      if (!value) continue
      value.$schema = versionedSchemaUrl(kind)
      if (kind === 'manifest') manifest = value
      else extended = value
      updateEditor(editor, value)
    }
  }

  function downloadJson(editor, key, filename) {
    if (!parseEditor(editor, key)) {
      editor.focus()
      return
    }

    const content = editor.value.endsWith('\n') ? editor.value : `${editor.value}\n`
    const file = new Blob([content], { type: 'application/json;charset=utf-8' })
    const url = URL.createObjectURL(file)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    link.hidden = true
    document.body.append(link)
    link.click()
    link.remove()
    window.setTimeout(() => URL.revokeObjectURL(url), 1000)
  }

  function renderManifestFields(source = manifest) {
    const authors = Array.isArray(source.authors) ? source.authors.join(', ') : ''
    for (const node of tool.querySelectorAll('[data-manifest-field]')) {
      const key = node.dataset.manifestField
      node.value = key === 'authors' ? authors : (source[key] ?? '')
    }
    renderManifestDependencies(source)
    renderManifestCapabilities(source)
  }

  function renderManifestDependencies(source = manifest) {
    const dependencies = Array.isArray(source.dependencies) ? source.dependencies : []
    manifestDependencies.innerHTML = dependencies.map((dependency, index) => `
      <div class="manifest-dependency-row">
        <label>Mod ID<input data-dependency-index="${index}" data-dependency-field="id" value="${escapeHtml(dependency?.id)}" autocomplete="off" spellcheck="false" placeholder="other-mod.id"></label>
        <label>Version range<input data-dependency-index="${index}" data-dependency-field="version" value="${escapeHtml(dependency?.version)}" autocomplete="off" spellcheck="false" placeholder="^1.0.0"></label>
        <label class="manifest-optional-field"><input data-dependency-index="${index}" data-dependency-field="optional" type="checkbox"${dependency?.optional === true ? ' checked' : ''}><span>Optional</span></label>
        <button class="remove-template" type="button" data-remove-dependency="${index}" aria-label="Remove dependency ${index + 1}">×</button>
      </div>`).join('') || '<p class="manifest-empty-state">No dependencies added.</p>'
  }

  function renderManifestCapabilities(source = manifest) {
    const selected = Array.isArray(source.capabilities) ? source.capabilities : []
    manifestCapabilities.innerHTML = manifestCapabilityOptions.map(capability => `
      <button class="manifest-capability${selected.includes(capability) ? ' is-selected' : ''}" type="button" data-manifest-capability="${capability}" aria-pressed="${selected.includes(capability)}"><span class="manifest-capability-check" aria-hidden="true">${selected.includes(capability) ? '✓' : ''}</span><span>${capability}</span></button>`).join('')
  }

  function renderSettingCatalog() {
    settingCatalog.innerHTML = templates.map(template => `
      <button type="button" class="template-card${selectedSettingTemplate === template.id ? ' is-selected' : ''}" data-setting-template="${template.id}" aria-pressed="${selectedSettingTemplate === template.id}">
        <span class="template-mark">${escapeHtml(template.label.slice(0, 1))}</span>
        <span class="template-card-copy"><strong>${escapeHtml(template.label)}</strong><small>${escapeHtml(template.description)}</small></span>
        <span class="template-add" aria-hidden="true">${plusIcon}</span>
      </button>`).join('')
  }

  function renderSettingFields(control) {
    selectedSettingTemplate = control
    const template = templates.find(item => item.id === control)
    if (!template) {
      settingFields.hidden = true
      settingFields.innerHTML = ''
      return
    }
    const isBoolean = ['toggle', 'checkbox'].includes(control)
    const isNumber = ['number', 'slider'].includes(control)
    const isChoice = ['select', 'radio', 'segmented', 'multiselect'].includes(control)
    let initialValue = ''
    if (isBoolean) initialValue = '<label class="builder-switch-field"><input name="setting-value" type="checkbox" checked><span>On by default</span></label>'
    else if (control === 'color') initialValue = '<input name="setting-value" type="color" value="#c9ad6e">'
    else initialValue = `<input name="setting-value" ${isNumber ? 'type="number" step="any" value="1"' : 'type="text"'} placeholder="${control === 'keybind' ? 'SDL:64 or Ctrl+SDL:29|Mouse:4' : 'Default value'}">`
    settingFields.hidden = false
    settingFields.innerHTML = `
      <form class="setting-template-form" data-setting-form>
        <div class="builder-form-heading"><div><span class="eyebrow">SETTING TEMPLATE</span><h4>${escapeHtml(template.label)}</h4></div><button type="button" class="text-button" data-close-setting-template>Close</button></div>
        <input name="setting-control" type="hidden" value="${control}">
        <div class="builder-form-grid">
          <label>Key in your mod code<input name="setting-key" required maxlength="80" placeholder="flightSpeed"></label>
          <label>Label for players<input name="setting-label" required maxlength="120" placeholder="Flight speed"></label>
          <label class="builder-field-wide">Description<input name="setting-description" maxlength="500" placeholder="Explain what this setting changes"></label>
          <label>Default value${initialValue}</label>
          <label>Settings group<input name="setting-group" value="Settings" maxlength="120"></label>
          ${isChoice ? '<label class="builder-field-wide">Choices<textarea name="setting-options" required rows="3" placeholder="slow = Slow\nnormal = Normal\nfast = Fast"></textarea><small>One choice per line. Use <code>value = label</code>.</small></label>' : ''}
          ${isNumber ? '<label>Minimum<input name="setting-min" type="number" step="any" placeholder="Optional"></label><label>Maximum<input name="setting-max" type="number" step="any" placeholder="Optional"></label><label>Step<input name="setting-step" type="number" min="0.000001" step="any" placeholder="Optional"></label>' : ''}
        </div>
        <div class="builder-form-actions"><button class="button button-primary" type="submit">Add setting</button></div>
      </form>`
    settingFields.querySelector('[name="setting-key"]').focus()
  }

  function renderExistingValueField(control, value) {
    if (['toggle', 'checkbox'].includes(control)) {
      const checked = value === true || value === 'true'
      return `<label class="builder-switch-field"><input name="setting-value" type="checkbox"${checked ? ' checked' : ''}><span>Default value is on</span></label>`
    }
    if (['number', 'slider'].includes(control)) {
      return `<label>Default value<input name="setting-value" type="number" step="any" value="${escapeHtml(value ?? '')}" required></label>`
    }
    if (control === 'color' && typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value)) {
      return `<label>Default color<input name="setting-value" type="color" value="${escapeHtml(value)}"></label>`
    }
    if (Array.isArray(value)) {
      return `<label>Default values<input name="setting-value" value="${escapeHtml(value.join(', '))}" placeholder="value-one, value-two"></label>`
    }
    if (value && typeof value === 'object') {
      return `<label class="builder-field-wide">Default value (JSON)<textarea name="setting-value" rows="3" data-value-type="json">${escapeHtml(JSON.stringify(value, null, 2))}</textarea></label>`
    }
    if (control === 'textarea') {
      return `<label class="builder-field-wide">Default value<textarea name="setting-value" rows="3">${escapeHtml(value ?? '')}</textarea></label>`
    }
    return `<label>Default value<input name="setting-value" value="${escapeHtml(value ?? '')}"${control === 'color' ? ' placeholder="#c99a42"' : ''}></label>`
  }

  function renderExistingSettingEditor(key, definition, group) {
    const control = definition.control || 'automatic'
    const options = definition.options && typeof definition.options === 'object' && !Array.isArray(definition.options)
      ? JSON.stringify(definition.options, null, 2)
      : ''
    const knownControl = templates.some(template => template.id === control)
    const controlOptions = `${knownControl ? '' : `<option value="${escapeHtml(control)}" selected>${escapeHtml(control)}</option>`}${templates.map(template => `<option value="${template.id}"${control === template.id ? ' selected' : ''}>${escapeHtml(template.label)}</option>`).join('')}`
    const isChoice = ['select', 'radio', 'segmented', 'multiselect'].includes(control)
    return `<form class="setting-edit-form" data-existing-setting-form data-edit-setting="${escapeHtml(key)}" data-value-control="${escapeHtml(control)}">
      <div class="setting-edit-grid">
        <div class="setting-key-readonly"><span>Key in mod code</span><code>${escapeHtml(key)}</code><small>This key stays fixed so existing mod code and references keep working.</small></div>
        <label>Control<select name="setting-control" data-existing-control>${controlOptions}</select></label>
        <label>Label for players<input name="setting-label" value="${escapeHtml(definition.label || key)}" required maxlength="120"></label>
        <label>Setting group<input name="setting-group" value="${escapeHtml(group)}" maxlength="120" placeholder="Leave blank for no group"></label>
        <label class="builder-field-wide">Description<textarea name="setting-description" rows="2" maxlength="500">${escapeHtml(definition.description || '')}</textarea></label>
        <div data-existing-value-field>${renderExistingValueField(control, definition.value)}</div>
        <label>Minimum<input name="setting-min" type="number" step="any" value="${escapeHtml(definition.min ?? '')}"></label>
        <label>Maximum<input name="setting-max" type="number" step="any" value="${escapeHtml(definition.max ?? '')}"></label>
        <label>Step<input name="setting-step" type="number" min="0.000001" step="any" value="${escapeHtml(definition.step ?? '')}"></label>
        <label>Minimum length<input name="setting-min-length" type="number" min="0" step="1" value="${escapeHtml(definition.minLength ?? '')}"></label>
        <label>Maximum length<input name="setting-max-length" type="number" min="0" step="1" value="${escapeHtml(definition.maxLength ?? '')}"></label>
        <label>Apply changes<select name="setting-apply-at"><option value="live"${(definition.applyAt || 'live') === 'live' ? ' selected' : ''}>Live</option><option value="restart"${definition.applyAt === 'restart' ? ' selected' : ''}>After restart</option></select></label>
        <label class="builder-field-wide" data-choice-options${isChoice ? '' : ' hidden'}>Choices (JSON object)<textarea name="setting-options" rows="4" placeholder='{"easy":"Easy","hard":"Hard"}'>${escapeHtml(options)}</textarea><small>Use JSON with value and label pairs. Grouped choices may include a label and group.</small></label>
      </div>
      <div class="builder-form-actions"><button type="button" class="text-button" data-cancel-setting-edit="${escapeHtml(key)}">Cancel</button><button type="submit" class="button button-primary">Apply changes</button></div>
    </form>`
  }

  function renderSettingItems() {
    const current = isObject(extended.settings) ? extended.settings : {}
    const keys = Object.keys(current)
    for (const key of expandedSettingKeys) if (!keys.includes(key)) expandedSettingKeys.delete(key)
    const rows = Object.entries(current).map(([key, raw]) => {
      const definition = isObject(raw) ? raw : { value: raw }
      const label = definition.label || key
      const control = definition.control || 'automatic'
      const group = (Array.isArray(extended.groups) ? extended.groups : []).find(item => item?.settings?.includes(key))?.label || ''
      const isOpen = expandedSettingKeys.has(key)
      return `<div class="builder-setting-entry${isOpen ? ' is-open' : ''}"><div class="builder-setting-row"><button type="button" class="builder-item builder-setting-toggle" data-toggle-setting="${escapeHtml(key)}" aria-expanded="${isOpen}"><span class="settings-disclosure-arrow" aria-hidden="true">›</span><span class="builder-item-copy"><strong>${escapeHtml(label)}</strong><small>${escapeHtml(key)} · ${escapeHtml(control)} · ${escapeHtml(group || 'Ungrouped')}</small></span></button><button type="button" class="remove-template" data-remove-setting="${escapeHtml(key)}" aria-label="Remove ${escapeHtml(label)}">×</button></div>${isOpen ? renderExistingSettingEditor(key, definition, group) : ''}</div>`
    })
    if (!rows.length) settingsExpanded = false
    settingItems.innerHTML = rows.length
      ? `<details class="builder-existing-settings"${settingsExpanded ? ' open' : ''}><summary class="builder-items-heading"><span class="settings-disclosure-arrow" aria-hidden="true">›</span><strong>Settings in this mod</strong><span>${rows.length}</span><small data-disclosure-label>${settingsExpanded ? 'Hide' : 'Show'} settings</small></summary>${rows.join('')}</details>`
      : '<p class="builder-empty">No settings yet. Choose a template above to add the first one.</p>'
    const disclosure = settingItems.querySelector('.builder-existing-settings')
    disclosure?.addEventListener('toggle', () => {
      settingsExpanded = disclosure.open
      disclosure.querySelector('[data-disclosure-label]').textContent = `${settingsExpanded ? 'Hide' : 'Show'} settings`
    })
  }

  function renderBadgeCatalog() {
    const selected = isObject(extended.links) ? extended.links : {}
    const links = example.links || []
    for (const id of expandedBadgeIds) if (!Object.hasOwn(selected, id)) expandedBadgeIds.delete(id)
    badgeCatalog.innerHTML = links.map(link => {
      const isSelected = Object.hasOwn(selected, link.id)
      const isExpanded = expandedBadgeIds.has(link.id)
      return `<button type="button" class="template-card badge-template${isSelected ? ' is-selected' : ''}${isExpanded ? ' is-editing' : ''}" data-badge-template="${escapeHtml(link.id)}" aria-label="${isSelected ? 'Open' : 'Add'} ${escapeHtml(link.label)} badge" aria-pressed="${isSelected}" aria-expanded="${isSelected && isExpanded}">
        <span class="badge-template-icon"><img src="${escapeHtml(link.iconUrl)}" alt=""></span>
        <span class="template-card-copy"><strong>${escapeHtml(link.label)}</strong><small>${escapeHtml(link.description)}</small></span>
        <span class="template-add" aria-hidden="true">${isSelected ? checkIcon : plusIcon}</span>
      </button>`
    }).join('')
    const selectedCount = Object.keys(selected).length
    if (!selectedCount) badgesExpanded = false
    const selectedRows = Object.entries(selected).map(([id, rawUrl]) => {
      const link = links.find(item => item.id === id)
      const label = link?.label || id
      const value = typeof rawUrl === 'string' ? rawUrl : ''
      const displayUrl = value.trim() || 'No link set'
      const invalid = value.trim() && !/^https:\/\//i.test(value.trim())
      const isExpanded = expandedBadgeIds.has(id)
      const icon = link
        ? `<img src="${escapeHtml(link.iconUrl)}" alt="">`
        : '<span aria-hidden="true">↗</span>'
      return `<div class="builder-setting-entry builder-badge-entry selected-badge-item${isExpanded ? ' is-open' : ''}"><div class="builder-setting-row"><button type="button" class="builder-item builder-setting-toggle selected-badge-summary" data-toggle-badge="${escapeHtml(id)}" aria-expanded="${isExpanded}" aria-label="${isExpanded ? 'Collapse' : 'Expand'} ${escapeHtml(label)} badge URL"><span class="settings-disclosure-arrow" aria-hidden="true">›</span><span class="selected-badge-mark">${icon}</span><span class="builder-item-copy"><strong>${escapeHtml(label)}</strong><small data-selected-badge-url="${escapeHtml(id)}">${escapeHtml(displayUrl)}</small></span></button><button type="button" class="remove-template" data-remove-badge="${escapeHtml(id)}" aria-label="Remove ${escapeHtml(label)}">×</button></div>${isExpanded ? `<label class="badge-url-field"><span class="badge-url-heading"><span><strong>Link URL</strong><small>links.${escapeHtml(id)}</small></span></span><input data-badge-url="${escapeHtml(id)}" type="url" value="${escapeHtml(value)}" placeholder="https://…" autocomplete="url"><small class="badge-url-help${invalid ? ' field-error' : ''}">${invalid ? 'Use a secure https:// address.' : 'The link is stored under <code>links</code> in extended.mod.json.'}</small></label>` : ''}</div>`
    }).join('')
    const disclosure = selectedCount
      ? `<details class="builder-existing-badges"${badgesExpanded ? ' open' : ''}><summary class="builder-items-heading"><span class="settings-disclosure-arrow" aria-hidden="true">›</span><strong>Selected badges</strong><span>${selectedCount}</span><small data-disclosure-label>${badgesExpanded ? 'Hide' : 'Show'} badges</small></summary><div class="selected-badge-list">${selectedRows}</div></details>`
      : '<p class="builder-empty">No badges selected. Choose a badge above to add its link.</p>'
    badgeFields.innerHTML = disclosure
    const badgeDisclosure = badgeFields.querySelector('.builder-existing-badges')
    badgeDisclosure?.addEventListener('toggle', () => {
      badgesExpanded = badgeDisclosure.open
      badgeDisclosure.querySelector('[data-disclosure-label]').textContent = `${badgesExpanded ? 'Hide' : 'Show'} badges`
    })
  }

  function addSetting(form) {
    const data = new FormData(form)
    const key = String(data.get('setting-key') || '').trim()
    const label = String(data.get('setting-label') || '').trim()
    const control = String(data.get('setting-control') || '')
    if (!/^[A-Za-z0-9._-]{1,80}$/.test(key)) {
      form.querySelector('[name="setting-key"]').setCustomValidity('Use 1 to 80 letters, numbers, dots, hyphens, or underscores.')
      form.querySelector('[name="setting-key"]').reportValidity()
      return
    }
    const settings = isObject(extended.settings) ? extended.settings : {}
    if (Object.hasOwn(settings, key)) {
      form.querySelector('[name="setting-key"]').setCustomValidity('This key already exists.')
      form.querySelector('[name="setting-key"]').reportValidity()
      return
    }
    const rawValue = data.get('setting-value')
    let value = rawValue
    if (['toggle', 'checkbox'].includes(control)) value = form.querySelector('[name="setting-value"]').checked
    else if (['number', 'slider'].includes(control)) value = Number(rawValue || 0)
    else if (control === 'multiselect') value = String(rawValue || '').split(',').map(item => item.trim()).filter(Boolean)
    else if (control === 'color') value = String(rawValue || '#c9ad6e')
    const setting = {
      value,
      label: label || key,
      control,
    }
    const description = String(data.get('setting-description') || '').trim()
    if (description) setting.description = description
    const optionsText = String(data.get('setting-options') || '').trim()
    if (optionsText) {
      const options = {}
      for (const line of optionsText.split(/\r?\n/).map(item => item.trim()).filter(Boolean)) {
        const [option, ...rest] = line.split('=')
        const optionKey = option.trim()
        if (optionKey) options[optionKey] = (rest.join('=').trim() || optionKey)
      }
      if (Object.keys(options).length) {
        setting.options = options
        if (['select', 'radio', 'segmented'].includes(control) && typeof value === 'string') {
          if (!Object.hasOwn(options, value)) value = Object.keys(options)[0]
          setting.value = value
        }
      }
    }
    for (const [formKey, jsonKey] of [['setting-min', 'min'], ['setting-max', 'max'], ['setting-step', 'step']]) {
      const candidate = data.get(formKey)
      if (candidate !== null && String(candidate).trim() !== '') setting[jsonKey] = Number(candidate)
    }
    extended.settings = { ...settings, [key]: setting }
    const groupLabel = String(data.get('setting-group') || 'Settings').trim() || 'Settings'
    extended.groups = Array.isArray(extended.groups) ? extended.groups : []
    let group = extended.groups.find(item => item?.label === groupLabel)
    if (!group) {
      group = { label: groupLabel, settings: [], actions: [] }
      extended.groups.push(group)
    }
    group.settings = Array.isArray(group.settings) ? group.settings : []
    if (!group.settings.includes(key)) group.settings.push(key)
    updateEditor(extendedEditor, extended)
    settingFields.hidden = true
    selectedSettingTemplate = ''
    settingsExpanded = true
    expandedSettingKeys.add(key)
    renderSettingCatalog()
    renderSettingItems()
    publishPreview()
  }

  function readExistingSettingValue(form, control) {
    const field = form.querySelector('[name="setting-value"]')
    if (!field) return ''
    if (field instanceof HTMLInputElement && field.type === 'checkbox') return field.checked
    const raw = field.value
    if (['number', 'slider'].includes(control)) return raw === '' ? 0 : Number(raw)
    if (control === 'multiselect') return raw.split(',').map(item => item.trim()).filter(Boolean)
    if (field.dataset.valueType === 'json' && raw.trim()) return JSON.parse(raw)
    return raw
  }

  function applyExistingSetting(form) {
    const current = parseEditor(extendedEditor, 'extended')
    if (!current) return
    const key = form.dataset.editSetting
    const settings = isObject(current.settings) ? current.settings : {}
    const rawPrevious = settings[key]
    const previous = isObject(rawPrevious) ? rawPrevious : { value: rawPrevious }

    let value
    try {
      value = readExistingSettingValue(form, form.elements.namedItem('setting-control').value)
      const control = form.elements.namedItem('setting-control').value
      const isChoice = ['select', 'radio', 'segmented', 'multiselect'].includes(control)
      const optionsText = isChoice ? String(form.elements.namedItem('setting-options')?.value || '').trim() : ''
      if (optionsText) {
        const options = JSON.parse(optionsText)
        if (!isObject(options) || Array.isArray(options)) throw new Error('Choices must be a JSON object.')
        previous.options = options
      } else {
        delete previous.options
      }
    } catch (error) {
      const optionsField = form.elements.namedItem('setting-options')
      if (optionsField) {
        optionsField.setCustomValidity(error.message)
        optionsField.reportValidity()
      }
      return
    }

    const fieldValue = name => String(form.elements.namedItem(name)?.value || '').trim()
    const updated = { ...previous, value, control: form.elements.namedItem('setting-control').value }
    updated.label = fieldValue('setting-label') || key
    const description = fieldValue('setting-description')
    if (description) updated.description = description
    else delete updated.description
    for (const [formKey, jsonKey] of [['setting-min', 'min'], ['setting-max', 'max'], ['setting-step', 'step'], ['setting-min-length', 'minLength'], ['setting-max-length', 'maxLength']]) {
      const raw = fieldValue(formKey)
      if (raw) updated[jsonKey] = Number(raw)
      else delete updated[jsonKey]
    }
    updated.applyAt = form.elements.namedItem('setting-apply-at').value
    current.settings = { ...settings, [key]: updated }
    current.groups = Array.isArray(current.groups) ? current.groups : []
    for (const group of current.groups) {
      if (Array.isArray(group.settings)) group.settings = group.settings.filter(item => item !== key)
    }
    const groupLabel = fieldValue('setting-group')
    if (groupLabel) {
      let group = current.groups.find(item => item?.label === groupLabel)
      if (!group) {
        group = { label: groupLabel, settings: [], actions: [] }
        current.groups.push(group)
      }
      group.settings = Array.isArray(group.settings) ? group.settings : []
      if (!group.settings.includes(key)) group.settings.push(key)
    }

    extended = current
    updateEditor(extendedEditor, extended)
    form.querySelector('[name="setting-options"]').setCustomValidity('')
    renderSettingItems()
    publishPreview()
  }

  function changeManifestField(node) {
    const current = parseEditor(manifestEditor, 'manifest')
    if (!current) return
    const key = node.dataset.manifestField
    const value = node.value.trim()
    if (key === 'authors') {
      const authors = value.split(',').map(item => item.trim()).filter(Boolean)
      if (authors.length) current.authors = authors
      else delete current.authors
    } else if (value) current[key] = value
    else delete current[key]
    manifest = current
    updateEditor(manifestEditor, manifest)
    publishPreview()
  }

  function changeManifestDependency(node) {
    const current = parseEditor(manifestEditor, 'manifest')
    if (!current) return
    const index = Number(node.dataset.dependencyIndex)
    const key = node.dataset.dependencyField
    if (!Number.isInteger(index) || index < 0 || !['id', 'version', 'optional'].includes(key)) return
    if (!Array.isArray(current.dependencies) || !isObject(current.dependencies[index])) return
    current.dependencies[index][key] = key === 'optional' ? node.checked : node.value.trim()
    manifest = current
    updateEditor(manifestEditor, manifest)
    publishPreview()
  }

  manifestEditor.value = `${JSON.stringify(manifest, null, 2)}\n`
  extendedEditor.value = `${JSON.stringify(extended, null, 2)}\n`
  renderLineNumbers(manifestEditor, 'manifest')
  renderLineNumbers(extendedEditor, 'extended')
  renderManifestFields(manifest)
  renderSettingCatalog()
  renderSettingItems()
  renderBadgeCatalog()
  publishPreview()

  for (const editor of [manifestEditor, extendedEditor]) {
    const key = editor.dataset.jsonEditor
    editor.addEventListener('input', () => {
      renderLineNumbers(editor, key)
      updatePreviewSoon()
    })
    editor.addEventListener('scroll', () => renderLineNumbers(editor, key))
    editor.addEventListener('change', () => {
      if (readEditors()) {
        renderManifestFields(manifest)
        renderSettingItems()
        renderBadgeCatalog()
        renderSettingCatalog()
      }
    })
  }

  tool.addEventListener('input', event => {
    const node = event.target
    if (!(node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement)) return
    if (node.matches('[data-manifest-field]')) changeManifestField(node)
    if (node.matches('[data-dependency-field="id"], [data-dependency-field="version"]')) changeManifestDependency(node)
    if (node.matches('[name="setting-options"]')) node.setCustomValidity('')
    if (node.matches('[data-badge-url]')) {
      const current = parseEditor(extendedEditor, 'extended')
      if (!current) return
      current.links = isObject(current.links) ? current.links : {}
      current.links[node.dataset.badgeUrl] = node.value.trim()
      extended = current
      updateEditor(extendedEditor, extended)
      const selectedUrl = [...badgeFields.querySelectorAll('[data-selected-badge-url]')].find(item => item.dataset.selectedBadgeUrl === node.dataset.badgeUrl)
      if (selectedUrl) selectedUrl.textContent = node.value.trim() || 'No link set'
      const helper = node.parentElement.querySelector('.badge-url-help')
      if (helper) {
        const invalid = node.value.trim() && !/^https:\/\//i.test(node.value.trim())
        helper.innerHTML = invalid
          ? 'Use a secure https:// address.'
          : 'The link is stored under <code>links</code> in extended.mod.json.'
        helper.classList.toggle('field-error', Boolean(invalid))
      }
      publishPreview()
    }
  })

  tool.addEventListener('change', event => {
    const node = event.target
    if (node instanceof HTMLInputElement && node.matches('[data-dependency-field="optional"]')) {
      changeManifestDependency(node)
      return
    }
    if (!(node instanceof HTMLSelectElement) || !node.matches('[data-existing-control]')) return
    const form = node.closest('[data-existing-setting-form]')
    if (!form) return
    const previousControl = form.dataset.valueControl
    let value
    try {
      value = readExistingSettingValue(form, previousControl)
    } catch {
      value = form.querySelector('[name="setting-value"]')?.value || ''
    }
    form.dataset.valueControl = node.value
    form.querySelector('[data-existing-value-field]').innerHTML = renderExistingValueField(node.value, value)
    form.querySelector('[data-choice-options]').hidden = !['select', 'radio', 'segmented', 'multiselect'].includes(node.value)
  })

  tool.addEventListener('click', event => {
    const target = event.target instanceof Element ? event.target.closest('button') : null
    if (!target) return
    if (target.matches('[data-download-json]')) {
      const downloads = {
        manifest: [manifestEditor, 'manifest', 'mod.json'],
        extended: [extendedEditor, 'extended', 'extended-mod.json'],
      }
      const download = downloads[target.dataset.downloadJson]
      if (download) downloadJson(...download)
      return
    }
    if (target.matches('[data-add-manifest-dependency]')) {
      const current = parseEditor(manifestEditor, 'manifest')
      if (!current) return
      current.dependencies = Array.isArray(current.dependencies) ? current.dependencies : []
      current.dependencies.push({ id: '', version: '', optional: false })
      const index = current.dependencies.length - 1
      manifest = current
      updateEditor(manifestEditor, manifest)
      renderManifestDependencies(manifest)
      manifestDependencies.querySelector(`[data-dependency-index="${index}"][data-dependency-field="id"]`)?.focus()
      publishPreview()
      return
    }
    if (target.matches('[data-remove-dependency]')) {
      event.preventDefault()
      event.stopPropagation()
      const current = parseEditor(manifestEditor, 'manifest')
      const index = Number(target.dataset.removeDependency)
      if (!current || !Array.isArray(current.dependencies) || !Number.isInteger(index)) return
      current.dependencies.splice(index, 1)
      if (!current.dependencies.length) delete current.dependencies
      manifest = current
      updateEditor(manifestEditor, manifest)
      renderManifestDependencies(manifest)
      publishPreview()
      return
    }
    if (target.matches('[data-manifest-capability]')) {
      const current = parseEditor(manifestEditor, 'manifest')
      if (!current) return
      const capability = target.dataset.manifestCapability
      const selected = new Set(Array.isArray(current.capabilities) ? current.capabilities : [])
      if (selected.has(capability)) selected.delete(capability)
      else selected.add(capability)
      if (selected.size) current.capabilities = manifestCapabilityOptions.filter(item => selected.has(item))
      else delete current.capabilities
      manifest = current
      updateEditor(manifestEditor, manifest)
      renderManifestCapabilities(manifest)
      publishPreview()
      return
    }
    if (target.matches('[data-setting-template]')) {
      renderSettingFields(target.dataset.settingTemplate)
      renderSettingCatalog()
      return
    }
    if (target.matches('[data-close-setting-template]')) {
      selectedSettingTemplate = ''
      settingFields.hidden = true
      renderSettingCatalog()
      return
    }
    if (target.matches('[data-cancel-setting-edit]')) {
      expandedSettingKeys.delete(target.dataset.cancelSettingEdit)
      renderSettingItems()
      return
    }
    if (target.matches('[data-toggle-setting]')) {
      const key = target.dataset.toggleSetting
      if (expandedSettingKeys.has(key)) expandedSettingKeys.delete(key)
      else expandedSettingKeys.add(key)
      renderSettingItems()
      return
    }
    if (target.matches('[data-badge-template]')) {
      const current = parseEditor(extendedEditor, 'extended')
      if (!current) return
      current.links = isObject(current.links) ? current.links : {}
      const id = target.dataset.badgeTemplate
      if (!Object.hasOwn(current.links, id)) {
        current.links[id] = ''
        updateEditor(extendedEditor, current)
      }
      extended = current
      expandedBadgeIds.add(id)
      badgesExpanded = true
      renderBadgeCatalog()
      publishPreview()
      return
    }
    if (target.matches('[data-toggle-badge]')) {
      const id = target.dataset.toggleBadge
      if (expandedBadgeIds.has(id)) expandedBadgeIds.delete(id)
      else expandedBadgeIds.add(id)
      renderBadgeCatalog()
      return
    }
    if (target.matches('[data-remove-badge]')) {
      event.preventDefault()
      event.stopPropagation()
      const current = parseEditor(extendedEditor, 'extended')
      if (!current?.links) return
      const id = target.dataset.removeBadge
      delete current.links[id]
      expandedBadgeIds.delete(id)
      extended = current
      updateEditor(extendedEditor, extended)
      renderBadgeCatalog()
      publishPreview()
      return
    }
    if (target.matches('[data-remove-setting]')) {
      event.preventDefault()
      event.stopPropagation()
      const current = parseEditor(extendedEditor, 'extended')
      if (!current?.settings) return
      const key = target.dataset.removeSetting
      delete current.settings[key]
      expandedSettingKeys.delete(key)
      for (const group of current.groups || []) {
        group.settings = (group.settings || []).filter(item => item !== key)
      }
      extended = current
      updateEditor(extendedEditor, extended)
      renderSettingItems()
      publishPreview()
      return
    }
    if (target.matches('[data-reset-example]')) {
      manifest = clone(example.manifest)
      extended = clone(example.extended)
      selectedSettingTemplate = ''
      settingsExpanded = false
      badgesExpanded = false
      expandedSettingKeys.clear()
      expandedBadgeIds.clear()
      settingFields.hidden = true
      updateSchemaReferences()
      manifestEditor.value = `${JSON.stringify(manifest, null, 2)}\n`
      extendedEditor.value = `${JSON.stringify(extended, null, 2)}\n`
      renderLineNumbers(manifestEditor, 'manifest')
      renderLineNumbers(extendedEditor, 'extended')
      renderManifestFields(manifest)
      renderSettingCatalog()
      renderSettingItems()
      renderBadgeCatalog()
      publishPreview()
    }
  })

  tool.addEventListener('submit', event => {
    const form = event.target
    if (!(form instanceof HTMLFormElement)) return
    event.preventDefault()
    if (!form.reportValidity()) return
    if (form.matches('[data-existing-setting-form]')) applyExistingSetting(form)
    else if (form.matches('[data-setting-form]')) addSetting(form)
  })

  window.addEventListener('message', event => {
    if (event.origin !== location.origin || event.source !== preview.contentWindow) return
    const data = event.data
    if (!data || typeof data !== 'object') return
    if (data.source === 'shroudforge-mod-settings-preview' && data.type === 'resize' && Number.isFinite(data.height)) {
      preview.style.height = `${Math.max(430, Math.ceil(data.height) + 2)}px`
      return
    }
    if (data.source === 'shroudforge-mod-settings-preview' && data.type === 'ready') {
      publishPreview()
      return
    }
    if (data.source !== 'shroudforge-mod-settings-preview') return
    const current = parseEditor(extendedEditor, 'extended')
    if (!current) return
    if (data.type === 'enabled-change') current.enabled = data.enabled === true
    if (data.type === 'settings-change' && isObject(data.values)) {
      current.settings = isObject(current.settings) ? current.settings : {}
      for (const [key, value] of Object.entries(data.values)) {
        const previous = current.settings[key]
        current.settings[key] = isObject(previous)
          ? { ...previous, value }
          : value
      }
    }
    extended = current
    updateEditor(extendedEditor, extended)
    renderSettingItems()
    renderBadgeCatalog()
    publishPreview()
  })

  versionSelect.addEventListener('change', () => {
    currentVersion = versionSelect.value
    if (exampleSourceLink?.dataset.versionUrlTemplate) {
      exampleSourceLink.href = exampleSourceLink.dataset.versionUrlTemplate.replace('{version}', encodeURIComponent(currentVersion))
    }
    updateSchemaReferences()
    publishPreview()
  })
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
