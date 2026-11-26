--- @meta

--- @class RuntimeOverlayApi
--- @class RuntimeOverlayScene
--- @field rectangles? RuntimeOverlayRectangle[] Filled pixel rectangles, drawn first.
--- @field textures? table[] Textured quads, drawn after rectangles.
--- @field programs? table[] Validated SPIR-V fragment draws, drawn last.
--- @field target_generation? string Required for programs, from overlay.status().target.device_generation when target.valid. Changed targets skip the scene until resubmission.
--- @class RuntimeOverlayRectangle
--- @field x integer Left pixel, -16384 to 16384.
--- @field y integer Top pixel, -16384 to 16384.
--- @field width integer Width, 1 to 16384.
--- @field height integer Height, 1 to 16384.
--- @field rgba integer Packed straight-alpha RRGGBBAA color.

--- Read a detached frame from the same captured store used by native mods.
--- Requires active ingame client lifecycle, runtime/runtime-map capabilities and exact-profile startup preparation.
--- runtime.has admits API access. Inspect frame/view status and supported_features before using values.
--- Status: 0 ready, 1 disabled, 2 unsupported, 3 no session, 4 not captured, 5 stale, 6 wrong generation.
--- player has separate pose/camera. Collections have meta/items. Fog has meta/tiles_hex/byte_count. Environment has clock_meta/weather_meta.
--- Positions are baseline Q32.32 world units, headings radians. No physical unit claim. uint64 identities/times/generations are decimal strings.
--- Discovery positions and legacy unproved iterator capture are unavailable. No coordinate scanning is performed.
---@param expected_session? string Earlier map frame session_generation, omit to accept current. Separate from runtime.world.session_id().
---@return table? frame Detached frame with status, state_flags, session_generation, clock_domain, now_ms, supported_features, player, world_markers, remote_players, deaths, waypoints, pings, discovery, fog, environment.
---@return string? reason API/provider acquisition failure. A non-ready view is reported inside the frame.
--- ### Example
--- ```lua
--- local frame, reason = runtime.world.map.get()
--- if frame and frame.player.meta.status == 0 then print(frame.player.pose.position[1]) end
--- ```
function runtime.world.map.get(expected_session) end

--- Read the environment view of the same detached map frame. Same runtime/runtime-map access contract as map.get.
--- Clock and weather have independent status/freshness. No write or weather override is performed.
---@param expected_session? string Earlier map frame session_generation.
---@return table? environment clock_meta, weather_meta, minute_of_day, daytime, weather_kind.
---@return string? reason API/provider acquisition failure.
--- ### Example
--- ```lua
--- local environment = runtime.world.environment.get()
--- if environment and environment.clock_meta.status == 0 then print(environment.minute_of_day) end
--- ```
function runtime.world.environment.get(expected_session) end

--- Read captured HUD/session flags. Same runtime/runtime-map access contract as map.get.
--- Bits: 1 active, 2 gameplay, 4 menu, 8 focused. supported=false means HUD unavailable.
---@param expected_session? string Earlier map frame session_generation.
---@return table? hud status, supported, state_flags, session_generation, now_ms, clock_domain.
---@return string? reason API/provider acquisition failure.
--- ### Example
--- ```lua
--- local hud = runtime.world.hud.get()
--- if hud and hud.supported and hud.status == 0 then print(hud.state_flags) end
--- ```
function runtime.world.hud.get(expected_session) end

--- Replace this package generation's immutable scene through the shared native renderer.
--- Requires active ingame client, runtime/runtime-overlay and early graphics preparation. No Lua callback runs on the render thread.
--- Max 4096 draws and 8 MiB UTF-8 JSON. Table input uses the existing 100000-value conversion limit. Use a JSON string for larger scenes.
--- Textures: {texture={id="1",revision="1",width=1,height=1,rgba={255,255,255,255}},x=0,y=0,width=32,height=32}.
--- Optional source_x/y/width/height, clip_center_x/y/radius, rotation_radians, tint_rgba and square_clip match native overlay V2.
--- Programs: {id="1",revision="1",spirv={word,...},x=0,y=0,width=32,height=32,textures={texture,...},constants={byte,...}}.
--- Programs use the native Vulkan 1.0/SPIR-V fragment contract, four texture bindings, at most 128 aligned constant bytes and no implicit HDR color conversion.
--- Nonzero decimal string IDs/revisions identify immutable content. Change revision when content changes. Omit unused arrays. Empty object clears.
--- Native callbacks and copied scenes are mutually exclusive within a package generation. Invalid input preserves its previous scene.
--- Success means copied/queued. Use status() for renderer progress. Disable/unload revokes and drains the same owner.
---@param scene RuntimeOverlayScene|string Drawing description or complete UTF-8 JSON.
---@return boolean accepted Scene published.
---@return string? reason Access or validation/submission failure.
--- ### Example
--- ```lua
--- local ok, reason = runtime.overlay.submit({rectangles={{x=20,y=20,width=100,height=30,rgba=0x3366ccff}}})
--- ```
function runtime.overlay.submit(scene) end

--- Clear this package generation's scene, with the same contract as submit.
---@return boolean cleared Submission accepted.
---@return string? reason Failure reason.
--- ### Example
--- ```lua
--- runtime.overlay.clear()
--- ```
function runtime.overlay.clear() end

--- Read shared renderer progress. Same client/capability/lifecycle/preparation contract as submit.
--- status=0 ready, 3 unavailable, 6 busy, 7 failed. reason uses SFOverlayReasonV1 in the native SDK.
---@return table? status status, reason, device_generation, rendered_frames, busy_frames, failed_frames, target. target includes valid, width/height, encoding, device_generation and HDR nits. uint64 counters are strings.
---@return string? reason Access/provider failure.
--- ### Example
--- ```lua
--- local status = runtime.overlay.status()
--- if status then print(status.status, status.rendered_frames) end
--- ```
function runtime.overlay.status() end
