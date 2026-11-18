# API catalogue snapshots

The current API catalogue is generated from `modules/mod-runtime/lua/**/definitions` during `npm run website:api-build`.

When the `shroudforge-api` SemVer changes, preserve the previous published catalogue here before removing its source revision:

```text
website/api-snapshots/<api-version>/catalog.json
website/api-snapshots/<api-version>/examples.json
website/api-snapshots/<api-version>/runtime-operations.json
```

These are immutable generated publication artifacts. Do not hand-edit signatures or copy the current catalogue under a different version. The website build validates each folder name against the version inside its `catalog.json` and lists only snapshots that are actually present.
