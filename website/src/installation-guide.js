(() => {
	const button = document.querySelector('#copy-installation-path')
	const path = document.querySelector('#installation-path')
	const label = button?.querySelector('.copy-path-label')
	if (!button || !path || !label) return

	let resetTimer
	button.addEventListener('click', async () => {
		clearTimeout(resetTimer)
		try {
			if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable')
			await navigator.clipboard.writeText(path.textContent.trim())
			label.textContent = 'Copied'
			button.dataset.state = 'success'
		} catch {
			label.textContent = 'Copy unavailable'
			button.dataset.state = 'error'
		}
		resetTimer = window.setTimeout(() => {
			label.textContent = 'Copy path'
			button.removeAttribute('data-state')
		}, 1800)
	})
})()
