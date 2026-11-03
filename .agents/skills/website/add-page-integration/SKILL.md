---
name: add-page-integration
description: Add page-specific interactive behavior to an existing published website page through its automatically discovered mount-page.js module, localized DOM/templates, and page-scoped controls.
---

# Add a Website Page Integration

Read `.agents/skills/repository/repository-working-rules.md`, the complete paired article, `docs/website/add-and-order-website-pages.en.md`, `docs/website/page-integration-loader.js`, and the closest existing integration.

Use the exact document page ID. Create `docs/website/page-integrations/<page-id>/mount-page.js` beside that integration's behavior and assets. The build discovers this convention automatically and rejects integrations without a page or entrypoint. Add no registration list.

Export `async function mountPage({ root, page, locale, copy, esc, state, copyText })`. The existing loader calls it lazily after ordinary article loading. Keep selectors, handlers, and transient page state scoped to the supplied root. Use the existing escaping and clipboard helpers. Report missing required elements clearly.

Keep localized templates and page assets in this same directory. Resolve imported/fetched assets relative to `import.meta.url`. Shared navigation and rendering remain in their current owners.

The loader deduplicates loads and mounts by page ID and retains page roots across navigation. Avoid duplicate listeners and invented unmount contracts. Ordinary Markdown escapes HTML, so create localized DOM inside the supplied root. An intentional complete `view` may instead supply its slot in both localized Markdown bodies.

The existing manifest editor at `docs/website/page-integrations/website-manifests/` demonstrates the complete-view route. Its example builder, independent validator, renderer, and package download are separate consumers. A downloaded manifest ZIP is not automatically a complete runnable Lua mod.

Run `npm run site:check` or `./dev/build/check_website.ps1`. Verify the generated page entry points to `page-integrations/<page-id>/mount-page.js` and that required JavaScript files and templates reach `build/website/`. Structural validation does not execute control handlers. When functional verification is requested, check both locales, control effects, navigation away/back, repeated opening, and browser errors.

Report page/integration sources, ID, assets, build result, and actual functional checks.
