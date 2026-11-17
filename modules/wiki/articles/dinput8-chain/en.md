# DInput8 chain loading standard

Many Enshrouded mod loaders and DLL mods want to be the game's `dinput8.dll`, and only one file can have that name. A loader that holds the name starts the other DLLs listed in `dinput8-chain.ini`, next to the game's exe. EMF, Kiln, ShroudForge, XHL-Core and Auto Loot Plus read this file. This page writes down the rules, so that every loader reads the file the same way and a player's list works whichever loader holds `dinput8.dll`.

This page is in the public domain (CC0): copy it into your own project's docs, change it, no credit needed. Changes that more than one loader agrees on are welcome as pull requests to EMF's repository.

The words must and should below are what a loader that follows this spec does.

## The file

```ini
; my mods
[Chain]
mods\mymod\mymod.dll
autolootplus.dll

[EMF]
creative.dll
```

- Where: `dinput8-chain.ini` in the folder of the game's exe (`enshrouded.exe` or `enshrouded_server.exe`).
- Encoding: a loader must read UTF-8 with or without a BOM, and UTF-16 LE with a BOM. It should also read the system's ANSI code page when the bytes are not valid UTF-8 (a file saved by an old Notepad). A tool that writes the file must keep the encoding and line breaks it found.
- Lines: blank lines are skipped. A line starting with `;`, `#` or `//` is a comment. `; ` in front of a DLL's line switches that DLL off, so players can turn one off without deleting the line. A comment may follow a name after a space or tab (`mymod.dll ; my mod`), and a name may stand in double quotes.
- Sections (names are not case-sensitive): `[Chain]` is shared: every loader that follows this spec starts its DLLs. Any other section belongs to one tool, which names it after itself (`[EMF]`, `[Kiln]`): other loaders must ignore it. Lines before any section count as `[Chain]` (older files have no section at all).
- `key=value` lines: a line `name=value` counts as a DLL entry when its value names a `.dll`. Other keys are settings of the tool that wrote them; other loaders ignore them.

## The DLLs

- Paths are relative to the exe's folder. `/` is read as `\`. A loader must refuse absolute paths, drive letters, UNC paths (`\\server\...`) and any `..` part: a chain file must not reach outside the game folder.
- Only `.dll` files. A line naming anything else is skipped and reported.
- Never `dinput8.dll` itself, and a loader must not start another copy of itself. EMF, for example, recognises its own loader under another name and does nothing in that copy.
- Each DLL once. When a DLL is listed twice (in `[Chain]` and a tool's own section, or twice in one section), it is started once, at its first place.
- Order: DLLs are started in the order of their lines, top to bottom, `[Chain]` and the loader's own section in the order they appear in the file.
- When: from the loader's own `DllMain` (or right after it, before any game code runs), with `LoadLibrary`. This is the moment a chained DLL would have started had it been `dinput8.dll` itself.
- At most 256 entries count. More are reported and skipped.
- Failures: a missing DLL, or one that fails to load (Windows' error 126, or 1114 when its `DllMain` failed), is skipped and reported. The game goes on.

## Reporting

A loader should log one line per chained DLL: started, skipped (switched off, refused, already loaded) or failed with Windows' error number. EMF also writes the same as a table to `emf\chain_status.txt`, so players can paste it.

## When you move another tool's dinput8.dll

An installer that finds another tool's `dinput8.dll` should rename it (never delete it), list the new name in its own section of `dinput8-chain.ini`, and put it back when uninstalled. EMF names it `<tool>.dll` when it recognises the tool, else after the DLL's version information, else `dinput8-previous.dll`, and records what it moved in `emf\moved.txt`.

## Exports

The game calls `DirectInput8Create` on whoever holds `dinput8.dll`. The loader must forward that call to Windows' own `dinput8.dll` (`%SystemRoot%\System32\dinput8.dll`). Chained DLLs do not receive DirectInput calls: a mod that does its work in `DirectInput8Create` must move it to `DllMain` or a thread it starts.

## Who follows it

| Tool | Reads `[Chain]` | Own section |
| --- | --- | --- |
| EMF | yes (also lines before any section) | `[EMF]` (DLLs its installer renamed) |
| ShroudForge | yes (it wants each entry inside `mods\<folder>\` and reports the others) | |
| Kiln | yes (also `[EMF]`, for DLLs EMF's older installers renamed) | none: its own list is `kiln\chain.ini` (quoted names and a comment after a name: planned) |
| XHL-Core, Auto Loot Plus | yes | |

Tell us when your tool reads the file, or reads it differently, and we will update the table.

## Source

[EMF shared dinput8-chain.ini specification](https://github.com/matenm/emf-enshrouded/blob/main/docs/modding/dinput8-chain.md). The original specification is dedicated to the public domain under CC0.
