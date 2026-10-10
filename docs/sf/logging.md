# Logging rules for ShroudForge and mods

This is the shared logging standard for ShroudForge modules and mods. Logs should let a player answer three questions quickly: **Did the mod start? What important operation did it perform? If it stopped, what happened and what can I do next?**

## Choose a level by usefulness to the player

| Level | Use it for | Examples |
| --- | --- | --- |
| **ERROR** | An operation failed or data may be incomplete, invalid, or unsafe to use. State what failed and the consequence. | A blueprint could not be written or verified. Recovery did not finish. A required UI script failed. |
| **WARN** | Work was blocked, paused, degraded, or its result is uncertain, but the application can continue. Include the reason and whether a request was sent. | A required server ID is missing (`request=not-sent`). A timeout leaves the server result unknown. An optional runtime feature is unavailable. |
| **INFO** | A meaningful state change, operation start, major phase, or confirmed result that helps explain mod behavior. | Mod initialized. Local or dedicated-server target selected. Operation started. Server confirmed live placement. |
| **DEBUG** | Useful investigation details that most players do not need during normal use. Keep them event-based rather than per frame. | Key/button input, window shown/hidden, effective settings and their source, one-time readiness wait details. |
| **TRACE** | High-volume or exhaustive diagnostics needed to reconstruct exactly what the UI or runtime observed. | Full visible UI state snapshots, repeated polling details, per-item/per-step data. |

Use the most severe level justified by the impact. A recoverable block is WARN. A confirmed failure is ERROR, not a vague WARN. Do not log successful routine checks as INFO.

## Write messages people can act on

- Start with the mod or module name/category, then name the action and its phase or outcome.
- Use plain English. Say what was expected, what happened, and the reason or next step when known.
- For multi-step work, reuse one operation number for start, meaningful phase changes, and final result. Give a blocked attempt an operation number too.
- Say whether a remote request was sent. Distinguish live-world confirmation from save persistence when persistence was not checked.
- Log a visible warning or error at WARN or ERROR with the same human-readable cause. Do not rely on a TRACE UI snapshot to explain a player-facing failure.
- Avoid secrets, credentials, and unnecessary personal data. Put full identifiers and payload detail at DEBUG or TRACE only when needed to diagnose the feature.

## Keep volume useful

- Do not write per-frame messages, per-second timers, unchanged state, or repeated successful polling results.
- Log meaningful state transitions rather than background polling samples. Every emitted user-visible UI message is retained at its stated level. Full UI snapshots are emitted at TRACE when the visible state changes.
- DEBUG and TRACE are real log levels. DEBUG entries are recorded when `logging.minimumLevel` is DEBUG or TRACE. TRACE entries are recorded only when it is TRACE.

Network health probes are routine checks. Log the first confirmed probe for a peer as INFO, repeated acknowledgements as DEBUG, and recovery after 15 seconds without new failures as INFO when a later probe is confirmed. Log the first send failure or timeout in an outage as WARN. Keep repeated failures in the same outage at DEBUG so a five-second probe interval does not flood the main log, even when another peer is responding.

For example, the player should see a short INFO trail for meaningful work:

```text
[I ...] [world-editor] Operation 12 started: placing blueprint "Keep" through Dedicated Server P2P.
[I ...] [world-editor] Operation 12: server confirmed 1,071 terrain cells in the live world. Save persistence was not checked.
```

The reason an attempt did not proceed belongs at WARN with the same operation number:

```text
[W ...] [world-editor] Operation 12 blocked: Dedicated Server SteamID64 is missing. Request was not sent.
```

An ERROR should identify the failed step and impact, for example: `Operation 12 failed: the server could not verify the terrain write. The blueprint may be incomplete.`

## Stored level and Debug Console filter

`logging.minimumLevel` controls what is written to the ShroudForge log. The default is INFO. Entries below that threshold are discarded when the logger writes. The Debug Console cannot bring them back later.

The Debug Console's **Display** selector is a separate view filter. It can hide recorded entries, and **All** means all entries that were actually recorded. It does not change the saved log or the configured minimum level. TRACE, DEBUG, INFO, WARN, and ERROR are the five log levels. All is only a display choice.

## World Editor application

The World Editor follows these rules. Its operation phases and results use INFO, blocked or uncertain P2P work uses WARN, failed writes or verification use ERROR, input and window visibility use DEBUG, and complete visible UI snapshots use TRACE. Player-facing notices are written to the same role-appropriate ShroudForge log as structured UI-message entries. There is no separate UI log. See the [World Editor guide](../../mods/world-editor/README.md).
