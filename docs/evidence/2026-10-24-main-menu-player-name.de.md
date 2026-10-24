# Spielername aus dem ursprünglichen Hauptmenümodell

Untersuchung vom 24. Oktober 2026. Die aktuelle Zuständigkeit beschreibt `docs/development/code-ownership.en.md`.

## Belegter Ursprung

Die Menüerweiterung lieferte mit `mainMenuEntries` einen ursprünglichen UI-Anker. Derselbe Menüaufbau referenziert `characterTextGroup`, ruft unmittelbar davor den ursprünglichen Auswahlgetter auf und übergibt dessen erhaltenen Rückgabewert an den Namensformatter. Der Formatter übernimmt einen String-View mit UTF-8-Zeiger und Bytelänge aus dem ausgewählten Datensatz.

Der alte Katalogtyp `keen::CharacterSaveGameEntry` war eine Untersuchungsspur. In der Reflection der aktuellen EXE ist er nicht vorhanden. Sein altes Layout wird nicht als aktuelles Laufzeitlayout verwendet.

Der Auswahlgetter vergleicht die zweibytegroße aktuelle Auswahlkennung mit Einträgen der ursprünglichen Quellcontainer. Quellabstände, Listenfelder, Eintragsgröße, Auswahlfelder und Namensspan lassen sich aus seinen Instruktionen ableiten. Sein globaler Renderkontext ist nur innerhalb eines UI-Frames gültig. Genau dort läuft der vorhandene native Menücallback.

## Korrigierte Integration

Der zuerst implementierte externe Rust-Leser mit Prozesshandles, eigenem Decoder, stabiler Besitzerkette und 500-ms-Hintergrundthread wurde nach der Nutzerkorrektur entfernt. Ebenso entfernt sind sein Cargo-Beispiel, die zusätzliche Decoderabhängigkeit und deren Paketierung. Es bleibt kein parallel laufender Namensmonitor.

`modules/game-runtime/native/main-menu/resolve_character_selection.cpp` verwendet den bereits aufgelösten Original-Menübuilder und die vorhandene Zydis-/Unwind-Infrastruktur. Er ermittelt den Namenvertrag ohne zusätzliche feste Ziel-RVA, EXE-Hashfreigabe oder ein neues Spielprofil. Er führt keine Spielfunktion aus.

`manage_native_main_menu.cpp` erfasst den Namen im bestehenden Menücallback. Der Callback bleibt auch bei ausgeblendetem Mods-Eintrag aktiv. Der Cache besitzt kopierte UTF-8-Bytes. Die erste gültige Auswahl und Änderungen der Auswahl, Quellheader oder Namensmetadaten lösen einen Leseversuch aus. Ein fehlgeschlagener Versuch liefert bis zur nächsten solchen Änderung keinen Namen und wird nicht mit einem Timer wiederholt. Ein Weltwechsel ohne Menücallback löscht einen bereits erfassten Namen nicht.

Nach dem ersten Auffinden wird nur der Quell- und Eintragsindex behalten. Jeder Menücallback berechnet die aktuelle Eintragsadresse aus dem gerade gelesenen Listenheader und vergleicht Kennung und Namensdeskriptor. Namenbytes werden bei unveränderten Metadaten nicht erneut gelesen. Eine Umordnung innerhalb derselben Listenallokation wird anhand der Eintragskennung erkannt.

Der ursprüngliche Formatter belegt einen 16-Byte-Namensspan. Der untersuchte ursprüngliche Producer ergänzt unmittelbar dahinter eine aus den Namensbytes berechnete Engineidentität. Die Integration übernimmt diesen zusätzlichen Datenvertrag nur bei passender Eintragsgrenze und identischer, aus den tatsächlich kopierten UTF-8-Bytes berechneter Identität. Der Producer erweitert die einzelnen Bytes vor seinem FNV-Schritt mit Vorzeichen. Das wird auch bei Unicode beibehalten. Dies ist eine Prüfung von Namensdaten und keine Freigabe anhand eines EXE-Hashes. Abweichende Daten führen zum Standardnamen.

Die Name-Erfassung installiert keinen zusätzlichen Hook oder Codepatch. Sie erweitert die Arbeit des bereits vorhandenen Menücallbacks. Die beiden ursprünglichen Einhängepunkte gehören zum Hauptmenüzweig. Änderungen in einem anderen Menümodus werden beim nächsten Erreichen dieses Hauptmenüzweigs erfasst.

Die optionale ABI-Funktion `KfcRuntimeSelectedCharacterName` und Lua `runtime.player.name()` lesen ausschließlich diesen Cache. Die bestehende Runtime-Statuspublikation transportiert denselben Wert unter `runtimeProvider.selectedCharacterName` an den Modloader. `modules/player-name/lib.rs` kontrolliert Prozesszuordnung und Frische des vorhandenen Status und wendet den konfigurierten Standardnamen an. Es liest weder Spielspeicher noch Spiellogs. Der frühere persistente `playerName`-Abschnitt wird nicht mehr als Quelle verwendet und nicht verändert.

## Untersuchung am laufenden Client

PID 27808 zeigte `Ikiju 育樹` mit Auswahlkennung `01 01` und `Bonsaibauer` mit `02 01`. Der Nutzer bestätigte die erste Auswahl, wechselte zu `Bonsaibauer` und danach zurück. Die damalige lesende Untersuchung und der inzwischen entfernte externe Rust-Prototyp bestätigten die passenden Werte einschließlich Unicode. Diese Ergebnisse belegen den Datenursprung. Sie sind keine Liveprüfung des später integrierten nativen Caches.

Andere Spielversionen und ein Livebetrieb der neuen Integration nach einem Spielneustart bleiben gesondert zu prüfen. Ein Build oder eine erfolgreiche statische Ableitung allein bestätigt keine während des Spiels ausgeführte Aktualisierung.

Die korrigierte Integration wurde mit `cargo check` für Spielername, Runtime-Bridge, Lua-API und Modloader-UI kompiliert. `dev/build/build_kfc_runtime.ps1`, `dev/build/build_modloader.ps1 -SkipUiBuild`, `dev/build/check_website.ps1`, die gezielte Rustformatprüfung und `git diff --check` waren erfolgreich. Der Modloaderbuild verwendete die vorhandenen Frontendbundles. Es wurden keine Tests ausgeführt und keine installierten Spielbinärdateien ersetzt.
