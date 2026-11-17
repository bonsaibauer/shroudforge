document.addEventListener('click', async event => {
	const button = event.target instanceof Element ? event.target.closest('[data-copy-schema]') : null
	if (!button) return
	const status = button.closest('.schema-card')?.querySelector('.schema-copy-status')
	if (!status) return
	try {
		await navigator.clipboard.writeText(button.dataset.copySchema)
		status.textContent = 'Schema URL copied.'
	} catch {
		status.textContent = 'Copy is unavailable. Select the JSON URL above.'
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
