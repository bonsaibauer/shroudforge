# Start wegen inkompatibler Asset-Sicherungen abgebrochen

Fehlerbehebung für ältere ShroudForge-Versionen. Die Versionen 1.6.8, 1.7.0 und 1.7.3 können den Spielstart abbrechen, wenn ihre Asset-Sicherungen nicht zu den installierten Spieldateien passen.

<!-- wiki-category: troubleshooting -->

## Warum der Start abgebrochen wird

ShroudForge erstellt Arbeitskopien von `enshrouded.kfc` und `enshrouded.kfc_resources`, damit es die Spieldateien vor der Vorbereitung von Asset-Mods wiederherstellen kann. Diese `.bak`-Dateien können von einer älteren Enshrouded-Version oder einem anderen Mod stammen. Wenn die KFC-Sicherung nicht zur aktuellen Spieldatei passt, können ältere ShroudForge-Versionen sie nicht sicher wiederherstellen und brechen den Start ab.

## Bei ShroudForge 1.7.5 oder neuer

Wenn die KFC-Sicherung inkompatibel ist, können diese Versionen beide Arbeitskopien in den ShroudForge-Papierkorb verschieben und neue Sicherungen aus den aktuellen Spieldateien erstellen. Dafür muss `shroudforge/shroudforge-updater.exe` im Enshrouded-Installationsordner vorhanden sein. Meldet das Protokoll, dass der Updater nicht verfügbar ist, repariere oder installiere ShroudForge mit dem vollständigen Release-Paket neu und starte das Spiel danach erneut. Lass die `.bak`-Dateien während der Reparatur liegen.

## Manuelle Lösung für die Versionen 1.6.8, 1.7.0 und 1.7.3

Nutze diese Schritte nur mit den genannten älteren ShroudForge-Versionen.

1. Beende ShroudForge und Enshrouded vollständig. Beende auch einen eventuell laufenden Dedicated Server.
2. Öffne den Enshrouded-Installationsordner. Ein typischer Steam-Pfad ist `SteamLibrary\steamapps\common\Enshrouded`. Verwende den Bibliotheksordner, in dem dein Spiel installiert ist.
3. Falls ein anderer Mod die aktuellen `.kfc`-Dateien geändert haben könnte, überprüfe zuerst die Enshrouded-Dateien über Steam. Wenn du die alten Sicherungen behalten möchtest, kopiere sie vor dem nächsten Schritt aus dem Spielordner heraus.
4. Lösche ausschließlich diese Dateien, falls vorhanden:
   - `enshrouded.kfc.bak`
   - `enshrouded.kfc_resources.bak`
5. Starte ShroudForge erneut. Es sollte neue Arbeitskopien aus den aktuellen Spieldateien erstellen.

Lösche nicht die `.kfc`- oder `.kfc_resources`-Spieldateien und keine anderen Sicherungen. Bricht der Start weiterhin ab, lass die übrigen Dateien unverändert und sende dem Support das ShroudForge-Protokoll.
