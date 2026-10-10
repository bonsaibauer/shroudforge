# Write useful mod log messages

Use `shroudforge.log` for technical mod activity. A useful message names the action and result, while an error also explains why it failed. The shared policy is in [Logging for mods and the loader](https://github.com/bonsaibauer/shroudforge/blob/main/docs/sf/logging.md).

```lua
shroudforge.log.info("Blueprint export started")
shroudforge.log.warn("Blueprint export paused, destination is unavailable")
shroudforge.log.error("Blueprint export failed, write verification did not match")
```

Use `INFO` for important operations and confirmed results, `WARN` for blocked or uncertain flows, and `ERROR` for actual failures. Use `DEBUG` and `TRACE` for diagnostic values and detailed traces. Do not silently suppress a duplicate failure, explain the role of each message when more than one layer is involved.

Client and Dedicated Server write separate ShroudForge logs. The effective minimum level controls what gets saved. The Debug Console filter only hides or shows lines that are already stored.
