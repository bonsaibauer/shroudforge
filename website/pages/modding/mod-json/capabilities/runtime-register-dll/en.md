# The `runtime-register-dll` capability

Register a package-local DLL during the runtime phase. Add the value to `capabilities` in `mod.json` only when the mod uses that feature.

```json
{
  "capabilities": ["runtime-register-dll"]
}
```

Declaring a capability does not report success. The loader checks the mod and the feature separately. See [the mod.json capabilities field](#doc-field-capabilities).
