# mod.json, your mod's information card

Every mod needs a `mod.json`. ShroudForge reads it before preparing or starting a mod. The file describes the mod and the capabilities it requests.

## Required fields

`id`, `name`, and `version` are required. Every other field is optional. Each property has its own page in this group.

## The file described here

The canonical schema is [manifest.schema.json](../../../schemas/manifest.schema.json). The loader also checks dependencies and runtime conditions in [manifest_reader.rs](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest_reader.rs).

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
