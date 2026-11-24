document.addEventListener('click', async event => {
	const button = event.target instanceof Element ? event.target.closest('[data-copy-schema]') : null
	if (!button) return
	const status = button.closest('.schema-card')?.querySelector('.schema-copy-status')
	if (!status) return
	try {
		await navigator.clipboard.writeText(button.dataset.copySchema)
		status.textContent = 'Schema URL copied.'
	} catch {
		status.textContent = 'Copy is unavailable. Copy the link address instead.'
	}
})

const versionPicker = document.querySelector('[data-schema-version]')
if (versionPicker) {
	const versionSections = [...document.querySelectorAll('[data-schema-version-section]')]
	const manifestExample = document.querySelector('[data-schema-manifest-example]')
	const sourceLink = document.querySelector('[data-schema-source-link]')

	versionPicker.addEventListener('change', () => {
		const selectedOption = versionPicker.selectedOptions[0]
		const selectedVersion = versionPicker.value
		for (const section of versionSections) {
			section.hidden = section.dataset.schemaVersionSection !== selectedVersion
		}
		if (manifestExample && selectedOption?.dataset.manifestSchemaUrl) {
			manifestExample.textContent = `"$schema": "${selectedOption.dataset.manifestSchemaUrl}"`
		}
		if (sourceLink && selectedOption) {
			sourceLink.href = selectedOption.dataset.sourceUrl || '#'
		}
	})
}

const blueprintVersionPicker = document.querySelector('[data-blueprint-version]')
if (blueprintVersionPicker) {
	const sections = [...document.querySelectorAll('[data-blueprint-version-section]')]
	const url = document.querySelector('[data-blueprint-url]')
	blueprintVersionPicker.addEventListener('change', () => {
		for (const section of sections) {
			section.hidden = section.dataset.blueprintVersionSection !== blueprintVersionPicker.value
		}
		const selectedLink = sections.find(section => !section.hidden)?.querySelector('a')
		if (url && selectedLink) url.textContent = selectedLink.href
	})
}
