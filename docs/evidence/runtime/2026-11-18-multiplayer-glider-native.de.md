# Multiplayer-Gleiter: aktuelle native Rotationsrechnung

Untersucht am 18. November 2026. Beide EXE-Dateien wurden als Daten gelesen. Ein Spielprozess wurde nicht verändert. Die RVA-Werte gelten ausschließlich für die genannten Images und sind keine Freigabe für einen Patch.

| Programm | Build | SHA-256 |
| --- | --- | --- |
| `enshrouded.exe` | 26430625 | `5d9ef2f140b8ed49e30dbee655841b89d08baec20f799a1817e171a18a850c77` |
| `enshrouded_server.exe` | 26430253 | `f0d824be7fcd48ebfe2458ff5c28da26060c2074e3ec1fcac587e5a1d508aeb2` |

## Originalinstruktionen im Dedicated Server

Die folgenden Instruktionen liegen im durch die PE-Unwinddaten beschriebenen Codebereich `0x807aa..0x86225`. Die Datenflussnamen und Feldzuordnungen stammen aus der getrennten früheren Reflection- und Registrierungsanalyse in [flight-dataflow-20261016.de.md](../game-builds/flight-dataflow-20261016.de.md). Die folgenden Instruktionen wurden erneut im oben genannten aktuellen Serverimage gelesen. Ihre Entsprechungen existieren auch im aktuellen Clientimage.

| Server-RVA | Instruktion oder Folge | Bedeutung im bekannten Datenfluss |
| --- | --- | --- |
| `0x815ba`, `0x815c2` | `movsd xmm0,[rax+0x18c]`, `mov edx,[rax+0x194]` | Die aktuelle `ActorInput`-Bewegung wird geladen. |
| `0x815d9` | `movzx eax,byte ptr [rax+0x2c8]` | Der Eingabemodus des aktuellen `PlayerInput` wird geladen. |
| `0x8163b..0x81657` | Laden von `[rdi+0x4d4]`, Multiplikation mit Eingabe und Zeitschritt, Addition zur bisherigen Neigung | Im Pitch-Speed-Modus bildet das Original aus dem gemeinsamen `pitchAngleSpeed` und dieser Spielereingabe einen individuellen Zielwert. |
| `0x817fe` | `f3 0f 10 05 3e af b5 00` | Lädt die untere Grenze von fünf Grad aus RVA `0xbdc744`. |
| `0x8180e..0x81823` | `comiss`, bedingter Sprung, `minss` und Übernahme der unteren Grenze | Der aktuelle Pitchzielwert in `xmm8` wird begrenzt. Der Zweig berücksichtigt außerdem die vorher verglichene Impulskomponente. |
| `0x8189e` | `movss xmm2,[rdi+0x4d4]` | Dieselbe gemeinsame Pitchrate begrenzt anschließend den Winkelschritt zum Ziel. |
| `0x81bd7..0x81c03` | Actorpointer laden, Zieladresse `[rdi+0xec4]` bilden und Helper aufrufen | Das berechnete Rotationsziel wird über den bestehenden Actorpfad verarbeitet. |

Die entsprechende untere Grenze lädt der Client bei RVA `0x26f6fe`. Die Eingabe- und Ratensequenz beginnt dort bei `0x26f4ba` beziehungsweise `0x26f53b`. Die aktuellen Client- und Serverinstruktionen sind in diesen Abschnitten strukturell gleich, ihre RIP-relativen Offsets und die Programm-RVAs sind verschieden.

Die folgende gemeinsame Kandidatensignatur findet in den ausführbaren Abschnitten beider genannten Images jeweils genau eine Stelle. `??` markiert die bildabhängigen RIP-relativen Displacements:

```text
F3 0F 10 05 ?? ?? ?? ?? F2 0F 11 4C 24 40 77 15 41 0F 2F C0 77 0F F3 0F 10 35 ?? ?? ?? ?? F3 41 0F 5D F0
```

Die Treffer liegen beim Client auf RVA `0x26f6fe` und beim Server auf RVA `0x817fe`. Die Eindeutigkeit dieser zwei Treffer belegt keine automatische Gültigkeit für andere Builds. Die maskierten Adressen müssen im jeweiligen Image decodiert und ihre Bedeutung muss geprüft werden. Der Ziel-Float wird auch an anderen Stellen verwendet.

## Zuordnung zum registrierten System im aktuellen Build

Die originale Systemregistrierung `actor_rotation` ist in beiden EXE-Dateien vorhanden. Ihre Callback-Pointer und die zugehörigen PE-Unwindketten wurden statisch aus den jeweiligen Images gelesen:

| Image | Registrierungsdeskriptor | Registrierter Callback | Pitchgrenze | Unwind-Haupteinstieg |
| --- | --- | --- | --- | --- |
| Client 26430625 | `0x1de6a00` | `0x26e620` | `0x26f6fe` | `0x26e620` |
| Server 26430253 | `0x133e3b0` | `0x80720` | `0x817fe` | `0x80720` |

Die Pitchstelle liegt jeweils in einem Unwindfragment, dessen verketteter Haupteinstieg genau dem registrierten Callback entspricht. Die Deskriptoren beider Images nennen dieselben Abhängigkeiten. Darunter sind `ActorRotation`, `Locomotion`, `ActorInput`, `PlayerInput`, `ServerConsumedPlayerInput`, `Actor`, `CurrentTransform` und `DynamicLocomotion`. Die vollständigen Listen der betroffenen sechs Slots stimmen für die beiden untersuchten Images überein. Dies belegt den statischen Zusammenhang zwischen registriertem System und den gefundenen Instruktionen in beiden Builds. Es ist noch kein Aufruf des produktiven `ResolveNamedSystemCode`, keine Livevalidierung und keine freigegebene Codeänderung.

Die unveränderten historischen Client- und Serverprofile enthalten bereits jeweils `runtime.patch.unlimited_flight` mit Modifier `override_rotation_constant`. Beide verwenden denselben acht Byte langen Detour und dieselbe Payload. Der Trampolin-Ersatz lädt `-1.57f` statt des Originalwerts und kehrt anschließend an die nächste Originalinstruktion zurück. Sein erklärter Geltungsbereich umfasst alle Ausführungen der Codeposition. Diese historischen Profile sind ein konkreter Vorläufer für die Umsetzung, aber ihre alten Adressen und die bestehende historische Ausführungsfreigabe gelten nicht für die aktuellen Images.

## Architekturentscheidung

Unsere Mod verändert `ItemInfo.equipment.gliderConfig.pitchAngleSpeed` abhängig von einem einzelnen Spieler. Die Ressource gehört dem Glidertyp und ist deshalb geteilt. Die Mod stoppt den steuernden Zweig bei mehr als einer gefundenen Spielerentity. Eine einfache Entfernung dieser Schranke würde konkurrierende Wünsche auf denselben Wert schreiben.

Der andere Ansatz greift in die originale Zielwinkelbegrenzung ein. Eine allgemeine Freigabe der Grenze ändert die Rechenregel für alle Glideraufrufe, die originale Routine verarbeitet aber für jeden aktuellen Actor dessen eigene Eingaben und dessen eigenes Rotationsziel. Dafür muss kein Lua-Tick pro Spieler den gemeinsamen Ressourcenwert umschalten. Eine pro Spieler einstellbare Freigabe benötigt zusätzlich einen überprüften Zusammenhang zwischen dem Actor im Hook und einer stabilen Spieleridentität sowie serverseitige Freigabe und gegebenenfalls Clientvorhersage.

Die beiden Images belegen eine mögliche Eingriffsstelle. Sie belegen weder, dass ein solcher Patch korrekt installiert ist, noch die Ausführungsrolle der aktuellen Session, Synchronisation, Kollisionsverhalten oder einen erfolgreichen Flug. Der Float an der Zieladresse wird auch von anderen Funktionen referenziert. Ein globales Überschreiben dieses Floats ist daher nicht gleichbedeutend mit einem gezielten Eingriff in die eine Ladeinstruktion.

Die vorhandene ShroudForge-Runtime ist bereits eine native DLL. Ihr allgemeiner ECS-Writer verlangt die autoritative Welt und erlaubt bei der hier betrachteten Bindung nur Komponenten ohne A- und B-Tracking. `DynamicLocomotion.state = BuildingFlying` ist ein anderer Ansatz für den Building-Flug und belegt keine gleichwertige Glidersteuerung. Die frühere Liveprobe änderte diesen Zustand zwar, beobachtete aber weiterhin aktive Gravitation und bestätigte keine Flugwirkung. Ein nachträglicher Schreibzugriff auf `CurrentTransform` ist nicht als gleichwertig zur ursprünglichen Rotationsrechnung belegt. Der Zugriff über Lua oder einen direkten DLL-Aufruf ändert diese Eigentums- und Phasenfragen nicht.

## Einordnung in die aktuelle Runtime-Strategie

Die bisherige Untersuchung hat keinen gleichwertigen individuellen Dateneingang für die feste Pitchgrenze ergeben. Die Runtime kann reflektierte ECS-Typen und Systemabhängigkeiten verbinden. `OriginalCode::ResolveNamedSystemCode` löst originale Systemdeskriptoren nach Namen auf. `ReadProcedure` und `FindUniqueReachable` untersuchen den Originalcode mit Zydis. Dieses Verfahren wird etwa für den Building-Prop-Spawn bereits verwendet. Die statische Registrierungs- und Unwindzuordnung von `actor_rotation` bis zum Fragment mit der Pitchgrenze ist für beide aktuellen Images belegt. Der neue Resolver für `runtime.patch.glider_pitch` verwendet den Originalnamen, den decodierten Funktionskörper, die zwei Winkelkonstanten und die verbundenen Klemmungsanweisungen. Er verlangt eine eindeutige Stelle und prüft den gespeicherten Code vor der Aktivierung erneut. Der produktive Resolver und der Gameplayeffekt sind noch nicht live bestätigt. Eine Registrierung oder ein Komponentenname allein beweist keinen aufrufbaren ABI-Vertrag und gibt keinen beliebigen Codeeingriff frei.

Der alte profilgebundene `runtimePatches`-Pfad existiert, ist für diese aktuellen Profile aber nicht angeschlossen. Die neue Fähigkeit nutzt dessen öffentliche Lua-Aufrufe und den vorhandenen Writer für ausführbaren Code. Sie legt keine RVA und keine AOB-Signatur im Mod ab. Die bekannte Signatur oben bleibt Untersuchungsbeleg. Das Verhalten bezüglich Mehrspielerautorität, Clientvorhersage und sichtbarem Flug muss noch geprüft werden.

Ein produktiver nativer Hook braucht eine exakt versionsgebundene Identität und eindeutige Prüfung der Originalinstruktionen, eine nachgewiesene Installations- und Rückbauphase, korrekte Fortsetzung einschließlich Registerzustand und getrennte Prüfung der Clientvorhersage und der Serverautorität. Ein bloßer AOB-Treffer oder ein unveränderter RVA beweist diese Eigenschaften nicht. Ein vollständiger Test benötigt mindestens zwei gleichzeitig gleitende Spieler mit unterschiedlichen Eingaben und Beobachtung durch beide Clients.

## Vergleich der vorliegenden Pakete

- Unified Tweaks setzt Gliderkoeffizienten über `ApplyGliderTuning` in gemeinsam genutzten `ItemInfo`-Ressourcen. Negative Zahlen passieren seinen Parser. Ein erfolgreicher Mehrspielerflug mit negativem Wert wurde nicht belegt, und ein ständig negativer Wert ändert die native Steuerung für alle Nutzer des Glidertyps.
- Für das ältere Embervale Toolkit 3.1.0 ist die Änderung der unteren Pitchgrenze im damaligen Clientcode dokumentiert. Das aktuelle Toolkit 3.5.0 enthält ein eigenes natives Backend, sein konkreter Handler für Infinite Flying wurde in dieser Untersuchung noch nicht vollständig zugeordnet. Ein dedizierter Mehrspielernachweis für diese Version liegt nicht vor.
- Die vorliegende Server-Companion-DLL besitzt Patchinfrastruktur und registriert unter anderem Glider-Reglernamen. Ein Handler für Infinite Flying oder eine autoritative Glider-Freigabe wurde damit nicht nachgewiesen.
