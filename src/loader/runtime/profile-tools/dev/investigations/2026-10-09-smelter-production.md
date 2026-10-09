# Schmelze: Rezeptdauer und Rohstoffverbrauch

## Ergebnis

Die Installationen enthalten unterschiedliche Zeitwerte in denselben Rezepten.
Die beiden Inventar-Mods ändern diese Werte nicht. Im zuletzt beobachteten
Serverlauf funktioniert der Nullkosten-Eingriff auch bei einer Schmelze.
Die zuvor berichtete Verbrauchsabweichung wurde in diesem Lauf nicht reproduziert.

Frisch aus jeder EXE extrahierte Typen und KFC-Ressourcen, ohne Typcache:

| Engine-Rezeptname | RecipeId | Client-Basisdauer | Server-Basisdauer |
| --- | ---: | ---: | ---: |
| `Material_T4_Factory_IronBar_SM` | 3802269334 | 1,5 s | 600 s |
| `Material_T5_Factory_IronBar_BF` | 46805630 | 1,35 s | 540 s |
| `Material_T2_Factory_CopperBar_SM` | 2859124257 | 0,75 s | 300 s |

Alle 1.954 Rezept-IDs stimmen überein. Genau 320 positive Zeitwerte unterscheiden
sich um Faktor 400. Eingaben, Ausgaben und Debugnamen stimmen überein. Die jeweilige
`.bak` enthält bereits dieselben hier verglichenen Werte wie ihre Installation.
Sie ist deshalb kein Nachweis unveränderter Originaldaten. Die Herkunft der
verkürzten Client-Werte ist nicht bewiesen.

`keen::Time` verwendet Nanosekunden. Das Serversystem `factory_station` bei
RVA `0xbb830` liest `RecipeInfo.craftingDuration` bei Offset 56, multipliziert
mit dem über `0x82cad0` gelesenen Weltfaktor und addiert die Dauer zum
`FactoryStation.recipeStart` bei Offset 80. Der untersuchte Server meldet
`factoryProductionSpeedFactor = 1`. Eine schnelle Client-Anzeige ändert diesen
serverseitigen Endzeitpunkt nicht.

## Tatsächlicher Serverdurchlauf

Nach zwischenzeitlichem Neustart ohne Mods wurde PID **11968** mit den sechs
Laufzeitmods gestartet. Beide relevanten Codeeingriffe wurden nochmals direkt
gelesen: `zero_resource_argument` setzt R12D auf null, `clear_item_use_argument`
setzt EAX auf null. Der Lauf ohne Mods, PID 21240, wird nicht als Mod-Test gewertet.

Bei Entity **1031** lief während der Messung das Kupferrezept **2859124257**.
Das zuvor ausgewählte Eisenrezept war inzwischen umgestellt worden.
Die Inventare liegen über `InventorySetup.linksEntities` an **1032** und **1033**;
das leere direkte `Inventory` der Station ist kein Beleg für fehlende Rohstoffe.

| ItemId | Bestand vorher | Bestand nachher |
| ---: | ---: | ---: |
| 455684957 | 220 | 220 |
| 372029138 (Zutat dieses Kupferrezepts) | 440 | 440 |
| 731732847 (weitere Zutat dieses Kupferrezepts) | 0 | 0 |
| 159114158 | 740 | 740 |
| 170689373 (Ausgabe dieses Kupferrezepts) | 0 | **10** |

Vorherige Beobachtung: **02:53:48 MESZ**, Ausgabe erstmals beobachtet:
**02:58:07 MESZ**, 09.10.2026. `recipeStart` wechselt von 154700003094 auf
454716675761 ns: **300,016672667 Sekunden** zwischen den Produktionsstarts.
Der nächste Lauf ist wieder `Running`. Der Server hat damit in diesem beobachteten
Durchlauf ohne Zutatenverbrauch produziert, mit seiner unverkürzten Rezeptdauer.
Das ist ein Live-Nachweis eines Kupferdurchlaufs; kein separater Eisen-,
Wiederverbindungs- oder Persistenztest. Speicherbeobachtungen sind nicht atomar.

Die Zutatenverarbeitung liegt beim Produktionsstart im Pfad
`0xb4f60 → 0xae9f0 → 0x1aba90 → 0x150990 → 0x150370 → 0x163e40 → 0x163e70`.
Die Ausgabe wird beim Ablauf über `0xb4d90` verarbeitet. Die nachgeschalteten
originalen Inventarhelfer wurden zusätzlich in Unicorn mit isolierten
Engine-Abhängigkeiten geprüft: **16 Fälle über beide EXEs**, jeweils Menge
0/1/10/20 und beide Slotpfade. Menge null erhält den Quellbestand; das ist
ergänzender Codebeweis, kein Ersatz für den obigen Live-Test.

## Umgesetzte Korrektur

[`sf-production-time`](../../../../../../mods/sf-production-time/README.md) verwendet
die vorhandene Lua-Asset-API für `RecipeRegistryResource.recipes[].craftingDuration`.
Es setzt eine absolute Basisdauer und verändert keine Zutaten, Ausgaben oder
Sofortrezepte. Das vermeidet wiederholtes Beschleunigen einer bereits geänderten
Baseline. Keine neuen nativen Hashes, Profile oder Netzwerkprotokolle.

Das Paket wurde in **beiden Spielinstallationen** mit `seconds = 1` aktiviert.
Es wird bei der nächsten Assetvorbereitung / beim nächsten Spielstart angewendet;
die laufenden Prozesse wurden nicht beendet und ihre Rezeptdaten nicht geändert.
Im Repo ist das neue Paket standardmäßig deaktiviert. Einstellungen werden nicht
automatisch zwischen Installationen übertragen. Der Weltfaktor gilt zusätzlich.

## Wiederholung und Werkzeuge

- `inspect_attribute_resources --type keen::RecipeRegistryResource` liest frische
  Ressourcen; `--backup` prüft die vorhandene Arbeitsbaseline separat.
- `compare-recipe-resources.py` vergleicht Engine-IDs, Zeiten, Ein-/Ausgaben und
  Debugnamen, einschließlich Herkunft und Hash der Exporte.
- `live-entity-managers` prüft nun die Komponentenregistrierung vor Tabellenkandidaten,
  berücksichtigt Tabellen bis 1.048.576 Plätze und liest den Entity-Zeilenindex
  korrekt als 32 Bit. Die frühere 64-Bit-Lesung verwarf gültige Entities.
- `capture-factory-state.py` revalidiert Registry, Layouts und Entity-Header,
  liest Stationen und verfolgt die verknüpften Inventare. Zugriff nur lesend.
- `verify-inventory-zero.py` führt die Originalhelfer ausschließlich im Emulator aus.
- `cargo test -p shroudforge-api --test production_time`: Logik, Idempotenz,
  Eingabevalidierung und unveränderte übrige Rezeptfelder. Die ignorierten Tests
  erlauben frische Exporte (`SF_RECIPE_SNAPSHOTS`) und einen vollständigen echten
  Lua/KFC-Schreib-/Lesezyklus auf privaten Dateikopien (`SF_FACTORY_GAME_SOURCE`).
  Beide echten Schreib-/Lesezyklen bestanden: jeweils 320 Zeitrezepte mit exakt
  einer Basis-Sekunde, alle übrigen Rezeptdaten unverändert. Beide Mod-Ursprungs-
  Audits decken jetzt neun Pakete ab; alle 16 Emulatorfälle bestanden ebenfalls.

Messartefakte dieser Sitzung liegen in `target/factory-*-recipes.json`,
`target/factory-recipe-comparison.json`, `target/factory-server-watch.jsonl`,
`target/factory-server-observation.json` und `target/factory-*-zero-proof.json`.
Sie werden nicht als ausführbare Profile ausgeliefert.
