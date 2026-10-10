# The `export` capability

Export functions for mod output. Add the value to `capabilities` in `mod.json` only when the mod uses that feature.

```json
{
  "capabilities": ["export"]
}
```

Declaring a capability does not report success. The loader checks the mod and the feature separately. See [the mod.json capabilities field](#doc-field-capabilities).
