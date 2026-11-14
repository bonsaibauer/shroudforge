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
