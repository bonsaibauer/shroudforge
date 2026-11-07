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

Die Brücke verbindet zwei Teile desselben Mods. Ein Mod erreicht nur die Funktionen, die seine eigenen DLLs registriert haben, nie die eines anderen Mods. Die einzigen Spieldaten, die der Host der DLL gibt, sind die lokale Kamera, nur lesend und unter denselben Bedingungen wie die Lua-API (siehe unten). Eine DLL ohne diesen Export wird genau wie bisher geladen.

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

## Die lokale Kamera

Eine DLL, die jedes Bild zeichnet, zum Beispiel als Vulkan-Layer, kann nicht auf das Lua-Update des Mods warten, um zu erfahren, wohin der Spieler blickt. Seit ABI 2 liest `host->read_local_camera` die Kamera des lokalen Spielers aus derselben Eingabeprobe wie `runtime.world.cursor.get`, in jedem Thread:

```c
double view[SHROUDFORGE_MOD_NATIVE_VIEW_VALUES];
if (host->abi_version >= 2
    && host->read_local_camera(host, view, SHROUDFORGE_MOD_NATIVE_VIEW_VALUES) == 1) {
    /* view[0..2] position in meters, view[3..6] orientation,
       view[7] fovY, view[8] aspect ratio, then the building cursor. */
}
```

Sie antwortet nur, solange der Mod ein aktiver Ingame-Laufzeitmod mit der Fähigkeit `runtime` in einem Spielclient ist. Das sind die Bedingungen der Lua-Laufzeit-API. Ergebnis 7 bedeutet, dass der Mod in diesem Moment keinen Laufzeitzugriff hat. Die Feldanordnung stammt aus der Reflexion des Spiels und ist bekannt, sobald ein Aufruf von `runtime.world.cursor.get` die Kamera dekodiert hat. Bis dahin ist Ergebnis 3 zu erwarten. Der Header listet alle Ergebniscodes.
