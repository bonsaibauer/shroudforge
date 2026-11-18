# SF Unlimited Flight

Independent runtime mod using the public ShroudForge API. It has no dependency
on SF Survival or another mod. The mod itself is disabled by default. When
enabled, upward gliding starts on, unlimited glider stamina starts off, and the
speed multipliers start at 100 percent.
Runtime updates run every 100 ms to reduce repeated ECS and resource access.
Steering inputs and landing are sampled at that interval. The pitch guard still
uses the measured elapsed time, and restoration uses the same checked writers.

`Upward Gliding` controls the original glider pitch-rate input. The separate
`Unlimited Glider Stamina` setting sets the glider-specific stamina depletion
modifier to zero. The active actor action selects its actual loaded `ItemInfo`.
Rotation and movement remain in the original game systems. The four percentage
sliders multiply the glider's original yaw rate, forward acceleration and air
resistance coefficients. Higher lift or descend resistance increases drag.

Upward steering currently supports `YawSpeedPitchSpeed` controls with one player
in the authoritative world. Negative input uses a signed native pitch rate,
neutral input holds and positive input descends. The sampled controller reduces
its rate near 85 degrees upward using twice the largest update interval observed
during the current flight, with a minimum horizon of 250 ms. An inverted basis
starts recovery through the original positive rate and blocks further upward
acceleration until the input is released and orientation is upright again.
This is not a hard engine limit. A stalled Lua
callback can leave the last stored rate active longer than its sampling horizon.
Target-angle controls and steering several players through one shared ItemInfo
are unsupported. The original rate is restored on landing or unsupported input.

ItemInfo values are shared by every user of that glider resource. The public API
owns normalization, expected values, claims, original baselines and restoration.
Other mods use the same `runtime.overrides` target factories. Disable shared
settings before leaving the world and wait for restoration. The loader performs
cleanup automatically when this mod is disabled, unloaded or its callback fails.

The native data path has been executed privately through the original game
instructions. That includes 108 parameter cases, 120 rotation steps, 12
interpolation cases and 177 frames with the captured player's actual rotation
template. The user confirmed immediate upward flight in the first performance
build. Its vertical flip was subsequently captured and the sampled guard was
corrected. The user confirmed the installed R2 build working without problems.
Its 120-second API measurement had no mod errors. A separate quantitative R2
near-vertical angle capture was not obtained. These observations predate the
100 ms update interval, which has not yet been validated in game.
