# DLL Manager

Öffne im Modloader **Kompatibilität → DLL-Verwaltung**. Oben in dieser Ansicht steht der passende Wiki-Artikel. Hier siehst du, welche Mod-DLLs ShroudForge gefunden hat und ob sie geladen wurden. Für bekannte Mods gibt es vorbereitete Profile. Andere DLLs wählst du dort aus und bestätigst sie.

## Kann ich mehrere native DLL-Mods verwenden?

Ja. Installiere jeden Mod in einem eigenen Ordner direkt unter `mods/` und übernimm alle mitgelieferten Dateien und Unterordner. Ein passendes Profil kann die richtige DLL-Auswahl bereits bereitstellen. Öffne bei einem Mod ohne passendes Profil in der DLL-Verwaltung **DLL-Auswahl anpassen**. Wähle nur die Einstiegs-DLLs aus, die laut Anleitung des Autors geladen werden sollen. Wähle das Ladeziel und bestätige, dass du der DLL vertraust. Lass **Normale DLL** für gewöhnliche native Bibliotheken eingestellt. Wähle **DirectInput-Proxy** nur für eine DLL, die als DirectInput-Proxy dient und `DirectInput8Create` exportiert. Klicke auf **Auswahl übernehmen** und starte das Spiel neu. Die Erkennung allein lädt keine DLL ohne Profil. Dafür braucht es eine bestätigte DLL-Auswahl, eine Deklaration in `native-plugin.ini` oder `dinput8-chain.ini` oder eine Registrierung durch die Lua-Mod.

## Was mache ich mit dem heruntergeladenen Mod?

- **Keine DLL dabei:** Installiere den Mod wie gewohnt. Mods, die für ShroudForge oder EML geschrieben wurden, brauchen keine zusätzliche DLL.
- **DLL dabei:** Lege die Mod-DLL in den Modordner, wie unten gezeigt. Danach erscheint sie in der DLL-Verwaltung. Bei einem bekannten Profil ist die Auswahl vorbereitet. Sonst wählst du die DLL dort aus und bestätigst sie.
- **Weitere Moddateien dabei:** Übernimm den Modordner vollständig. Lösche mitgelieferte Dateien wie `mod.json`, `native-plugin.ini` oder Konfigurationen nicht.

## Wenn ein Mod mehrere DLL-Dateien enthält

Viele Modordner enthalten DLL-Dateien, die nur Hilfsdateien sind. Wähle nur die DLLs aus, die das Modprofil oder die Anleitung des Modautors zum Laden nennt. Mit den Pfeilen legst du die Reihenfolge der ausgewählten DLLs innerhalb dieses Modordners fest. Für gewöhnliche DLLs aus verschiedenen Modordnern gibt es keine globale manuelle Reihenfolge.

## Mod mit DLL installieren

Wenn der ShroudForge-Installer das heruntergeladene Modpaket erkennt, installiere es dort. Bei einer manuellen Installation bekommt jeder Mod einen eigenen Ordner direkt unter `mods/`. Lege die Mod-DLL in diesen Ordner. Liegt sie im Archiv neben dem Modordner, lege sie zum Mod. Behalte den ursprünglichen Dateinamen und die mitgelieferten Unterordner bei.

```text
mods/
  mein-mod/
    mod.json        (falls enthalten)
    dinput8.dll     (oder die DLL des Mods)
    config.ini      (falls enthalten)
```

Eine DLL wird nicht allein durch das Ablegen geladen. Wähle sie wie oben beschrieben in der DLL-Verwaltung aus.

## Warum darf die Moddatei auch dinput8.dll heißen?

ShroudForges eigene Startdatei liegt unter `dinput8.dll` im Spielordner. Die Moddatei liegt getrennt unter `mods/<Modordner>/dinput8.dll`. ShroudForge lädt sie über diesen vollständigen Pfad. Bei einer passenden Profil- oder DLL-Manager-Zuordnung leitet ShroudForge DirectInput-Aufrufe an sie weiter. Beide Dateien dürfen denselben Namen tragen. Benenne die Moddatei nicht um und lege sie nicht in den Spielordner. Ersetze dort nicht ShroudForges Startdatei.

## Profile verstehen

Ein Profil ist eine vorbereitete Einstellung für einen bekannten Mod. Es legt fest, welche Dateien ShroudForge lädt und wie. ShroudForge ordnet das Profil anhand der Modkennung in `mod.json` oder, wenn sie fehlt, anhand des Ordnernamens zu. Du kannst ein Profil auch in der DLL-Verwaltung auswählen. Profile ändern die Moddateien nicht. Eine eigene gespeicherte DLL-Auswahl ersetzt die Dateiauswahl des Profils.

## Bestehende DirectInput-Chain

Eine vorhandene `dinput8-chain.ini` im Spielordner verwendet das etablierte Format:

```ini
[Chain]
mods\mein-mod\dinput8.dll
mods\anderer-mod\native.dll
```

Die Einträge müssen auf Dateien innerhalb direkter Modpakete wie `mods/<Modordner>/` verweisen. DLLs im Spielhauptordner und Pfade außerhalb dieser Pakete werden abgewiesen. Die Reihenfolge hat Vorrang vor automatisch ermittelten Deklarationen. Die Datei legt die Reihenfolge der DirectInput-Chain fest, nicht die Reihenfolge gewöhnlicher DLLs aus verschiedenen Modordnern. Derselbe aufgelöste Dateipfad wird nur einmal geladen. Die erste gültige DLL mit eigenem `DirectInput8Create`-Export erhält die weitergeleiteten Aufrufe. Weitere DirectInput-Proxy-DLLs können geladen werden, ihre DirectInput-Exporte sind in dieser Chain aber nicht aktiv. Gehe nicht davon aus, dass mehrere Proxys zusammen funktionieren. Eigene Startdateien werden übersprungen, rekursive Weiterleitung fällt auf die originale Windows-Bibliothek zurück.

## Schalter, Ziel und Neustart

Der Hauptschalter steuert alle von ShroudForge ausgeführten DLL-Ladewege, einschließlich INI, Profil, Chain und Lua-Registrierung. Eine deaktivierte Auswahl unterdrückt das automatische Laden dieses Ordners. Client, Server oder beide bestimmen den Zielprozess. Dateinamenfilter gelten unmittelbar vor dem Laden. Sie erkennen keine Modfunktionen anhand des Namens.

Geladene DLLs bleiben bis zum Prozessende im Speicher. Ausschalten, andere Reihenfolge oder ein anderes Profil erfordern deshalb einen Spielneustart. Doppelte Deklarationen derselben Datei erzeugen keine zweite Instanz.

## Ein Ladefehler wird angezeigt

Prüfe Modname, Dateipfad und Ursache. Windows-Fehler 126 kann eine fehlende Hauptdatei oder eine ihrer benötigten Bibliotheken bedeuten. Fehlt der DirectInput-Export, prüfe die ausgewählte Datei und ihren Lademodus. „Geladen“ bestätigt den Ladeschritt, nicht sämtliche Modfunktionen.

Ein unbekanntes natives Pluginformat, ein Vulkan-Layer, ein zusätzlicher vollständiger Loader oder ein externes Programm benötigt seine vorgesehene Integration. Einfaches Laden ersetzt diese Schnittstelle nicht.

## Pakete installieren

Der Installer findet ein einzelnes Modpaket auch in verschachtelten Archivordnern. Bekannte Profile übernehmen ausdrücklich benannte Begleitdateien. Ein neues Archiv mit mehreren Modpaketen wird nicht automatisch gesammelt installiert. Entpacke es und installiere die einzelnen Modordner nacheinander. Im laufenden Betrieb werden keine Archive geöffnet und keine fremden Programme gestartet.
