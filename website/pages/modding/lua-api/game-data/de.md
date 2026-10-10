# Spielressourcen, ECS und reflektierte Datentypen

Die Lua-API enthält mehrere Bereiche, die sich nach dem Zeitpunkt und der Art des Zugriffs unterscheiden. Die [API-Referenz](#api) listet alle Funktionen und Typen.

## Assets vorbereiten

`game.assets` liest und verändert unterstützte Spielressourcen in der Vorbereitungsphase. Änderungen werden für einen späteren Spielstart vorbereitet. Sie werden nicht sofort in eine bereits laufende Welt übernommen. Deklariere die passende `patch`- oder `export`-Fähigkeit und prüfe die verfügbaren Ressourcentypen des aktiven Profils.

## Laufzeit und ECS

`runtime.ecs` arbeitet während des Spiels mit unterstützten Komponenten. Der Loader prüft Profil, Typen und Ausführungsphase. Verwende nur dokumentierte Abfragen und Schreibfunktionen und prüfe ihren Rückgabestatus. Eine bestätigte Speicheroperation ist kein unabhängiger Nachweis der sichtbaren Spielwirkung.

## Typen und Datenwerte

`game.types` und `runtime.types` stellen die reflektierten Typinformationen des aktiven Spielprofils bereit. `runtime.values` kann eigene Bytefolgen gemäß einem bekannten Typ lesen oder erzeugen. Das Dereferenzieren beliebiger Engine-Zeiger ist nicht Teil dieses sicheren Wertezugriffs.

## Warum ein Spielupdate wichtig ist

Die verfügbaren Typen und Operationen hängen vom installierten Enshrouded-Build und Profil ab. Ein Mod kann vorhanden und geladen sein, obwohl ein bestimmter nativer Zugriff nicht unterstützt wird. Prüfe `runtime.has(feature)` und den Status der konkreten Funktion.
