# SF Unlimited Flight

Independent runtime mod using the public ShroudForge API. It has no dependency
on SF Survival or another mod. The mod itself is disabled by default. When
enabled, upward gliding and unlimited glider stamina start on, and the
speed multipliers start at 100 percent.
Runtime updates run every 100 ms to reduce repeated ECS and resource access.
Steering inputs and landing are sampled at that interval. Each pitch movement
uses at most 100 ms of measured elapsed time. Restoration uses checked writers.

`Upward Gliding` steers each gliding player's ECS orientation independently. The separate
`Unlimited Glider Stamina` setting sets the glider-specific stamina depletion
modifier to zero. The active actor action selects its actual loaded `ItemInfo`.
The original game systems continue to calculate yaw, bank and movement. The four percentage
sliders multiply the glider's original yaw rate, forward acceleration and air
resistance coefficients. Higher lift or descend resistance increases drag.

There is no player-count limit in the mod. Run it in the authoritative process:
the host for a hosted world, or the dedicated server for a server world.
A joined client with read-only replica access cannot control server entities.
`YawSpeedPitchSpeed` integrates each player's input at the original glider pitch
rate. Negative input climbs, neutral holds and positive input descends.
`YawSpeedPitchTargetAngle` steers toward that player's input times 90 degrees,
using the same original rate. Individual movement steps stop at 85 degrees
upward and 80 degrees downward. An inverted orientation blocks scripted pitch
movement until upright and the climb input has been released. Recovery state
belongs to the individual entity and ends when that player's glider use ends.

The original pitch calculation begins with each entity's `CurrentTransform`.
A shared `ItemInfo.pitchAngleSpeed` of zero makes the native pitch step hold
that current angle. The mod applies this same zero to all users of a resource,
then writes each player's finite pitch step through `runtime.ecs.write_fields`
with the expected starting quaternion. Player inputs never select a shared rate.
A busy or changed transform skips that player's step until the next update.
A Lua pause leaves the native pitch rate at zero. No stored negative rate can
continue rotating the player past the scripted bound during the pause.

All active users of a glider resource are admitted before its pitch rate is
held. An unsupported input mode restores that group's original native rate.
The original rate is also restored after its last active glider user lands,
on disable and on unload. Current orientations are movement state, so cleanup
does not rotate players back to a pose captured earlier in their flight.

ItemInfo values are shared by every user of that glider resource. The public API
owns normalization, expected values, claims, original baselines and restoration.
Other mods use the same `runtime.overrides` target factories. Disable shared
settings before leaving the world and wait for restoration. The loader performs
cleanup automatically when this mod is disabled, unloaded or its callback fails.

The earlier single-player native rate data path was executed privately through the original game
instructions. That includes 108 parameter cases, 120 rotation steps, 12
interpolation cases and 177 frames with the captured player's actual rotation
template. The user confirmed immediate upward flight in the first performance
build. Its vertical flip was subsequently captured and the sampled guard was
corrected. The user confirmed the installed R2 build working without problems.
Its 120-second API measurement had no mod errors. These historical observations
do not validate the new per-entity movement path. Current client and server
instruction evidence confirms native pitch holding at rate zero. A fresh server
capture confirms the writable untracked `CurrentTransform` registration.
Lua fixtures cover simultaneous players and restoration. Visible multiplayer
flight and client prediction with this implementation remain unverified.
