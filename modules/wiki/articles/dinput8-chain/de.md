# DInput8-Chain-Ladestandard

Viele Enshrouded-Modloader und DLL-Mods möchten die `dinput8.dll` des Spiels sein. Diese Datei darf jedoch nur einmal vorhanden sein. Ein Loader, der diesen Namen trägt, startet die anderen DLLs aus `dinput8-chain.ini` neben der Spiel-EXE. EMF, Kiln, ShroudForge, XHL-Core und Auto Loot Plus lesen diese Datei. Diese Seite hält die Regeln fest, damit jeder Loader die Datei gleich liest und die Liste eines Spielers unabhängig davon funktioniert, welcher Loader die `dinput8.dll` bereitstellt.

Diese Seite steht unter CC0 und ist gemeinfrei. Du kannst sie in die Dokumentation deines Projekts übernehmen und ändern. Eine Quellenangabe ist nicht erforderlich. Änderungen, denen mehrere Loader zustimmen, sind als Pull Request im EMF-Repository willkommen.

Die Wörter „muss“ und „sollte“ beschreiben im Folgenden das Verhalten eines Loaders, der dieser Spezifikation folgt.

## Die Datei

```ini
; my mods
[Chain]
mods\mymod\mymod.dll
autolootplus.dll

[EMF]
creative.dll
```

- Speicherort: `dinput8-chain.ini` im Ordner der Spiel-EXE (`enshrouded.exe` oder `enshrouded_server.exe`).
- Kodierung: Ein Loader muss UTF-8 mit oder ohne BOM sowie UTF-16 LE mit BOM lesen. Er sollte außerdem die ANSI-Codepage des Systems lesen, wenn die Bytes kein gültiges UTF-8 ergeben, etwa bei einer mit einem alten Notepad gespeicherten Datei. Ein Werkzeug, das die Datei schreibt, muss die gefundene Kodierung und Zeilenumbrüche beibehalten.
- Zeilen: Leere Zeilen werden übersprungen. Zeilen, die mit `;`, `#` oder `//` beginnen, sind Kommentare. Ein `; ` vor einer DLL-Zeile deaktiviert diese DLL. So können Spieler sie ausschalten, ohne die Zeile zu löschen. Ein Kommentar darf nach einem Leerzeichen oder Tab hinter dem Namen stehen (`mymod.dll ; my mod`). Namen dürfen in doppelte Anführungszeichen gesetzt werden.
- Abschnitte, deren Namen nicht zwischen Groß- und Kleinschreibung unterscheiden: `[Chain]` ist gemeinsam. Jeder Loader, der dieser Spezifikation folgt, startet die dort aufgeführten DLLs. Jeder andere Abschnitt gehört zu einem einzelnen Werkzeug und trägt dessen Namen, etwa `[EMF]` oder `[Kiln]`. Andere Loader müssen ihn ignorieren. Zeilen vor dem ersten Abschnitt gehören zu `[Chain]`. Ältere Dateien enthalten möglicherweise gar keine Abschnitte.
- Zeilen im Format `key=value`: Eine Zeile `name=value` gilt als DLL-Eintrag, wenn ihr Wert eine `.dll` benennt. Andere Schlüssel sind Einstellungen des Werkzeugs, das sie geschrieben hat. Andere Loader ignorieren sie.

## Die DLLs

- Pfade sind relativ zum Ordner der EXE. `/` wird als `\` gelesen. Ein Loader muss absolute Pfade, Laufwerksbuchstaben, UNC-Pfade (`\\server\...`) und jeden Pfadteil `..` ablehnen. Eine Chain-Datei darf nicht aus dem Spielordner herausführen.
- Nur `.dll`-Dateien sind zulässig. Eine Zeile mit einem anderen Dateityp wird übersprungen und gemeldet.
- Niemals `dinput8.dll` selbst laden. Ein Loader darf außerdem keine zweite Kopie von sich selbst starten. EMF erkennt beispielsweise seine eigene Loader-Datei auch unter einem anderen Namen und lädt diese Kopie nicht.
- Jede DLL wird nur einmal gestartet. Steht eine DLL mehrfach in der Datei, etwa in `[Chain]` und im eigenen Abschnitt eines Werkzeugs oder zweimal im selben Abschnitt, wird sie nur an ihrer ersten Position gestartet.
- Reihenfolge: DLLs starten entsprechend der Reihenfolge ihrer Zeilen von oben nach unten. Dabei werden `[Chain]` und der eigene Abschnitt des Loaders in der Reihenfolge berücksichtigt, in der sie in der Datei stehen.
- Zeitpunkt: Der Loader verwendet `LoadLibrary` aus seiner eigenen `DllMain` oder unmittelbar danach, bevor Spielcode ausgeführt wird. Zu diesem Zeitpunkt wäre eine verkettete DLL gestartet worden, wenn sie selbst die `dinput8.dll` wäre.
- Höchstens 256 Einträge werden berücksichtigt. Weitere Einträge werden gemeldet und übersprungen.
- Fehler: Eine fehlende DLL oder eine DLL, die nicht geladen werden kann, wird übersprungen und gemeldet. Dazu zählen Windows-Fehler 126 sowie Fehler 1114, wenn ihre `DllMain` fehlschlägt. Das Spiel wird fortgesetzt.

## Meldungen

Ein Loader sollte für jede verkettete DLL eine Zeile protokollieren: gestartet, übersprungen, etwa weil sie deaktiviert oder abgelehnt wurde oder bereits geladen ist, oder fehlgeschlagen mit der Windows-Fehlernummer. EMF schreibt dieselben Informationen zusätzlich als Tabelle nach `emf\chain_status.txt`, damit Spieler sie weitergeben können.

## Wenn du die dinput8.dll eines anderen Werkzeugs verschiebst

Ein Installationsprogramm, das die `dinput8.dll` eines anderen Werkzeugs findet, sollte sie umbenennen und niemals löschen. Es trägt den neuen Namen im eigenen Abschnitt von `dinput8-chain.ini` ein und stellt die Datei bei der Deinstallation wieder her. Wenn EMF das Werkzeug erkennt, nennt es die Datei `<tool>.dll`. Andernfalls verwendet EMF Versionsinformationen der DLL oder den Namen `dinput8-previous.dll`. Die verschobene Datei wird außerdem in `emf\moved.txt` vermerkt.

## Exporte

Das Spiel ruft `DirectInput8Create` bei dem Loader auf, der die `dinput8.dll` bereitstellt. Der Loader muss diesen Aufruf an die eigene Windows-Datei `dinput8.dll` weiterleiten (`%SystemRoot%\System32\dinput8.dll`). Verkettete DLLs erhalten keine DirectInput-Aufrufe. Ein Mod, der seine Arbeit in `DirectInput8Create` erledigt, muss diese Logik in `DllMain` oder in einen von ihm gestarteten Thread verschieben.

## Wer die Spezifikation verwendet

| Werkzeug | Liest `[Chain]` | Eigener Abschnitt |
| --- | --- | --- |
| EMF | Ja, auch Zeilen vor dem ersten Abschnitt | `[EMF]`, für DLLs, die das Installationsprogramm umbenannt hat |
| ShroudForge | Ja. Jeder Eintrag muss unter `mods\<folder>\` liegen. Andere Einträge werden gemeldet. | |
| Kiln | Ja, auch `[EMF]` für DLLs, die ältere EMF-Installationsprogramme umbenannt haben | Keiner. Die eigene Liste steht in `kiln\chain.ini`. Namen in Anführungszeichen und Kommentare hinter Namen sind geplant. |
| XHL-Core, Auto Loot Plus | Ja | |

Melde uns, wenn dein Werkzeug die Datei liest oder anders interpretiert. Dann aktualisieren wir diese Tabelle.

## Quelle

[Gemeinsame EMF-Spezifikation für dinput8-chain.ini](https://github.com/matenm/emf-enshrouded/blob/main/docs/modding/dinput8-chain.md). Die ursprüngliche Spezifikation steht unter CC0 und ist gemeinfrei.
