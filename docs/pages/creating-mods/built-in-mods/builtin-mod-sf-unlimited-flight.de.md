---
{
  "id": "builtin-mod-sf-unlimited-flight",
  "locale": "de",
  "title": "SF Unlimited Flight",
  "summary": "Getrennte Einstellungen für Aufwärtsflug, Gleiter-Ausdauer, Drehrate, Vorwärtsbewegung und Luftwiderstand.",
  "navigation": {
    "path": [
      "Eingebaute Mods"
    ]
  },
  "source": {
    "file": "mods/sf-unlimited-flight/extended.mod.json",
    "pointer": "/settings"
  }
}
---

# SF Unlimited Flight

SF Unlimited Flight bietet sechs Einstellungen für das Fliegen. Das Paket zielt auf Client und Dedicated Server und verwendet die Laufzeitberechtigung `runtime`. Aktiviere die Mod beim Host oder auf dem Dedicated Server, damit sie die Spieler dieser Welt steuert.

| Einstellung | Startwert | Bereich und Wirkung |
| --- | --- | --- |
| `unlimitedFlying` | `true` | Kontrollkästchen. Steuert die Neigung jedes gleitenden Spielers unabhängig und ermöglicht Aufwärtsflug. |
| `unlimitedGliderStamina` | `true` | Kontrollkästchen. Verhindert Ausdauerverbrauch beim Gleiten. |
| `turnSpeed` | `100` | 25 bis 400 Prozent, in 5er-Schritten. Multipliziert die ursprüngliche Gleiter-Drehrate. |
| `moveSpeed` | `100` | 25 bis 400 Prozent, in 5er-Schritten. Multipliziert die ursprüngliche Vorwärtsbeschleunigung. |
| `liftSpeed` | `100` | 25 bis 400 Prozent, in 5er-Schritten. Multipliziert den Luftwiderstand in Längsrichtung. Ein höherer Wert bedeutet mehr Widerstand. |
| `descendSpeed` | `100` | 25 bis 400 Prozent, in 5er-Schritten. Multipliziert den vertikalen Luftwiderstand. Ein höherer Wert bedeutet mehr Widerstand. |

Das Paket selbst ist zunächst deaktiviert. Es liest die Schlüssel aus seiner `extended.mod.json`. Die Neigungssteuerung unterstützt Eingaben als Drehgeschwindigkeit und als Zielwinkel. Im Mod gibt es keine Begrenzung der Spielerzahl. Jede Spielereingabe steuert die Neigung ihrer eigenen Entity. Die vier Prozentregler verwenden die gemeinsamen Koeffizienten des aktiven Gleiters und betreffen alle Spieler mit demselben Gleitertyp.

Die [Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/sf-unlimited-flight/README.md) enthält die technischen Grenzen und Wiederherstellungsregeln.
