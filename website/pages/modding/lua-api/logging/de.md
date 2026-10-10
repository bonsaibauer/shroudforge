# Verständliche Mod-Meldungen schreiben

Nutze `shroudforge.log` für technische Abläufe des Mods. Eine gute Meldung nennt Aktion und Ergebnis, ein Fehler nennt zusätzlich den Grund. Die genaue gemeinsame Richtlinie steht in [Logging für Mods und Loader](https://github.com/bonsaibauer/shroudforge/blob/main/docs/sf/logging.md).

```lua
shroudforge.log.info("Blueprint export started")
shroudforge.log.warn("Blueprint export paused, destination is unavailable")
shroudforge.log.error("Blueprint export failed, write verification did not match")
```

Nutze `INFO` für wichtige Vorgänge und bestätigte Ergebnisse, `WARN` für blockierte oder unklare Abläufe, `ERROR` für echte Fehlschläge. `DEBUG` und `TRACE` sind für Diagnosewerte und Detailverläufe. Stelle identische Fehler nicht still ab, erkläre stattdessen die Rolle jeder Meldung, wenn mehrere Ebenen beteiligt sind.

Im Client und Dedicated Server werden getrennte ShroudForge-Logs geschrieben. Die effektive Mindeststufe steuert, was gespeichert wird. Der Debug-Console-Filter blendet nur gespeicherte Zeilen aus oder ein.
