# SF Unlimited Flight

This independent Lua mod uses the public ShroudForge runtime API. It is disabled
by default. When enabled, upward gliding and unlimited glider stamina default to
on and the speed multipliers default to 100 percent.

`Upward Gliding` uses `runtime.patch.available` and `runtime.patch.set_enabled`
to request `runtime.patch.glider_pitch`. The runtime resolves the original
`actor_rotation` system and its glider pitch clamp in each process. While the
intervention is active, the game continues to calculate each actor's movement
from that actor's own input. The intervention changes the pitch limit for all
gliding actors in the process, not for an individual player. Its activation is
independent of the shared glider item and of the number of players.

The mod requests restoration when the setting is disabled or the mod unloads.
A temporarily unavailable world does not by itself prove that the session ended.
A failed restore keeps unload pending through
the existing lifecycle. The native runtime also attempts restoration on shutdown.
Other mods can use the same public patch calls. As with other existing runtime
patches, the switch is process-wide and callers must coordinate its ownership.

For a dedicated server, the server and participating clients each need the
matching runtime and mod settings. The engine's replication and client
prediction behavior with this intervention has not yet been verified in a live
multiplayer session.

`Unlimited Glider Stamina` uses the per-entity stamina attribute. The four speed
sliders use loaded `ItemInfo` fields. These values are shared by all players using
the same glider type and are restored by the existing runtime override scope.
The mod uses a 100 ms update interval for its settings and data overrides. The
game performs the actual pitch calculation in its original execution path.
