# GitHub Pages

This folder contains the complete source for the legacy GitHub Pages address. It keeps the previous `/en/` and `/de/` URLs and every old `#doc-*` article fragment, but publishes one small English redirect screen instead of the retired documentation site.

`redirect-routes.json` is the route table. It preserves all 177 former article IDs and the previous named view fragments. Known pages link to their current Wiki or ShroudForge destination. The fallback points to the Wiki main page.

Build the Pages artifact from the repository root:

```powershell
npm run githubpages:build
```

The generated `build/githubpages/` contains only the reusable page, its CSS and JavaScript, the route map, and the hammer icon. The screen shows the current legacy URL and a clickable destination. It does not redirect automatically.
