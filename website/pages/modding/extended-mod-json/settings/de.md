# Feld `settings`

Dynamische Schlüssel und Werte, die der Modloader als Mod-Einstellungen darstellt und Lua lesen kann.


Jeder Schlüssel ist mod-eigen und muss dem Schlüssel entsprechen, den Lua an `shroudforge.settings.get(key, fallback)` übergibt. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls), [settings.<key>](#doc-extension-setting-key) und [alle Metadatenfelder](#doc-field-setting-key-value).
