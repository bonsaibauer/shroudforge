# Modloader-P2P-Verbindung

Hier erfährst du, wann der Modloader einen Dedicated Server über Steam P2P erreicht und welche Enshrouded-Benutzergruppen die Serveradministration freischalten.

## Im Spiel verbinden

1. Starte Enshrouded und betrete eine Spielwelt des Dedicated Servers.
2. Öffne den Modloader und wähle **Server** als Ziel aus.
3. Warte, bis der Serverstatus **Verbunden** anzeigt. Im Desktopfenster und im Hauptmenü ist keine P2P-Verbindung in einer Spielwelt bestätigt. Deshalb bleibt das Serverpasswortfeld ausgeblendet.
4. Gib das Passwort eines passenden Eintrags unter `userGroups` ein. Der Server prüft das Passwort über die P2P-Verbindung.

## Erforderliche Rechte der Servergruppe

Das eingegebene Passwort muss exakt zu einer Gruppe mit einem nicht leeren `name` passen. Dieselbe Gruppe muss alle fünf unten aufgeführten Rechte auf den JSON-Wert `true` setzen. Ein fehlendes Feld, `false` oder ein Wert mit einem anderen Datentyp qualifiziert die Gruppe nicht für die Serveradministration im Modloader.

```json
"userGroups": [
  {
    "name": "Admin",
    "password": "REPLACE_WITH_A_PRIVATE_PASSWORD",
    "canKickBan": true,
    "canAccessInventories": true,
    "canEditWorld": true,
    "canEditBase": true,
    "canExtendBase": true,
    "reservedSlots": 0
  }
]
```

`reservedSlots` legt reservierte Spielplätze fest und gehört nicht zur Rechteprüfung des Modloaders. ShroudForge sendet das eingegebene Passwort zur Prüfung an den Server und speichert es nicht. Der Server prüft die Gruppenrechte erneut, wenn er Adminaktionen ausführt. Fünf fehlgeschlagene Anmeldeversuche lösen eine 60-sekündige Anmeldesperre aus.
