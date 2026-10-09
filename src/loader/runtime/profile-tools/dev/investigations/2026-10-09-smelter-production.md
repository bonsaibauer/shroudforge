# Schmelze: Rezeptdauer und Rohstoffverbrauch

## Aktueller Client: 09.10.2026, Start 04:42:41 MESZ, PID 15556

Diese Nachprüfung betrifft ausschließlich den lokalen Client. Der Benutzer hat
ausdrücklich klargestellt, dass Production Time auf dem Server für diesen Test
nicht aktiv ist. Serverbeobachtungen sind kein Wirkungsnachweis für diesen Test.

Die installierte Client-Einstellung ist `enabled: true`, `seconds: 1`. Der neue
Assetstart bricht nicht mehr beim World Editor ab: Er meldet eine geänderte
Ressource und einen abgeschlossenen Durchlauf in 3036 ms. Eine erneute direkte
KFC-Auslesung (`target/production-client-15556-disk.json`) bestätigt für das
Eisenrezept 3802269334 / `Material_T4_Factory_IronBar_SM` 1.000.000.000 ns.

Der ausschließlich lesende Speicherscan desselben Clients findet dagegen zwei
vollständige RecipeInfo-Datensätze mit 600.000.000.000 ns. Validiert wurden
Rezept-ID, Werkstatt-ID 4121605386, zwei Inputs und ein Output anhand der frischen
Reflection. Adressen: `0x200191b9ac8`, `0x260ce810b08`. Auf den ersten Datensatz
zeigen vier Pointer in Rezept-Pointertabellen (`0x30044f8a9c0`,
`0x30044f99878`, `0x300c1cd8500`, `0x300c1ce74a8`). Artefakte:
`target/production-live-recipes.json`,
`target/production-client-recipe-references.json`. Ein vollständiger Aufruftrace
der laufenden Factory-Funktion wurde nicht aufgenommen; Speicherfunde sind
nicht als gemessener kompletter Produktionszyklus zu bezeichnen.

Im Startpfad wurde eine konkrete Ursache für wiederholte Baseline-Rücksetzungen
gefunden: `export_pass_needed()` berücksichtigte auch Runtime-Mods mit Exportrecht.
Mit aktiviertem World Editor und global aktivierten Exporten umging jeder Start
deshalb die Abkürzung für bereits korrekt vorbereitete Assets. `run_inner()`
stellt zunächst die Original-Baseline her und patcht sie anschließend erneut.
Dies geschieht auf dem asynchronen Bootstrap-Thread, ohne das Assetladen des Spiels
zu synchronisieren. Ein erfolgreicher Datei-Commit bestätigt deshalb keine
Übernahme durch die bereits laufende Engine. Die beobachtete Datei-/Speicher-
Abweichung passt zu diesem Startproblem; die einzelnen Dateiöffnungszeitpunkte
der Engine wurden nicht getraced.

Korrektur: Nur Mods mit `requires_pregame()` und Exportrecht dürfen einen weiteren
Pregame-Exportdurchlauf erzwingen. Regression prüft den echten `run_startup()`-
Pfad mit bereits vorbereitetem Fingerprint, unterschiedlicher Original-Baseline
und gleichzeitig aktiver Patch-Mod sowie Runtime-/Export-Mod. Ein echter
Export-only-Mod muss weiterhin einen Exportdurchlauf anfordern.

Diese Änderung beseitigt das unnötige Zurücksetzen bei unveränderter Konfiguration.
Sie synchronisiert noch nicht die erstmalige Anwendung geänderter Asset-Mods
beim direkten Spielstart. Für diese bleibt der vor Prozessstart ausgeführte
Prepare-/Launch-Pfad die sichere Reihenfolge. Keine Live-Speicherschreibzugriffe,
keine automatischen Prozessneustarts und keine Serveränderungen in dieser Prüfung.

## Nachprüfung des lokalen Clients: 09.10.2026, 04:15–04:25 MESZ

Die frühere Aussage, die zusätzliche Produktions-Mod behebe die Abweichung nach
einem Neustart, war unvollständig. Zwei unabhängige Probleme wurden nachgewiesen:

1. Die Client-Arbeitsbaseline enthält schon beschleunigte Zeiten. Frisch ausgelesen:
   Eisen 1.500.000.000 ns, Kupfer 750.000.000 ns. Ausschalten einer späteren Mod
   stellt nur diese Baseline wieder her, keine unveränderten Originalzeiten.
2. Der Assetstart von PID 3740 und später PID 29320 brach bei `world-editor` mit
   `ShroudForge feature is unavailable: runtime.lifecycle` ab. Der Pregame-Runner
   führte Runtime-Mods mit Exportrecht aus, weil er Exportrecht fälschlich als
   Auftrag zur Assetausführung wertete. Die Oberfläche übersetzte den gespeicherten
   Vorbereitungsfehler in eine weitere Neustartaufforderung.

Die installierte Einstellung von `sf-production-time` stand bei dieser Prüfung
auf `enabled: true`; die Assetausführung war dennoch fehlgeschlagen. Die eine
Sekunde dieser Mod wurde in diesem Start nicht angewendet. Im offenen Client
war die Eisen-Schmelze als FactoryStation Entity 1454 mit Rezept 3802269334
lesbar. Die Live-Beobachtung wurde durch einen weiteren Client-Neustart beendet.

Korrektur im Code: Der Pregame-Runner führt ausschließlich Mods aus, deren
Manifest `requires_pregame()` erfüllt. Ein fehlgeschlagener Assetstart wird im
Modstatus und Gesamtstatus als `asset-preparation-failed` mit dem tatsächlichen
Fehler gemeldet. Regressionstests bestanden: echter KFC-Schreib-/Lesezyklus auf
einer Client-Dateikopie mit gleichzeitig aktivierter Runtime-/Export-Mod und
eingeschalteten Exporten; außerdem Statusprüfung für fehlgeschlagene Vorbereitung.
Release-Build erfolgreich. Diese Korrektur wurde in dieser Nachprüfung nicht in
den laufenden Client geladen. Die bereits veränderte Arbeitsbaseline wurde nicht
als Original neu deklariert oder anhand vermuteter Originalwerte überschrieben.

Die zusätzliche Mod ist optional, wenn eine explizite neue Produktionsdauer
gewünscht ist. Sie repariert keine früher veränderte Baseline. Für die Behebung
der beiden oben genannten Fehler ist keine weitere Mod erforderlich.

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
