# World Editor

Weltbereiche kopieren, drehen, speichern und einfügen.

## Paketangaben

- Mod-ID: `world-editor`
- Zielprozesse: Client und Dedicated Server
- Laufzeitberechtigung: ja
- Deklarierte spielerseitige Einstellungen: 37

## Einstellungen

## Fenster im Spiel

Die Position bezieht sich auf das Spielfenster und wird in logischen Pixeln gemessen. X = −1 zentriert das Fenster. Y wird vom oberen Rand nach unten gemessen. Änderungen gelten während des laufenden Spiels.

- [Breite des Blueprint-Fensters](#doc-builtin-world-editor-panelwidth)
- [Höhe des Blueprint-Fensters](#doc-builtin-world-editor-panelheight)
- [Fensterposition X, minus eins zentriert](#doc-builtin-world-editor-panelpositionx)
- [Fensterposition Y ab oberem Rand](#doc-builtin-world-editor-panelpositiony)

## Grenzen des World Editors

- [Maximale Props pro Blueprint](#doc-builtin-world-editor-maximumcopyableprops)

## Native Entity-Operationen

Erstellen und Platzieren verwenden native Spieloperationen.

- [Obere Hälfte der Template-UUID in Hexadezimal](#doc-builtin-world-editor-templateuuidhigh)
- [Untere Hälfte der Template-UUID in Hexadezimal](#doc-builtin-world-editor-templateuuidlow)
- [Tracking-ID](#doc-builtin-world-editor-trackingid)
- [Entity-Position X](#doc-builtin-world-editor-entityx)
- [Entity-Position Y](#doc-builtin-world-editor-entityy)
- [Entity-Position Z](#doc-builtin-world-editor-entityz)
- [Material-Feedback-ID nur für Platzierung](#doc-builtin-world-editor-feedbackid)
- [Untere Begrenzung X](#doc-builtin-world-editor-boundsminx)
- [Untere Begrenzung Y](#doc-builtin-world-editor-boundsminy)
- [Untere Begrenzung Z](#doc-builtin-world-editor-boundsminz)
- [Obere Begrenzung X](#doc-builtin-world-editor-boundsmaxx)
- [Obere Begrenzung Y](#doc-builtin-world-editor-boundsmaxy)
- [Obere Begrenzung Z](#doc-builtin-world-editor-boundsmaxz)

## World Editor

Voxel- und Prop-Blueprints oder reine Prop-Blueprints aufnehmen, prüfen, einfügen und über native Welt-APIs rückgängig machen.

- [Quellzelle X als manueller Ersatzwert](#doc-builtin-world-editor-sourcex)
- [Quellzelle Y als manueller Ersatzwert](#doc-builtin-world-editor-sourcey)
- [Quellzelle Z als manueller Ersatzwert](#doc-builtin-world-editor-sourcez)
- [Auswahlgröße X als manueller Ersatzwert](#doc-builtin-world-editor-sizex)
- [Auswahlgröße Y als manueller Ersatzwert](#doc-builtin-world-editor-sizey)
- [Auswahlgröße Z als manueller Ersatzwert](#doc-builtin-world-editor-sizez)
- [Dauerhafter Blueprint-Name](#doc-builtin-world-editor-blueprintname)
- [Neuer Name beim Umbenennen oder Duplizieren](#doc-builtin-world-editor-blueprintnewname)
- [Drehung vor dem Einfügen](#doc-builtin-world-editor-rotationquarterturns)
- [Obere Blueprint-Achse](#doc-builtin-world-editor-rotationaxis)
- [Voxel-Modus beim Einfügen](#doc-builtin-world-editor-pastevoxelmode)
- [Props am Einfügeziel](#doc-builtin-world-editor-targetpropmode)
- [Zielzelle X](#doc-builtin-world-editor-targetx)
- [Zielzelle Y](#doc-builtin-world-editor-targety)
- [Zielzelle Z](#doc-builtin-world-editor-targetz)
- [Weltposition X für Props-only-Blueprints](#doc-builtin-world-editor-targetworldx)
- [Weltposition Y für Props-only-Blueprints](#doc-builtin-world-editor-targetworldy)
- [Weltposition Z für Props-only-Blueprints](#doc-builtin-world-editor-targetworldz)
- [Interner Entity-Handle](#doc-builtin-world-editor-entityhandle)

Die Einstellungen werden in `extended.mod.json` des Mod-Pakets gespeichert. Der Mod liest sie über `shroudforge.settings.get(key, fallback)`. Weitere Details stehen in der [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).
