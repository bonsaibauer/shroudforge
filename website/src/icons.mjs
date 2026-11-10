const paths = {
	download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5M12 15V3"/>',
	external: '<path d="M15 3h6v6M10 14 21 3"/><path d="M21 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6"/>',
	file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 2v6h6M8 13h8M8 17h5"/>',
	'book-open': '<path d="M12 7v14"/><path d="M3 18V5a2 2 0 0 1 2-2h3a4 4 0 0 1 4 4 4 4 0 0 1 4-4h3a2 2 0 0 1 2 2v13a1 1 0 0 1-1 1h-4a4 4 0 0 0-4 2 4 4 0 0 0-4-2H4a1 1 0 0 1-1-1Z"/>',
	copy: '<rect x="8" y="8" width="14" height="14" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/>',
	search: '<circle cx="11" cy="11" r="7"/><path d="m16 16 5 5"/>',
	check: '<path d="m5 12 4 4L19 6"/>',
	alert: '<path d="m10.3 3.9-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.7-3.1l-8-14a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4m0 4h.01"/>',
	minus: '<circle cx="12" cy="12" r="9"/><path d="M8 12h8"/>',
	question: '<circle cx="12" cy="12" r="9"/><path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3m.1 4h.01"/>',
	chevron: '<path d="m9 5 7 7-7 7"/>',
	shield: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11"/><path d="m8 12 3 3 5-6"/>',
	github: '<path d="M9 19c-4.3 1.3-4.3-2-6-2m12 5v-3.9a3.4 3.4 0 0 0-.9-2.7c3-.3 6.1-1.5 6.1-6.8a5.3 5.3 0 0 0-1.5-3.7 4.9 4.9 0 0 0-.1-3.7s-1.2-.4-3.8 1.4a13 13 0 0 0-6.8 0C5.4.8 4.2 1.2 4.2 1.2A4.9 4.9 0 0 0 4.1 5a5.3 5.3 0 0 0-1.5 3.7c0 5.3 3.1 6.5 6.1 6.8a3.4 3.4 0 0 0-.9 2.6V22"/>',
	discord: '<path d="M8 5a16 16 0 0 1 8 0l1-2 3 1c2 3 3 7 3 11a16 16 0 0 1-6 4l-1-2a14 14 0 0 0 2-1 14 14 0 0 1-12 0l2 1-1 2a16 16 0 0 1-6-4C1 11 2 7 4 4l3-1 1 2Z"/><path d="M8 11v2m8-2v2"/>',
	youtube: '<rect x="2" y="5" width="20" height="14" rx="4"/><path d="m10 9 5 3-5 3Z"/>',
	branch: '<circle cx="6" cy="5" r="3"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="19" r="3"/><path d="M6 8v8M18 9a9 9 0 0 1-9 9"/>',
}

export function icon(name, className = '') {
	return `<svg class="icon ${className}" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name] || paths.file}</svg>`
}
