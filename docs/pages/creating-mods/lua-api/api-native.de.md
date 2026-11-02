---
{
  "id": "api-native",
  "locale": "de",
  "title": "Die eigene DLL des Mods aus Lua aufrufen",
  "summary": "Eine Mod-DLL kann Funktionen registrieren, die der Lua-Code desselben Mods über shroudforge.native aufruft.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen",
      "Funktionsanleitungen"
    ]
  },
  "apiPrefixes": [
    "shroudforge.native."
  ]
}
---

# Die eigene DLL des Mods aus Lua aufrufen

Ein Mod kann eine DLL in seinem Ordner mitliefern, angegeben in `native-plugin.ini` oder registriert mit `loader.runtime.register_dll`. ShroudForge lädt sie einmal pro Spielprozess. Exportiert die DLL `ShroudforgeModNativeInit`, ruft ShroudForge diese Funktion direkt nach dem Laden auf. Dort registriert die DLL benannte Funktionen, und der Lua-Code desselben Mods ruft sie über `shroudforge.native` auf.

Die Brücke verbindet zwei Teile desselben Mods. Ein Mod erreicht nur die Funktionen, die seine eigenen DLLs registriert haben. Weder die DLL noch der Lua-Code erhalten dadurch Zugriff auf das Spiel oder auf andere Mods. Eine DLL ohne diesen Export wird genau wie bisher geladen.

## Die Seite der DLL

Kopiere `modules/mod-runtime/native_bridge/shroudforge_mod_native.h` aus dem ShroudForge-Repository in dein DLL-Projekt. Es ist ein einfacher C-Header.

```c
#include "shroudforge_mod_native.h"

static void SHROUDFORGE_MOD_NATIVE_CALL add(void* context,
    const ShroudforgeNativeValue* arguments, size_t count,
    ShroudforgeNativeResults* results)
{
    if (count != 2 || arguments[0].kind != SHROUDFORGE_NATIVE_INTEGER
        || arguments[1].kind != SHROUDFORGE_NATIVE_INTEGER) {
        results->fail(results, "add expects two integers");
        return;
    }
    ShroudforgeNativeValue sum = { SHROUDFORGE_NATIVE_INTEGER };
    sum.as.integer = arguments[0].as.integer + arguments[1].as.integer;
    results->push(results, &sum);
}

SHROUDFORGE_MOD_NATIVE_EXPORT int32_t SHROUDFORGE_MOD_NATIVE_CALL
ShroudforgeModNativeInit(const ShroudforgeModHost* host)
{
    if (host->abi_version < 1) return 0;
    host->log(host, SHROUDFORGE_NATIVE_LOG_INFO, "native part ready");
    return host->register_function(host, "add", add, NULL) == 1;
}
```

`host` bleibt bis zum Ende des Prozesses gültig. Er liefert die ID und den Ordner des besitzenden Mods, eine Funktion `log`, die aus jedem Thread unter dieser ID ins ShroudForge-Log schreibt, und `register_function`, die nur während `ShroudforgeModNativeInit` funktioniert. Gibt der Export null zurück, meldet der DLL-Manager die DLL als fehlgeschlagen, und ihre Funktionen werden zurückgezogen.

## Die Seite von Lua

```lua
local sum, reason = shroudforge.native.call("add", 2, 40)
if sum == nil then
  shroudforge.log.warn("native add failed: " .. reason)
end
```

Erlaubte Argumente sind nil, Wahrheitswerte, Ganzzahlen, Zahlen, Strings und Buffer. Strings und Buffer erreichen die native Funktion ohne Kopie und sind nur während des Aufrufs gültig. Die Funktion gibt bis zu 32 Werte zurück. Zurückgegebene Bytes kommen als Lua-Strings an. Eine Funktion ohne Rückgabewert liefert `true`. Ein Fehler oder ein Name, den die DLL nicht registriert hat, liefert `nil` und einen Grund. `shroudforge.native.functions()` listet die registrierten Namen.

## Threads und Zeitverhalten

Ein Aufruf ist synchron. Er läuft im Thread der Mod-Laufzeit, nie im Spielthread, und das Update des Mods wartet auf ihn. Halte native Funktionen kurz. Lange Arbeit läuft in Threads, die die DLL selbst startet. Lua fragt das Ergebnis bei einem späteren Update ab, wie bei anderen ShroudForge-Operationen mit einem ausstehenden Zustand.

Funktionen gehören wie die DLL selbst zum Prozess. Sie überstehen ein Neuladen des Lua-Codes der Mods, sodass ein neu geladener Mod sie wiederfindet.

Eine native Funktion darf keine Ausnahme werfen und nicht in ShroudForge hinein abwickeln. Ein Absturz in der DLL beendet den Spielprozess wie jeder andere native Absturz.
