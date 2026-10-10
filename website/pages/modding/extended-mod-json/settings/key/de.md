# Schlüssel `settings.<key>`

Der Platzhalter `<key>` wird durch einen Namen ersetzt, den der Mod-Autor wählt. Zum Beispiel heißt `settings.flightSpeed` eine Einstellung `flightSpeed`.

Erlaubt sind 1 bis 80 Buchstaben, Zahlen, Punkte, Bindestriche und Unterstriche. Groß- und Kleinschreibung zählt. Lua verwendet exakt denselben Namen: `shroudforge.settings.get("flightSpeed", 1.0)`.

Dieser Schlüssel ist kein globaler ShroudForge-Schlüssel. Jeder Mod hat seine eigenen Einstellungswerte.
