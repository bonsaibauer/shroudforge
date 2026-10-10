# Berechtigung `export`

Exportfunktionen für Mod-Ausgaben. Trage den Wert in das Feld `capabilities` in `mod.json` ein, wenn der Mod die Funktion wirklich nutzt.

```json
{
  "capabilities": ["export"]
}
```

Die Deklaration ist keine Erfolgsmeldung. Der Loader prüft den Mod und die jeweilige Funktion zusätzlich. Siehe [Berechtigungen in mod.json](#doc-field-capabilities).
