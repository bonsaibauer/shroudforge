---
{
  "id": "mod-json-guide",
  "locale": "en",
  "title": "mod.json, your mod's information card",
  "summary": "This file declares the mod ID, name, version, and requested capabilities. It belongs in the root folder of your mod.",
  "navigation": {
    "path": [
      "mod.json"
    ],
    "overview": true,
    "label": "Overview"
  }
}
---

# mod.json, your mod's information card

Every mod needs a `mod.json`. ShroudForge reads it before preparing or starting a mod. The file describes the mod and the capabilities it requests.

## Required fields

`id`, `name`, and `version` are required. Every other field is optional. Each property has its own page in this group.

## The file described here

The canonical schema is [manifest.schema.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The loader also checks dependencies and runtime conditions in [read_mod_manifest.rs](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/read_mod_manifest.rs).

## A small example

```json
{
  "id": "example.hello-ember",
  "name": "Hello Ember",
  "version": "1.0.0",
  "authors": ["Your Name"],
  "capabilities": ["runtime"],
  "dependencies": []
}
```

## Related topics

- [The optional extended.mod.json](#doc-extended-mod-json-guide) holds settings and Modloader actions.
- [Build a mod step by step](#first) walks through a complete small package.
- [Understand and migrate EML mods](#doc-eml-migration) explains provenance and differences.
