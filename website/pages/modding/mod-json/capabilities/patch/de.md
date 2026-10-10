# Berechtigung `patch`

Vorbereitung unterstützter Spieldateien vor dem Start. Trage den Wert in das Feld `capabilities` in `mod.json` ein, wenn der Mod die Funktion wirklich nutzt.

```json
{
  "capabilities": ["patch"]
}
```

Die Deklaration ist keine Erfolgsmeldung. Der Loader prüft den Mod und die jeweilige Funktion zusätzlich. Siehe [Berechtigungen in mod.json](#doc-field-capabilities).
