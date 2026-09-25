# Mod link badge registry

Each link badge has one JSON definition named after its `id`. Definitions set
Crowdin translation keys for its label and short tooltip description, the
matching SVG icon filename, and the appearance class used by the UI. The actual
translated strings live only in the UI locale catalogs. `order.json` is the single display
order for every badge; populated links render in this sequence in one continuous
row, with wrapping only when the available width requires it.

To add a badge, add its definition JSON, list its id in `order.json`, and add
its SVG under `assets/`. Mod manifests store only direct HTTPS URL fields whose
names match those ids, for example `"support-bmac": "https://…"`.
