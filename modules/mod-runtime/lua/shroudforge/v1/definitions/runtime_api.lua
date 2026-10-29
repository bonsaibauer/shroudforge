--- @meta

--- The execution-facing part of the single ShroudForge API.
--- Its shape is identical before and inside the game; `phase` and `has()` report availability.
--- @class RuntimeApi
--- @field schema_version integer Version of the runtime API schema.
--- @field types TypeRegistry Same registry object as game.types; includes all reflection entries.
--- @field phase "pregame"|"ingame" Whether the game is preparing startup data or is running.
--- @field is_client boolean True when this mod runs in a player client process.
--- @field is_server boolean True when this mod runs in a Dedicated Server process.
--- @field player RuntimePlayerApi Cached local character selection from the main menu.
--- @field network RuntimeNetworkApi Steam Networking Messages P2P channel on the current client or dedicated-server process.
--- @field services RuntimeServicesApi Reflected views of original named engine DI services with independently validated plain-data storage contracts.
--- @field impact RuntimeImpactApi Checked semantic calls to original named Impact operations.
--- @field overrides RuntimeOverridesApi Typed reversible changes with shared storage claims and automatic mod-scope cleanup.
--- @field report_effect fun(state:'waiting'|'no-target'|'no-change'|'write-confirmed'|'write-failed', detail:string?):boolean,string? Report an observed ECS action state. A confirmed write means typed memory was written, not that the gameplay effect was independently verified.

--- @class RuntimeLifecycle
--- @field on_load fun()? Called when the runtime mod starts.
--- @field on_update fun(delta_seconds:number)? Called at the configured update interval while the mod is active.
--- @field on_unload fun()? Called when the runtime mod stops.
--- @field update_interval_ms integer? Minimum 8, maximum 1000, default 50. Missed intervals are not replayed.
runtime = {}

--- Return the cached original client GameFlow state, using the same source as the Modloader UI.
--- True means PreGame, including its selection/browser screens. False means a validated other state.
--- Returns nil and a reason during transitions, before detection, or when the provider is unavailable.
--- Reads only the existing native cache and does not require world access or an execution profile.
--- Requires an active runtime mod with the runtime capability.
--- @return boolean? main_menu
--- @return string? reason
function runtime.is_main_menu() end

---@class RuntimePlayerApi
runtime.player = {}
--- Return the cached name selected in the local client's main menu, including Unicode.
--- The native menu reads the name on the first selection and when selection/name metadata changes.
--- This getter performs no game-memory scan or refresh. It does not require an active world.
--- Returns nil and a reason before a name is available and on Dedicated Servers.
--- Requires an active runtime mod with the runtime capability.
---@return string? name
---@return string? reason
function runtime.player.name() end

--- Reversible scalar changes through the common typed writers.
--- Targets resolve original storage identities. Independent mods receive exclusive
--- storage claims, so they cannot capture another mod's modified baseline.
--- Registered trackers are restored by the loader on disable, errors and unload,
--- including mods without on_unload. Busy cleanup retains its writer scope and retries.
---@class RuntimeOverridesApi
runtime.overrides = {}
---@class RuntimeOverrides
---@field fault string? Uncertain outcome that prevents later helper writes.
---@field writes integer Confirmed writes in the current update.
---@field waiting_count integer Operations waiting for a valid snapshot or safe restore.
---@field details table<string, boolean> Distinct waiting details for the current update.
local RuntimeOverrides = {}
---Create an independent ownership tracker for this mod instance.
---@return RuntimeOverrides
function runtime.overrides.new() end
---@class RuntimeWriteClaim
local RuntimeWriteClaim = {}
---@return boolean acquired
---@return string? reason
function RuntimeWriteClaim:acquire() end
function RuntimeWriteClaim:release() end
---@class RuntimeOverrideTarget
---@field key string Stable target identity used by apply. Do not replace this key.
---@field claim RuntimeWriteClaim Original storage claim bound to this mod and world.
---@field shared boolean? Whether the stored input is shared by world participants.
---@field read fun():table?,string? Returns the owned observed scalar plus dependent values.
---@field write fun(snapshot:table,value:unknown):boolean,unknown,string?,unknown Uses the guarded public writer and its confirmed result.
---@field normalize fun(snapshot:table,value:unknown):unknown?,string? Applies the original scalar encoding or attribute program.
---@class RuntimeAttributeSnapshotContext
---@field active boolean True only during one update. Set false before retaining a target for cleanup.
---@field session integer? World session for the cached root.
---@field entity integer? Entity owning the cached root.
---@field root_hash integer? Root identity returned by the attribute catalog.
---@field component_name string? Resolved component layout for the cached root.
---@field snapshot table? Latest complete root snapshot or confirmed update result.
---Bind an original named attribute on one current entity. Full root values guard dependent inputs.
---A shared snapshot context is limited to one update and one entity/root. The guarded writer checks the full snapshot before every mutation.
---@param entity integer
---@param selector string|integer Original attribute name or stored ID.
---@param snapshot_context RuntimeAttributeSnapshotContext? Optional per-update context shared by attributes of the same entity and root.
---@return RuntimeOverrideTarget? target
---@return string? reason
function runtime.overrides.attribute(entity, selector, snapshot_context) end
---Bind one inline scalar. The original component/root calculator remains authoritative.
---@param entity integer
---@param component TypeSelector
---@param path string
---@param expected_paths string[]? Additional fields used to calculate the target.
---@return RuntimeOverrideTarget? target
---@return string? reason
function runtime.overrides.component(entity, component, path, expected_paths) end
---@param resource Resource Current loaded resource snapshot.
---@param path string Reflected naturally aligned 1/2/4/8-byte scalar path.
---@return RuntimeOverrideTarget? target
---@return string? reason
function runtime.overrides.resource(resource, path) end
---@param view RuntimeTemplateComponent
---@param path string
---@param expected_paths string[]?
---@return RuntimeOverrideTarget? target
---@return string? reason
function runtime.overrides.template(view, path, expected_paths) end
---@param view RuntimeService
---@param path string
---@param expected_paths string[]?
---@return RuntimeOverrideTarget? target
---@return string? reason
function runtime.overrides.service(view, path, expected_paths) end
---Advanced custom targets must obtain a claim from these same canonical factories.
---@return RuntimeWriteClaim? claim
---@return string? reason
function runtime.overrides.get_component_claim(entity, component) end
---@return RuntimeWriteClaim? claim
---@return string? reason
function runtime.overrides.get_resource_claim(resource) end
---@return RuntimeWriteClaim? claim
---@return string? reason
function runtime.overrides.get_template_claim(view) end
---@return RuntimeWriteClaim? claim
---@return string? reason
function runtime.overrides.get_service_claim(view) end
---Reset per-update counters and selections. Validated world changes discard old
---entity state. Unrestored shared inputs retain their claims and fault until restart.
function RuntimeOverrides:begin_update() end
---Record an unavailable or conflicting operation without treating it as a write.
---@param reason string
function RuntimeOverrides:waiting(reason) end
---Apply one owned reversible setting. adapter.read returns {value=..., values=...}; adapter.write must compare its observed snapshot and return applied, actual, reason, canonical.
---The helper preserves the first baseline, detects external edits, and reports uncertain writes without blindly retrying them.
---@param key string Target key. A typed target's canonical key takes precedence.
---@param setting string Setting name used by restore_disabled.
---@param adapter RuntimeOverrideTarget Typed reader/writer with an API ownership claim.
---@param desired fun(snapshot:table, baseline:unknown):unknown?,string? Calculate the target from the current snapshot and preserved baseline.
---@param shared boolean? True when all world participants share this stored value.
function RuntimeOverrides:apply(key, setting, adapter, desired, shared) end
---Restore values whose settings are disabled, preserving conflicts and retryable busy states.
---@param enabled fun(setting:string):boolean
function RuntimeOverrides:restore_disabled(enabled) end
---Restore every still-owned value through its original adapter.
function RuntimeOverrides:restore_all() end
---Restore dynamic targets not selected by apply since begin_update, such as a glider no longer used.
function RuntimeOverrides:restore_unused() end
---@return boolean
function RuntimeOverrides:has_ownership() end
---@return boolean
function RuntimeOverrides:has_shared_ownership() end
---Forget expired entity ownership. Refuses to discard unrestored shared ownership.
---@return boolean reset
---@return string? reason
function RuntimeOverrides:reset() end
---@param message string
---@return string
function RuntimeOverrides:summary(message) end

---@class RuntimeServicesApi
runtime.services = {}
---@class RuntimeService
---@field key string Exact original DI name, for example ecs::GameSettings.
---@field type Type Corresponding qualified reflected type, for example keen::ecs::GameSettings.
---@field scope 'host-world-shared' The authoritative world's consumers share this service.
---@field session_id integer The original nonzero world session. Views cannot authorize another session.
---@field write_supported boolean A successful view has an original mutable plain-data binding. A current Scene gate, capability and unfaulted session are still required for every write.
local RuntimeService = {}
---Resolve an exact original service name through the engine's named registry and mutable system binding.
---All mods use the same reflection codec and expected-value write path. No feature name, RVA or hash belongs in a mod.
---Only independently validated inline plain-data contracts are admitted by the exact-image provider profile.
---Missing contracts, unreflected services, pointer-owning layouts and inactive or changing worlds return nil and a reason.
---Requires an active mod with runtime.ecs.read. Services currently require a local authoritative world.
---@param key string Original DI name without the keen:: prefix.
---@return RuntimeService? service
---@return string? reason
function runtime.services.get(key) end
---Read a fresh original service snapshot under the authoritative world's Scene and lifetime guards.
---Uses reflected inline field paths, including nesting and one-based static array indices.
---@param paths string[] Nonempty dense array of distinct field paths, at most 1024 paths.
---@return table<string, unknown>? values
---@return string? reason
function RuntimeService:read_fields(paths) end
---Compare and write one reflected inline field, preserving all other service bytes.
---Requires runtime.ecs.write and the same current session. expected is mandatory.
---The provider compares the entire fresh service snapshot and confirms the result and binding while its original Scene gate is held.
---Optional expected_fields checks other service-relative input values before that full-snapshot CAS.
---A conflict changes nothing. Uncertain writes stop further writes in the session.
---Does not synthesize engine events, UI refreshes or persistence notifications.
---@param path string Reflected path relative to the service type.
---@param value unknown Value compatible with the reflected inline field.
---@param expected unknown Expected current field value, including false for boolean fields.
---@param expected_fields table<string, unknown>? Nonempty map of at most 1024 service-relative expected fields.
---@return boolean ok
---@return string? reason
function RuntimeService:write_field(path, value, expected, expected_fields) end

---@class RuntimeImpactApi
runtime.impact = {}
---Invoke an independently validated original operation by its reflected type.
---Currently supports only keen::impact_nodes::IsSameEntity. The provider resolves
---the actual isSameEntity registration in the original ImpactSystem name table.
---The original callback receives owned VM buffers after all Scene/session gates
---are released. No executable patch, function-address search or per-build operation
---profile is used. Original service-owner discovery still uses the existing native
---provider contract. Requires an active ingame mod with runtime.ecs.read.
---Inputs are {entityAId={id=uint32},entityBId={id=uint32}} using raw engine IDs.
---They are not opaque runtime.ecs handles. This compares IDs without resolving
---entities. branch is the direct comparison, before graph negate/control routing.
---Other operations return nil,reason until their own invocation contract exists.
---@param type TypeSelector Original reflected operation type or its Type object.
---@param inputs table Exact named values for the validated operation.
---@return {branch:boolean}? result
---@return string? reason
function runtime.impact.call(type, inputs) end

---@class RuntimeNetworkApi
runtime.network = {}
---@class RuntimeNetworkStatus
---@field available boolean Steam Networking Messages is initialized in this process and the mod has runtime access.
---@field role 'client'|'server' Whether this process is a player client or Dedicated Server.
---@field local_steam_id string? Decimal Steam ID of this process, when Steam exposes it.
---@field local_dedicated_server_steam_id string? Current Steam ID of a ShroudForge Dedicated Server running in this Windows session, if one publishes it.
---@field remote_dedicated_server_steam_id string? Dedicated Server SteamID64 learned from its automatic Network health probe.
---@field configured_server_steam_id string? Remote Dedicated Server fallback from Settings > Modules > Network.
---@field allowed_client_steam_ids string[]? Optional server-side client allowlist; empty means authenticated Enshrouded players.
---@field service_ready boolean Whether the built-in Steam session callback is registered for automatic session acceptance.
---@field reason string? Why the network transport is unavailable in this process.
--- Report whether this process can use the built-in Steam P2P message channel.
---@return RuntimeNetworkStatus status
function runtime.network.status() end
---@class RuntimeNetworkSendOptions
---@field channel integer? Destination channel, 0..65534; 65535 is reserved for the automatic Network health probe. Use the same channel on receive.
---@field reliable boolean? Reliable, ordered delivery on this peer/channel; default true.
--- Send raw bytes to a Steam peer. This is a separate Steam P2P session, not an Enshrouded game packet. True means Steam accepted the send request, not that the peer acknowledged it.
---@param peer_steam_id string Decimal SteamID64 from runtime.network.status().local_steam_id on the remote process.
---@param payload string Binary-safe Lua string, maximum 512 KiB.
---@param options RuntimeNetworkSendOptions?
---@return boolean ok
---@return string? reason
function runtime.network.send(peer_steam_id, payload, options) end
--- Send a UTF-8 text message from this mod to a named mod on a specific Steam peer. The destination mod receives it through receive_mod; this does not broadcast or identify the human player beyond their SteamID64.
---@param peer_steam_id string Decimal SteamID64 of the specific client or server process.
---@param target_mod_id string Mod ID installed on the receiving process.
---@param payload string UTF-8 text, maximum 512 KiB including protocol envelope.
---@param options RuntimeNetworkSendOptions? Only reliable is used; destination channel is derived from target_mod_id.
---@return boolean ok
---@return string? reason
function runtime.network.send_mod(peer_steam_id, target_mod_id, payload, options) end
--- Accept an incoming session request from a known peer. Runtime Network also accepts Steam session requests automatically for authenticated transport; this manual call remains for compatibility.
---@param peer_steam_id string Decimal SteamID64.
---@return boolean accepted
---@return string? reason
function runtime.network.accept(peer_steam_id) end
--- Return SteamID64 strings for clients Enshrouded has authenticated and that remain connected to this Dedicated Server, filtered by the Network module's optional allowlist.
---@return string[]|nil peers
---@return string|nil reason
function runtime.network.connected_peers() end
---@class RuntimeNetworkReceiveOptions
---@field channel integer? Local channel to read, default 0.
---@field limit integer? Maximum messages to return in one call, 1..32, default 16.
---@class RuntimeNetworkMessage
---@field peer_steam_id string SteamID64 of the remote process that sent this message.
---@field payload string Binary-safe Lua string.
---@field reliable boolean Whether Steam used reliable delivery for this message.
--- Read currently queued messages from one channel. Call from the mod's runtime update callback.
---@param options RuntimeNetworkReceiveOptions?
---@return RuntimeNetworkMessage[]? messages
---@return string? reason
function runtime.network.receive(options) end
--- Receive messages addressed to this mod. Call from runtime on_update; each message includes the sender mod ID and Steam peer ID. Runtime Network accepts the Steam transport session automatically; mods should still validate their own protocol and permissions.
---@param limit integer? Maximum messages, 1..32, default 16.
---@return RuntimeModNetworkMessage[]? messages
---@return string? reason
function runtime.network.receive_mod(limit) end
---@class RuntimeModNetworkMessage
---@field peer_steam_id string SteamID64 of the specific remote process.
---@field from_mod string Sender mod ID claimed in the protocol envelope.
---@field to_mod string This receiving mod ID.
---@field payload string UTF-8 text payload.
---@field reliable boolean Whether Steam used reliable delivery for this message.

--- @class RuntimeValuesApi
runtime.values = {}
--- Decode an owned KFC byte string. This does not dereference engine pointers.
---@param type TypeSelector
---@param bytes string
---@return any? value
---@return string? reason
function runtime.values.decode(type, bytes) end
--- Validate and serialize a Lua value using the current type layout.
---@param type TypeSelector
---@param value any
---@return string? bytes
---@return string? reason
function runtime.values.encode(type, value) end

---Normalize an inline scalar through the exact reflected field codec used by runtime writers.
---This only uses owned bytes. Dynamic/reference fields and non-finite values are rejected.
---@param type TypeSelector Reflected root layout.
---@param path string Inline field path, including one-based static indices.
---@param value unknown
---@return unknown? canonical
---@return string? reason
function runtime.values.normalize_field(type, path, value) end
--- Decode the exact reflected default. Never invent zero defaults when none are present.
--- DS containers requiring engine allocation return nil with an explicit reason.
---@param type TypeSelector
---@return any? value
---@return string? reason
function runtime.values.new(type) end

runtime.functions = {}
---@class RuntimeNativeBinding
---@field id string native:<decimal RVA>
---@field name string Original engine descriptor name when unique; otherwise unclear_<hex RVA>.
---@field name_provisional boolean False only for an original engine name.
---@field engine_descriptors RuntimeEngineDescriptor[] Original names and parser-resolved dependencies from the executable.
---@field modifiers table[] Checked interventions attached to their owning native function.
---@field operations table[] Existing checked runtime wrappers attached to the same function.
---@field key string Executable SHA256/name; rejects use with another executable.
---@field rva integer Relative virtual address of this candidate in the selected executable.
---@field provisional boolean True when the candidate name or binding has not been fully verified.
---@field callable boolean True only when this descriptor has a verified owned-buffer adapter.
---@field native_callable boolean Raw engine calls are not exposed by provisional bindings.
---@field execution string? Adapter implementation, when available.
---@field owners table[] Registration names/hashes, slot offsets and pointer origins.
---@field signature table Known adapter contract plus explicit unresolved native contract.
---@field validation table Separate address, type ownership, signature, context and effect evidence.
---@field buffer_transform table? Minimum source/destination lengths, exact copies, optional constant result.
---@field reason string? Why the candidate cannot be called when it is unresolved.
---@field call fun(source_bytes:string,destination_bytes:string):string?,string?,integer? Runs the verified adapter on owned bytes. Unknown bindings return nil and a reason.

---@class RuntimeEngineDescriptor
---@field name string Original engine system name.
---@field descriptor_rva integer Descriptor address relative to the loaded executable.
---@field dependency_slots RuntimeFunctionDependencySlot[] Original descriptor offsets and dependency names.
---@field dependency_resolution table Counts for reflected types and validated live registrations. Native signatures and access modes are reported separately.
---@class RuntimeFunctionDependencySlot
---@field offset integer Original byte offset. All six dependency spans are preserved.
---@field entries string[] Original dependency names.
---@field resolved_entries RuntimeFunctionDependency[] Parser identities joined to current runtime/template registration roles.
---@field access_mode 'read'|'write'|'configuration-read'|nil Only present with scheduler evidence for the exact executable.
---@field storage_role 'runtime'|'template'|nil Original registry namespace used by the system builder.
---@field system_record_mask_offset integer? Original system record mask offset, when proven.
---@field access_evidence string? Exact-image original scheduler investigation.
---@field optionality nil Optional versus required dependencies remain unproven.
---@class RuntimeFunctionDependency
---@field name string Original dependency name.
---@field qualified_name string Exact parser type name.
---@field type Type? The canonical reflected Type object, including fields, inheritance, alignment and defaults.
---@field reflection_resolved boolean Exact parser identity and inheritance are valid.
---@field registration_resolved boolean Exactly one live registration matches the parser names, hashes and sizes.
---@field registrations RuntimeFunctionRegistration[] All live associations. An ambiguous or invalid relation remains inspectable.
---@field reason string? Why reflection or live registration is unresolved.
---@class RuntimeFunctionRegistration
---@field index integer Live component registration index.
---@field qualified_name string Original registration name, which can differ from its runtime storage type.
---@field roles ('registration'|'runtime'|'template')[] Roles of this dependency type in the registration.
---@field runtime_type Type? Actual entity storage layout, if present and validated.
---@field template_type Type? Configuration layout, if present and validated.
---@field runtime_size integer? Entity storage size.
---@field validated boolean Parser identities, hashes and sizes agree with the live registration.
---@field reason string? Validation failure for this registration.

--- Pages of all observed native code candidates: unwind groups, image code pointers,
--- and live registration callbacks, including provisional/uncallable entries.
--- Code pointers can target internal labels. Unreferenced leaf/inlined code may be absent.
---@param offset integer? Zero-based, default 0.
---@param limit integer? 1..4096, default 256.
---@return table? page Descriptors contain evidence; use get() to obtain a binding with call().
---@return string? reason
function runtime.functions.list_native(offset, limit) end
--- Alias of list_native.
---@param offset integer?
---@param limit integer?
---@return table? page
---@return string? reason
function runtime.functions.list(offset, limit) end
--- Search the same complete inventory by an original function-name substring and/or reflected component.
--- Component search includes registration names, runtime/template roles, system dependencies and owned callbacks.
--- The returned page includes global dependency_resolution counts and the filtered count.
---@param options {name:string?,component:TypeSelector?}?
---@param offset integer? Zero-based filtered offset, default 0.
---@param limit integer? 1..4096, default 256.
---@return table? page
---@return string? reason
function runtime.functions.find(options, offset, limit) end
--- Finds a native binding by original name, checked operation ID, RVA, or explicit binding key.
--- Unresolved candidates are returned with their reason so they can be inspected without calling them.
---@param selector integer|string Original engine name, checked operation ID, RVA, native:<decimal>, unclear_<hex>, or SHA256/name.
---@return RuntimeNativeBinding? binding Includes unresolved bindings and their explicit failure reason.
---@return string? reason
function runtime.functions.get(selector) end
--- Native descriptor/owned-buffer adapter only; does not attach typed world-operation wrappers.
---@param rva integer|string
---@return RuntimeNativeBinding? binding
---@return string? reason
function runtime.functions.get_native(rva) end
--- Return a checked operation wrapper or a proven owned-buffer adapter.
--- Use get() to inspect incomplete bindings; bind() returns nil,reason for those.
---@param operation string Operation ID or provisional binding name/key.
---@return function? binding
---@return string? reason
function runtime.functions.bind(operation) end
--- Lists the checked operations and the current availability of each one.
---@return table<string, RuntimeFeatureStatus>
function runtime.functions.get_operations() end

---@class RuntimeFunctionModifier
---@field id string ShroudForge intervention ID, not an original engine name.
---@field backend_kind 'guarded-profile-patch' Existing exact-build profile patch backend.
---@field execution 'guarded-profile-patch' Execution does not become a generic native function call.
---@field profile_required boolean True. A verified exact-build patch and unique owner association are required.
---@field native_callable boolean False. Modifier binding never grants a raw engine call.
---@field owner RuntimeNativeBinding Exact owning function, original name when evidenced.
---@field effect string Instruction-level effect and scope.
---@field scope string Shared helpers affect all callers of the code site.
---@field callers table[] Static call paths to named engine systems; not a complete dynamic call graph.
---@field available fun():boolean Rechecks the existing capability and native byte guards.
---@field set_enabled fun(enabled:boolean):boolean,string? Same checked backend as legacy runtime.patch.
--- Resolve an existing guarded profile patch through its verified owning function and current KFC attribute definitions.
---@param id string
---@return RuntimeFunctionModifier? modifier
---@return string? reason
function runtime.functions.bind_modifier(id) end

--- Known features: `game.assets.write`, `export`, `runtime.lifecycle`, ECS operations,
--- and build-verified `runtime.world.*` operations.
--- @param feature string
--- @return boolean
function runtime.has(feature) end

--- Raises a Lua error when the named feature is not available to this mod in the current process.
--- Check runtime.has(feature) first when an unavailable feature should be handled without an error.
--- @param feature string Feature name such as runtime.lifecycle or runtime.ecs.read.
function runtime.require(feature) end

--- @class RuntimeFeatureStatus
--- @field available boolean True when the requested operation can run in this process now.
--- @field reason string? Explanation when the operation is unavailable.

--- Returns the availability and, if unavailable, the reason for a runtime operation.
--- @param feature string
--- @return RuntimeFeatureStatus
function runtime.status(feature) end

--- Enumerate declared runtime operations and current phase, mod and provider availability.
--- Unknown native candidates in development catalogs are not executable operations.
--- @return table<string, RuntimeFeatureStatus>
function runtime.get_operations() end

--- @class RuntimeEcsApi
runtime.ecs = {}

---@class RuntimeComponentRegistration
---@field index integer Engine registration index, distinct from Type.index.
---@field qualified_name string Original engine registration name.
---@field runtime_type Type? Actual entity storage layout, possibly Dynamic*.
---@field template_type Type? Configuration layout, separate from entity bytes.
---@field runtime_size integer Zero for template-only registrations.
---@field storage 'entity'|'template-only' Whether this registration owns entity data or only a configuration template.
---@field flags_bits integer Raw engine flags, semantics not assumed.
---@field storage_flags_bits integer Raw packed flags adjacent to the 16-bit storage size.
---@field callbacks table[] Code ownership evidence with origin/slot_offset/function_rva; resolve via runtime.functions.get(callback.function_rva).
---@field read_available boolean True when the validated layout can be read by the current adapter.
---@field write_available boolean True when the validated layout can be written by the current adapter.
---@field read_fields_available boolean Entity reads are ready. Each selected inline field is validated separately, including in a partial component.
---@field write_fields_available boolean This component is allowed by the current native write policy. Does not grant access to dynamic or reference descriptors.
---@field write_fields_reason string? Why the current component write policy is unavailable.
---@field partial_value boolean Some nested values require an unsupported native container codec.
---@field value_reason string? Why a complete component value cannot be decoded.
---@field reason string? Why this registration or operation is unavailable.

--- All registrations from the validated live engine registry, including template-only entries.
--- Does not require a per-build component hash/index profile. Engine hook/layout compatibility is still required.
---@return {schema_version:integer,layout_version:integer,source:string,count:integer,runtime_type_count:integer,entries:RuntimeComponentRegistration[]}? registry
---@return string? reason
function runtime.ecs.get_registry() end

--- Resolve an original registration or its runtime storage type by qualified name or Type.
---@param selector TypeSelector
---@return RuntimeComponentRegistration? component
---@return string? reason
function runtime.ecs.get_component(selector) end

---@class RuntimeAttribute
---@field name string Original KFC debugNames entry.
---@field hash integer Stored engine attribute ID; not a type hash or FNV(name).
---@field root_hash integer Hash identifying the root attribute calculation.
---@field index integer Zero-based element index within the root's storage.
---@field scalar_type string? KFC calculation scalar type.
---@field components table[] Definition GUID, reflected storage scalar type, size and offsets.
---@field structure table Original parent/child/sibling links and calculation words.
---@field calculation_words integer[] Original program words.
---@field calculation table Decoded AttributeOps instructions, attribute references and Push literals.
---@field storage_writable boolean Static layout agreement, independent of current entity/permissions.
---@field storage_reason string? Missing ownership or contradictory layout evidence.
---@field calculation_scalar_type string Effective model scalar; unresolved paths retain the KFC declaration.
---@field calculation_writable boolean Root program has a consistent or sign-independent scalar interpretation.
---@field calculation_reason string? Why a root update is blocked.
---@field write_value_domain string Scalar range restriction, including common integer range for signedness conflicts.
--- Lists reflected attributes and their storage or calculation metadata for the current game build.
--- This describes the available layout and does not mean every value can be read or written.
---@return {version:string,count:integer,entries:RuntimeAttribute[]}? attributes
---@return string? reason
function runtime.ecs.get_attributes() end
--- Finds one reflected attribute by its original name or stored engine ID.
---@param selector string|integer Original attribute name or stored ID.
---@return RuntimeAttribute? attribute
---@return string? reason
function runtime.ecs.get_attribute(selector) end
--- Reads one numeric attribute value from an entity using its reflected storage type.
---@param entity integer Entity handle from runtime.ecs.query.
---@param selector string|integer Original attribute name or stored engine ID.
---@return number? value Interpreted according to the reflected storage scalar type.
---@return string? reason
function runtime.ecs.read_attribute(entity, selector) end
--- Updates one checked attribute input and recalculates its complete initialized root.
--- Uses the same calculation and complete-snapshot CAS rules as write and write_fields for AttributeComponent subtypes.
--- Header, root identity and storage descriptors are read-only. Requires an exact validated calculation profile.
--- Conflicting integer signedness permits only the common nonnegative range; incompatible scalar representations reject writes.
---@param entity integer
---@param selector string|integer
---@param value number
---@return boolean ok
---@return string? reason
function runtime.ecs.write_attribute_storage(entity, selector, value) end

---Read the entire related attribute root as {entity, root_hash, component, scalar_type, values={original_name=value}}.
---@param entity integer
---@param selector string|integer Original attribute name or stored attribute ID.
---@return table? result
---@return string? reason
function runtime.ecs.read_attributes(entity, selector) end

---Evaluate a detached root in native descending order. values must contain every original name in the root.
---Returns named values and a per-program trace with before_bits/after_bits in root index order.
---Does not write game state. Unsupported programs and non-finite values are rejected.
---@param selector string|integer
---@param values table<string, number>
---@return table? result
---@return string? reason
function runtime.ecs.evaluate_attributes(selector, values) end

---Set one attribute and recalculate its complete root. Requires an exact verified executable profile.
---The guarded write rejects a changed snapshot and checks the entity generation and component layout.
---Applies locally; the engine owns replication. Does not forward client calls to the server.
---Signedness conflicts permit only common nonnegative input values and type-independent calculations.
---Optional expected_values checks named values in this same root against the fresh snapshot before recalculation.
---A changed expected value rejects the update. The complete native component CAS still applies afterward.
---@param entity integer
---@param selector string|integer
---@param value number
---@param expected_values table<string, number>? Expected original attribute names from the selected root only.
---@return table? result Updated values and calculation trace, or nil on failure.
---@return string? reason
function runtime.ecs.update_attribute(entity, selector, value, expected_values) end

--- Return every type that is registered as a component in the live Keen ECS.
--- The returned Type objects expose their exact names, sizes and fields through `game.types`.
--- Reflection-only ECS helpers, enums and resources are intentionally not included.
--- @return Type[]? components
--- @return string? reason
function runtime.ecs.get_components() end

---@class RuntimeEcsTypeRegistrationRole
---@field role 'runtime'|'template' Role of this exact reflected type in the engine registration.
---@field index integer Engine registration index, distinct from the reflection type index.
---@field qualified_name string Original engine registration name.
---@class RuntimeEcsCatalogEntry
---@field type Type Canonical reflected type from the current parser registry.
---@field qualified_name string Exact reflected type name.
---@field size integer Reflected storage size in bytes.
---@field registered boolean This exact type occurs as runtime or template layout in the validated registry.
---@field runtime_resolved boolean The provider resolves a matching entity storage layout, independent of entity access readiness.
---@field resolved boolean A runtime mapping or matching template registration is validated. Does not imply a live entity exists.
---@field registration_state 'registered'|'reflected-only' Reports observed engine registration separately from reflection.
---@field storage 'entity'|'template-only'|'unregistered' Observed engine registry role. A provider fallback can resolve storage before the registry is available.
---@field registrations RuntimeEcsTypeRegistrationRole[] Observed runtime/template relations for this reflected type.
---@field read_available boolean A component snapshot can currently be read. Nested value decoding can still be partial.
---@field write_available boolean Whole-value writes are ready, including the complete owned-value codec.
---@field read_fields_available boolean Entity reads are ready. Selected field paths still require inline-layout validation.
---@field write_fields_available boolean This component is allowed by the current native write policy, independently of complete-component codec support.
---@field write_fields_reason string? Mod/API denial or component-specific write policy limitation.
---@field partial_value boolean The complete value has an unsupported nested codec.
---@field value_reason string? Why the complete owned-value codec is unavailable.
---@field read_reason string? Mod lifecycle, capability or API denial when present.
---@field write_reason string? Mod/API denial, unsupported complete-value codec or component-specific write policy limitation.
---@field reason string? Missing registration or incompatible reflected layout.
---@class RuntimeEcsCatalog
---@field version string Current parser registry version.
---@field registry_available boolean The provider exposes a validated engine registry.
---@field registered_type_count integer Number of reflected keen::ecs::* types with an observed runtime or template relation.
---@field runtime_type_count integer Number with an observed runtime role and matching size.
---@field template_type_count integer Number with an observed template role and matching size. Roles can overlap.
---@field resolved_count integer Number with a validated runtime mapping or matching template registration.
---@field reflected_only_count integer Number without an observed engine registration relation.
---@field entity_access_ready boolean Validated current entity reads are ready, independently of the current mod's permissions. The KFC4 original-owner backend does not require a dispatcher hook.
---@field entity_writes_ready boolean Native guarded writes are ready, independently of the complete-component codec and mod permissions.
---@field entries RuntimeEcsCatalogEntry[] All reflected keen::ecs::* types, including unregistered helpers and template-only layouts.
--- List reflected ECS types and their actual runtime/template registration roles.
--- Mapping identity is independent of the current entity phase and dispatcher readiness.
--- Read/write availability still requires the active mod lifecycle, capabilities and entity provider.
---@return RuntimeEcsCatalog catalog
function runtime.ecs.get_catalog() end

--- Query live entities by real `keen::ecs::*` component type names or Type objects.
--- Returns `nil, reason` while unavailable or the first incremental scan is pending. Retry from a later update.
--- Later calls revalidate the last complete snapshot while advancing discovery. Deleted entities, changed generations and missing components are filtered on every call. Newly matching entities appear after the refresh completes. Every read/write independently validates its handle again.
--- @param ... string|Type Qualified type name or Type from the current registry.
--- @return integer[]? entities Opaque, generation-checked ShroudForge entity handles.
--- @return string? reason
function runtime.ecs.query(...) end
--- Query live entities whose CurrentTransform pivots overlap bounds expanded by padding * max(abs(scale)).
function runtime.ecs.query_bounds(bounds, padding, ...) end

--- Resolve a real `keen::EntityId.id` obtained from a reflected component to
--- the current generation-checked ShroudForge handle. The id is looked up in
--- the live entity manager; it is never treated as a pointer or cached handle.
--- @param keen_entity_id integer
--- @return integer? entity
--- @return string? reason
function runtime.ecs.resolve(keen_entity_id) end

--- Read one live ECS component by an opaque entity handle and real `keen::ecs::*` type.
--- Returns `nil, reason` until the current game build has a verified ECS provider.
--- @param entity integer
--- @param component string|Type Qualified type name or Type from the current registry.
--- @return table? value
--- @return string? reason
function runtime.ecs.read(entity, component) end
--- Read an owned snapshot of the complete entity storage bytes, including opaque DS descriptors.
--- Does not follow pointers. The same runtime.ecs.read capability and entity checks apply.
---@param entity integer
---@param component TypeSelector
---@return string? bytes
---@return string? reason
function runtime.ecs.read_bytes(entity, component) end

--- Read selected inline fields from one owned component snapshot, even when a sibling uses an unsupported container.
--- Paths use reflected names, dot-separated nesting and one-based static indices, such as `dataStorage[1]`.
--- Supports scalar/enum/Guid values and inline structs/static arrays containing only supported leaves.
--- Does not follow or decode DS, Blob or ObjectReference descriptors. Enclosing descriptor overlaps are rejected.
--- Requires runtime.ecs.read, an active ingame mod and a current generation-checked entity handle.
---@param entity integer
---@param component TypeSelector Original registration or its actual runtime storage type.
---@param field_paths string[] Nonempty dense array of distinct field paths, at most 1024 paths.
---@return table<string, unknown>? values Owned field values keyed by the exact requested path.
---@return string? reason
function runtime.ecs.read_fields(entity, component, field_paths) end

--- Resolve a current entity's original template UUID to its loaded TemplateResource.
--- Requires runtime.ecs.read and a separately validated definition UUID layout.
--- Returns the same ResourceService snapshot type as game.assets.get_loaded_resource.
--- This identifies the template owner. It does not prove that every runtime field still aliases its configuration.
---@param entity integer Current generation-checked entity handle.
---@return Resource? template
---@return string? reason
function runtime.ecs.get_template(entity) end

---@class RuntimeTemplateComponent
---@field key string Stable resource GUID/type/part plus concrete component name, for deduplication within session_id.
---@field type Type Concrete reflected configuration type. Distinct from the runtime storage type.
---@field scope 'shared-template' Every entity using this template shares the configuration change.
---@field session_id integer Original nonzero world session. Views never authorize another session.
local RuntimeTemplateComponent = {}

---Read fresh selected aligned 32-bit scalars from this exact shared template configuration.
---Paths are relative to the concrete component, such as fallDamageStartDistance.
---Requires an active mod with runtime.ecs.read. Missing or duplicate concrete variants return a reason.
---@param paths string[] Nonempty dense array of distinct reflected field paths, at most 1024.
---@return table<string, number|string>? values Values keyed by the exact selected paths.
---@return string? reason
function RuntimeTemplateComponent:read_fields(paths) end

---Compare and write one aligned 32-bit reflected scalar, with independent complete native readback under the original gates.
---Requires runtime.assets.write and the runtime.ecs.write authority gate in the original world session.
---expected is required. A mismatch writes nothing. References, descriptors and non-finite floats are rejected.
---The native gate compares the complete fresh resource snapshot before the single-scalar mutation.
---A later unlocked observation or world change does not reverse a confirmed native success. Future operations recheck the session.
---Optional expected_fields checks other component-relative inputs in the same fresh snapshot before that native CAS.
---The resource binding permits guarded restoration after the original entity disappears in the same session.
---Does not publish events or establish that this engine consumer observes later configuration changes.
---An 'uncertain' result stops further writes through this view and must never be blindly retried.
---@param path string Reflected component-relative field path.
---@param value number|string New scalar, or original enum name.
---@param expected number|string Expected current scalar, or original enum name.
---@param expected_fields table<string, number|string>? Nonempty map of at most 1024 component-relative expected scalars.
---@return boolean ok
---@return string? reason
function RuntimeTemplateComponent:write_field(path, value, expected, expected_fields) end

---Bind a current entity's original TemplateResource and one uniquely registered concrete configuration variant.
---Validates the live template role, entity UUID before/after lookup and the current world session.
---This is shared-template storage. It neither selects an entity runtime column nor allocates/replaces a template.
---@param entity integer Current generation-checked entity handle.
---@param component TypeSelector Original engine registration or its concrete template configuration type.
---@return RuntimeTemplateComponent? component_view
---@return string? reason
function runtime.ecs.get_template_component(entity, component) end

--- Validate and write selected inline field values. Complete-component codec support is not required.
--- Reads a fresh complete component snapshot inside this call, then compares it again in the validated engine execution phase before applying changes.
--- A snapshot change rejects the write. Optional expected_fields also checks selected values from an earlier read.
--- Only reflected value leaves are masked. Padding, siblings and external-storage descriptors are preserved.
--- Dynamic/reference fields, overlapping selections and non-finite floats are rejected.
--- Requires runtime.ecs.write, an active ingame mod, the native dispatcher and a current entity generation/layout.
--- AttributeComponent subtypes run their original complete root calculation and also mask any changed derived scalars. Their headers are read-only.
--- Does not publish engine events or establish replication and gameplay semantics.
---@param entity integer
---@param component TypeSelector Original registration or its actual runtime storage type.
---@param changes table<string, unknown> Nonempty field-path/value map, at most 1024 entries. Static indices are one-based.
---@param expected_fields table<string, unknown>? Nonempty field-path/value map checked before applying changes.
---@return boolean ok True when values already agree or the guarded write succeeds.
---@return string? reason
---@return table<string, unknown>? confirmed Selected fields from the confirmed result, including original attribute calculations. No later unlocked read is used.
function runtime.ecs.write_fields(entity, component, changes, expected_fields) end

--- Write changed reflected fields from a value previously returned by `read`.
--- Padding and unselected source fields are preserved. A changed complete source snapshot rejects the write.
--- AttributeComponent subtypes preserve their headers and run the original complete root calculation before CAS.
--- Writes are verified by readback. An unconfirmed rollback blocks further writes in the current native binding epoch.
--- Returns `false, reason` until the current game build has a verified ECS provider.
--- @param entity integer
--- @param component string|Type Qualified type name or Type from the current registry.
--- @param value table
--- @return boolean ok
--- @return string? reason
function runtime.ecs.write(entity, component, value) end

--- @class RuntimeWorldApi
runtime.world = {}
--- @class RuntimeWorldCursorApi
runtime.world.cursor = {}
--- Read the local building cursor. The original-owner backend resolves the unique
--- ClientPlayerInput in the main-client ECS manager and decodes data.cursorInput
--- using the active reflection, without hooks or engine calls. Its bounded initial
--- discovery can return nil,reason while scanning. Retry on a later update.
--- sequence counts successful samples, not simulation ticks. Position keeps Q32.32
--- integers and flags retain their reflected width. Legacy execution profiles keep
--- their verified hook snapshot. A cursor read never grants voxel write authority.
--- The reflected snapshot also holds `value.camera`, the camera the local client
--- sends with its input (data.camera): `position` (Q32.32 integers), `orientation`
--- ({x, y, z, w}), `fovY`, `aspectRatio`, `nearPlane`, `farPlane`, `distance`;
--- absent from legacy snapshots and when the layout lacks these fields.
--- @return table? snapshot with `sequence` and reflected-layout `value.primaryTransform` fields.
--- @return string? reason
function runtime.world.cursor.get() end

--- Ordinary local player building input; availability: runtime.world.building.input.
--- Exact client build only; follows the existing game input path. The server
--- still checks permissions/resources/range. No server mod RPC is created.
runtime.world.building = {}
--- @class BuildingInput
--- @field player integer Process-local ClientPlayerInput entity handle.
--- @field action 'select'|'place'|'remove'|'dismantle'|'undo' `remove` is the secondary building action for voxels; `dismantle` holds the contextual action for props.
--- @field targetEntityId integer? Exact live ECS entity ID required for `dismantle`.
--- @field itemId integer? Required for select; current KFC ItemInfo item ID.
--- @field materialItemId integer? Optional stock cycle material item ID.
--- @field slot integer? Current actionbar slot for select.
--- @field position number[]? Required for place/remove/dismantle, world units.
--- @field rotation number[]? Quaternion x,y,z,w, normalized by the provider.
--- @field scale number[]? Positive x,y,z, defaults to 1,1,1.
--- @param input BuildingInput
--- The first accepted request reserves the shared input sequence for this Lua
--- API instance. Call cancel(latest_id) when the whole sequence ends, including
--- after successful observation, so another mod can use the building tool.
--- @return integer? request_id
--- @return string? reason
function runtime.world.building.submit(input) end
--- Only the latest request owned by this Lua API instance can be queried.
--- 'dispatched' means input emitted, NOT accepted/replicated/persisted.
--- @param request_id integer
--- @return 'unknown'|'queued'|'pressed'|'dispatched'|'timeout'|'cancelled'
function runtime.world.building.status(request_id) end
--- Stops pending input. A pressed action is released on the next input tick;
--- already dispatched changes cannot be undone by cancelling.
--- @param request_id integer
--- @return boolean
function runtime.world.building.cancel(request_id) end

--- @class RuntimeWorldVoxelApi
runtime.world.voxel = {}

--- Return metadata for a native world grid. The active backend currently exposes the `voxel` grid.
--- The registered reader returns nil,'voxel-grid-pending' while its Scene gate
--- or original metadata is temporarily unavailable. Retry on a later update.
--- A changed world session returns nil,'world-session-changed'.
--- @param grid_id string Grid identifier, currently "voxel".
--- @return table? spec {id, origin, cellSize, maximum}
--- @return string? reason
function runtime.world.voxel.get_grid_spec(grid_id) end

--- Read a bounded, x-fastest voxel region. Each number packs material in the low byte and density in the high byte.
--- Availability depends on the installed backend. A reader does not establish
--- voxel.write, collision, replication or persistence support.
--- A session-bound provider returns nil,'voxel-read-pending' while
--- an original cache request or the Scene gate is pending. Retry on a later update.
--- Failed or partial reads never return a successful cell array.
--- @param x integer
--- @param y integer
--- @param z integer
--- @param size_x integer
--- @param size_y integer
--- @param size_z integer
--- @return integer[]? cells
--- @return string? reason
function runtime.world.voxel.read(x, y, z, size_x, size_y, size_z) end

--- Write a bounded, x-fastest voxel region and verify the native readback. Each cell is a packed 16-bit material/density value.
--- @param x integer
--- @param y integer
--- @param z integer
--- @param size_x integer
--- @param size_y integer
--- @param size_z integer
--- @param cells integer[]
--- @return boolean ok
--- @return string? reason
function runtime.world.voxel.write(x, y, z, size_x, size_y, size_z, cells) end

--- Report whether the native provider currently supports this named world operation.
--- @param operation string
--- @return boolean available
function runtime.world.operation_available(operation) end

--- Report whether a validated live voxel-world context is available.
--- @return boolean active
--- @return string? reason
function runtime.world.context_active() end
--- Return the validated native target for automatic editor routing.
--- direct identifies a local authority ECS owner. Individual write operations
--- still require their own availability and permission checks. client-read-only
--- identifies a remote world replica whose mutations must use the host.
--- unknown means no context has been validated yet.
--- @return 'direct'|'client-read-only'|'unknown' kind
function runtime.world.context_kind() end
--- Conservative process-local world/ECS lifetime marker; zero when unavailable.
--- Not a persistent world ID or a singleplayer/multiplayer indicator.
---@return integer
function runtime.world.session_id() end

--- @class RuntimeWorldEntityApi
runtime.world.entity = {}

--- Query placeable props using the validated ECS owner and dynamically registered
--- CurrentTransform/UsedItem storage. Returns transforms and item IDs without Lua
--- reflection reads. The original-owner read path does not install game-code hooks.
--- @param bounds number[] World-space min xyz followed by max xyz.
--- @param padding number Conservative pivot margin, scaled by each prop's maximum absolute scale.
--- @return table[]? props
--- @return string? reason
function runtime.world.entity.query_props(bounds, padding) end

--- Register ItemInfo placement AABBs for native prop-bound filtering.
--- @param recipes table[] Entries with itemId, bounds[6], and feedback.
--- @return boolean? registered
--- @return string? reason
function runtime.world.entity.register_prop_recipes(recipes) end

--- Query props whose native, rotated and scaled ItemInfo placement AABBs intersect bounds.
--- Each result includes entityId and templateUuidHighHex/templateUuidLowHex from the live native entity definition.
--- Requires placement recipes registered with register_prop_recipes.
--- Native scans and cached-result validation are incremental. Retry nil,reason
--- containing "still scanning" from a later update. No partial result is published.
--- @param bounds number[] World-space min xyz followed by max xyz.
--- @return table[]? props
--- @return string? reason
function runtime.world.entity.query_props_in_bounds(bounds) end

--- Resolve the current transform, ECS entity ID, item ID, and native template UUID for an opaque live prop handle.
--- A nil prop with reason "entity-absent" confirms that the issued handle's
--- identity is absent in the same live session, using two original membership
--- lookups. Other nil results include stale sessions and unreadable/changing
--- data and do not prove deletion. Older bool-only providers cannot confirm absence.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @return table? prop
--- @return string? reason
function runtime.world.entity.get_transform(entity_handle) end

--- Update a live prop's three scale values through the canonical full-component
--- ECS snapshot/CAS writer. Original ownership, Scene gates and tracking policy
--- apply. Tracked components without a supported native phase remain unavailable.
--- Readback verifies data only, not collision, replication or save persistence.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @param scale number[] Three finite scale values for x, y, and z.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.set_scale(entity_handle, scale) end

--- Spawn a native entity in the live prop-update context and wait for its live ECS record.
--- UUIDs are hexadecimal qwords. Returns the new opaque entity handle, not an engine queue token.
--- @param template_uuid_high_hex string
--- @param template_uuid_low_hex string
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param tracking_id integer
--- @param flags integer
--- @return integer? entity_handle
--- @return string? reason
function runtime.world.entity.spawn(template_uuid_high_hex, template_uuid_low_hex, position, rotation, tracking_id, flags) end

--- Dispatch an engine placement and finish-building call in a validated live actor placement context.
--- A successful return confirms dispatch without independently proving entity materialization, persistence, or collision updates.
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param bounds number[] AABB min xyz followed by max xyz.
--- @param tracking_id integer
--- @param feedback_id integer
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.place(position, rotation, bounds, tracking_id, feedback_id) end

--- Remove the prop identified by its current live handle. The native operation resolves its
--- transform and registered ItemInfo placement recipe, dispatches removal, and verifies that
--- this exact handle disappeared, matching Shroudtopia's WorldApi handle-only contract.
--- @param entity_handle integer Handle returned by query_props or spawn.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.destroy(entity_handle) end

--- Legacy spatial destroy overload. Prefer the handle overload for editor operations.
--- This operation does not establish save persistence or collision updates.
--- @param position number[] Three world coordinates.
--- @param rotation number[] Non-zero quaternion x,y,z,w; KFC Runtime normalizes it before dispatch.
--- @param bounds number[] AABB min xyz followed by max xyz.
--- @param tracking_id integer
--- @param feedback_id integer Ignored by the native destroy call; retained for the shared place/destroy signature.
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.destroy(position, rotation, bounds, tracking_id, feedback_id) end

--- Dispatch the profile's finish-building operation in a validated live actor placement context.
--- @param complete boolean
--- @return boolean ok
--- @return string? reason
function runtime.world.entity.finish_building(complete) end

--- @class RuntimePatchApi
runtime.patch = {}
--- Report whether the active build profile validated a named runtime patch.
--- @param name string Profile operation name such as runtime.patch.no_fall_damage.
--- @return boolean available
function runtime.patch.available(name) end
--- Safely enable or restore a profile-defined runtime patch.
--- @param name string Profile operation name such as runtime.patch.no_fall_damage.
--- @param enabled boolean
--- @return boolean ok
--- @return string? reason
function runtime.patch.set_enabled(name, enabled) end

--- @class ShroudForgeLogApi
local log = {}
--- Writes a detailed TRACE-level message to this mod's log.
--- @param ... unknown
function log.trace(...) end
--- Writes a DEBUG-level diagnostic message to this mod's log.
--- @param ... unknown
function log.debug(...) end
--- Writes an INFO-level message about a meaningful mod event.
--- @param ... unknown
function log.info(...) end
--- Writes a WARN-level message about a blocked or uncertain mod operation.
--- @param ... unknown
function log.warn(...) end
--- Writes an ERROR-level message about a failed mod operation.
--- @param ... unknown
function log.error(...) end

--- @class ShroudForgeApi
--- @field version string Version of the ShroudForge API available to this mod.
--- @field mod_id string Stable ID of the currently running mod.
--- @field mod_kind "lua" Runtime package kind for the currently running mod.
--- @field log ShroudForgeLogApi Write messages to this mod's shared ShroudForge log.
shroudforge = {}
