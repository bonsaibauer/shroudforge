# Mehrspieler-Aufwärtsflug über die vorhandene ECS-API

Stand: 17. November 2026. Anlass ist Issue #48. Der bisherige Lua-Mod verweigert die Neigungssteuerung bei mehr als einer `PlayerInput`-Entity. Er verwendet die gemeinsame Gleiterressource `ItemInfo.equipment.gliderConfig.pitchAngleSpeed` als spielerabhängigen Steuerwert. Eine Entfernung der Spielerzahlprüfung allein würde weiterhin verschiedene Eingaben auf denselben Wert schreiben.

## Originaler Datenfluss der aktuellen EXEs

Die neuen statischen Belege liegen im ignorierten Verzeichnis `dev/profiles/captures/unlimited-flight-ecs-dataflow-20261117/`. Die ursprüngliche Registrierung `actor_rotation` liefert den Einstieg. Die Instruktionen wurden aus den vollständigen EXEs gelesen. Es wurden keine Enginefunktionen aufgerufen und keine ausführbaren Bytes verändert.

| Ziel | SHA-256 | `actor_rotation` |
| --- | --- | --- |
| Client 26430625 | `5d9ef2f140b8ed49e30dbee655841b89d08baec20f799a1817e171a18a850c77` | `0x26e620` |
| Server 26430253 | `f0d824be7fcd48ebfe2458ff5c28da26060c2074e3ec1fcac587e5a1d508aeb2` | `0x80720` |

Der Client liest bei `0x26f161` und `0x26f189` die aktuelle Orientierung aus `CurrentTransform`. Die Rechnung bei `0x26f79e` bis `0x26f7ed` begrenzt den Pitchschritt durch die geladene Pitchrate und den Zeitschritt. Bei `0x26f955` wird der Schritt zur aktuellen Neigung addiert. Auf dem Server entsprechen dem die Stellen `0x8189e` bis `0x818ed` und `0x81a55`. Bei Pitchrate null bleibt dieser Schritt null. Die Yawrechnung läuft separat weiter.

Der Zielwinkelmodus multipliziert `ActorInput.desiredLocalMoveInput.z` mit `π/2`. Die aktuellen Originalanweisungen stehen beim Client bei `0x26f52b` und `0x26f530`, beim Server bei `0x8162b` und `0x81630`. Die Umrechnung in Grad ist somit Eingabe mal 90. Der Geschwindigkeitsmodus verwendet die Eingabe für einen Winkelschritt.

Die Rollkorrektur verwendet weiterhin separat `ItemInfo.rollAngleSpeed`. Beim Client stehen der Ratenzugriff bei `0x26f7f1` und die Addition des Rollschritts bei `0x26f9cd`, beim Server entsprechend bei `0x818f1` und `0x81acd`. Der gewöhnliche Rollzielwert folgt aus der seitlichen Eingabe mal −55 Grad, ergänzt um mögliche Turbulenz. Pitchrate null deaktiviert diese Korrektur nicht. Damit kann die ursprüngliche Engine eine invertierte Bank weiterhin korrigieren. Das ist kein uneingeschränkter oder live bestätigter Recovery-Nachweis für jeden Rollparameter und Enginezustand.

Die ältere unveränderte Originalausführung aus `2026-10-16-kfc4-glider-patch-to-data.de.md` enthält bereits eine Haltefolge mit Rate null. Die aktuellen Instruktionslisten bestätigen die benötigte Rückkopplung erneut für beide heutigen Images. Die frühere Aussage, ein beliebiger Transformschreibzugriff genüge als Ersatz, wird dadurch nicht pauschal bestätigt. Entscheidend ist hier das zusätzliche Halten des originalen Pitchschritts.

## Verwendung der vorhandenen öffentlichen API

`mods/sf-unlimited-flight/src/mod.lua` erfasst zunächst die aktiven Benutzer jeder Gleiterressource. Für eine vollständig aufgelöste Gruppe hält `runtime.overrides.resource` die Pitchrate einheitlich auf null. Die ursprüngliche positive Rate bleibt im vorhandenen Override-Tracker als Baseline erhalten.

Anschließend liest der Mod die aktuelle Orientierung jedes Spielers und berechnet dessen eigenen Neigungsschritt. Die Achse folgt aus Weltaufwärtsrichtung und Vorwärtsrichtung dieser Quaternion. Der Schritt verwendet die ursprüngliche Rate, höchstens 100 Millisekunden gemessene Zeit und Grenzen von −85 bis +80 Grad. `runtime.ecs.write_fields` schreibt ausschließlich die reflektierten Orientierungsfelder von `keen::ecs::CurrentTransform`. Die erwartete Ausgangsquaternion und der vollständige native Komponentenvergleich bleiben erhalten. Komponentenindizes, Offsets und RVAs stehen nicht im Modcode.

Ein unlesbarer Actor verhindert für dieses Update die gemeinsame Haltefreigabe. Ein fehlender Ressourcenbezug oder nicht unterstützter Eingabemodus verhindert sie für die betroffene Gleitergruppe. Der bestehende Tracker stellt unbenutzte Raten wieder her. Ein späterer einzelner Transformkonflikt überspringt den betreffenden Bewegungsschritt und kann im nächsten Update mit einem frischen Snapshot erneut versucht werden.

Landen, Deaktivieren und Entladen stellen die gemeinsame Rate über den bisherigen Cleanupvertrag wieder her. Eine ausgeführte Orientierung ist Bewegungszustand. Sie wird beim Cleanup nicht auf eine alte Flugpose zurückgesetzt. Eine pausierte Lua-Ausführung hinterlässt Rate null und keinen weiterlaufenden negativen Pitchbefehl.

## Aktueller lesender Runtime-Abgleich

Die frische Serveraufnahme liegt unter `dev/profiles/captures/flight-server-live-20261117-0043/server/`. Sie wurde gegen PID `19944` und die oben genannte Serveridentität aufgenommen. Die Registryaufnahme um 00:43:52 Uhr MEZ und der Produktionsverifier bestätigen 640 Registrierungen, 541 Laufzeitlayouts und 99 reine Templates. `CurrentTransform` hat aktuell Index 133, Größe 56 und kein A- oder B-Trackingbit. Der Mod löst diese Registrierung weiterhin dynamisch auf. `Actor`, `ActorInput`, `PlayerInput` und `Gravity` sind A-getrackt und werden durch diese Lösung nur gelesen.

Die Aufnahme um 00:45:45 Uhr MEZ fand eine lebende Spielerentity mit Eingabemodus `YawSpeedPitchSpeed`. Der Serverstatus bestätigte den aktivierten Mod, autoritative Schreibbereitschaft und einen Spieler ohne aktive Gleiteraktion. Client PID `39656` war eine lesende Replik. Der vorhandene serverseitige Systemdeskriptor `server_history` liest `CurrentTransform` und schreibt `ServerTransformationHistory`. Das belegt den normalen Folgekonsumenten, ersetzt aber keinen beobachteten Netzwerk-Flugtest.

Der kanonische Captureaufruf scheiterte nach der Registryaufnahme in der zusätzlichen statischen Funktionsphase, weil dem Standard-Python `capstone` fehlte. Der Produktionsverifier wurde danach ausdrücklich erfolgreich aufgerufen. Es gab bei diesen Aufnahmen keine Spielwrites und keine Installationsänderungen.

Die Lua-Prüffälle prüfen die Steuerung von zwei und 16 synthetischen Spielern mit derselben Gleiterressource, beide Eingabemodi, individuelle Konflikte und die Wiederherstellung gemeinsam genutzter Werte. `cargo test --release --locked -p shroudforge-api --test flight --offline` ist mit einem bestandenen Rust-Test erfolgreich. Dieser führt die vollständige Lua-Fixture aus. `npm run site:check` und die auf die Änderung begrenzte Diffprüfung sind ebenfalls erfolgreich.

Sichtbarer gleichzeitiger Flug und mögliche Clientkorrekturen mit diesem neuen Code sind noch nicht live bestätigt. Die bestehenden Fehlermeldungen eines Weltwechsels mit unrestaurierten gemeinsamen Werten behalten ihren bisherigen Schutzvertrag. Der neue Modcode wurde während dieser Untersuchung nicht in die laufende Spielinstallation übernommen.
