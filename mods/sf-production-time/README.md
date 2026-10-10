# SF Production Time

Sets every positive `keen::RecipeInfo.craftingDuration` to the configured number
of seconds during asset preparation. Default: one base second. Instant recipes,
ingredients, output counts and unlock conditions are preserved. The world's
`factoryProductionSpeedFactor` still multiplies this duration.

Enable it with the same `seconds` value in the client and dedicated-server mod
settings, then prepare/start both installations. A running process retains its
already loaded recipes. This does not send client settings to the server.

This is an explicit asset mod using the existing Lua KFC API. It has no native
patch, hash list or build profile. Setting an absolute value avoids repeatedly
multiplying recipes that are already accelerated. The package is disabled by
default. Disabling it does not prove that an old `.bak` is unmodified. In an
earlier client investigation, the backup already contained accelerated recipe
values. Treat this as an observation from that installation, not as a guarantee
about every backup.

`sf-no-resource-cost` controls quantities in an inventory operation. it does not
control production time. Use that mod on the process running the production
system when free ingredients are wanted.
