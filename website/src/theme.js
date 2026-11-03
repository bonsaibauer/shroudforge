(() => {
	const storageKey = 'shroudforge-theme'
	const root = document.documentElement
	let theme = 'light'
	try {
		const savedTheme = localStorage.getItem(storageKey)
		if (savedTheme === 'light' || savedTheme === 'dark') theme = savedTheme
	} catch {}

	function applyTheme(value) {
		theme = value
		root.dataset.theme = theme
		document.querySelector('meta[name="theme-color"]')?.setAttribute('content', theme === 'light' ? '#ffffff' : '#050b14')
		document.querySelector('meta[name="color-scheme"]')?.setAttribute('content', theme)
		document.querySelector('#theme-toggle')?.setAttribute('aria-checked', String(theme === 'light'))
	}

	applyTheme(theme)

	function connectToggle() {
		const toggle = document.querySelector('#theme-toggle')
		if (!toggle) return
		toggle.setAttribute('aria-checked', String(theme === 'light'))
		toggle.addEventListener('click', () => {
			applyTheme(theme === 'dark' ? 'light' : 'dark')
			try {
				localStorage.setItem(storageKey, theme)
			} catch {}
		})
		toggle.hidden = false
	}

	if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', connectToggle, { once: true })
	else connectToggle()
})()
