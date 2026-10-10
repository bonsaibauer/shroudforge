# Dateien lesen und Exporte speichern

Die `shroudforge.io`-API trennt mitgelieferte Paketdateien von exportierten Spielerdaten. Ein Mod sollte nicht annehmen, dass beide Bereiche denselben Pfad oder dieselbe Schreibberechtigung haben.

## Berechtigung

Für Exportfunktionen muss `export` in `mod.json` stehen. Prüfe den API-Rückgabewert, bevor du einen Erfolg meldest. Nutze die [API-Referenz](#api), um die genaue Funktion und Argumente nachzuschlagen.

## Typischer Ablauf

1. Bestimme einen relativen Exportpfad.
2. Erzeuge die Bytes mit der passenden Buffer- oder Serialisierungsfunktion.
3. Rufe die Exportfunktion auf.
4. Prüfe den Rückgabewert und schreibe eine verständliche Ergebnis- oder Fehlermeldung.

Der Loader begrenzt Pfade auf den jeweiligen Modbereich. Die Exportfunktion ist kein allgemeiner Zugriff auf beliebige Dateien des Computers.
