#include "engine-context/engine_context_state.h"

using namespace EngineContextImplementation;

namespace EngineContext {
bool Configure(const nlohmann::json& profile, std::string& error) {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    try {
        const auto& version_value = profile.at("schemaVersion");
        if (!version_value.is_number_integer() || (version_value != 1 && version_value != 2))
            throw std::runtime_error("unsupported engine owner profile format");
        const auto version = version_value.get<unsigned>();
        if (version == 1 && (profile.contains("entityCommandQueueLayout") || profile.contains("buildingPropSpawnLayout")))
            throw std::runtime_error("explicit queue and spawn layouts require profile format version 2");
        const auto& identity = profile.at("image");
        if (!image_size || image_sha256.empty() || profile.at("target") != image_target ||
            identity.at("sha256") != image_sha256 || identity.at("timestamp") != image_timestamp ||
            identity.at("size") != image_size)
            throw std::runtime_error("engine owner profile does not match the exact loaded image");
        const auto& owners = profile.at("ownerLayout");
        Layout layout;
        layout.image_size = image_size;
        layout.dedicated_server = image_target == "enshrouded_server.exe";
        auto offset = [](const nlohmann::json& object, const char* key, std::size_t maximum = 0x4000000) {
            const auto value = object.at(key).get<std::size_t>();
            if (value > maximum) throw std::runtime_error(std::string("engine owner offset out of range: ") + key);
            return value;
        };
        layout.root_rva = offset(owners, "rootObjectRva", image_size - 1);
        layout.root_vtable = offset(owners, "rootVtableRva", image_size - 1);
        layout.client_offset = offset(owners, "clientPointerOffset");
        layout.client_vtable = offset(owners, "clientVtableRva", image_size - 1);
        layout.controller_offset = offset(owners, "controllerPointerOffset");
        layout.controller_vtable = offset(owners, "controllerVtableRva", image_size - 1);
        layout.controller_back_pointer = offset(owners, "controllerClientBackPointerOffset");
        layout.main_owner_offset = offset(owners, "controllerMainOwnerOffset");
        nlohmann::json dependencies;
        if (profile.contains("systemDependencyLayout")) {
            if (!profile.at("provenance").value("systemDependenciesValidated", false))
                throw std::runtime_error("system dependencies lack original scheduler validation");
            const auto& model = profile.at("systemDependencyLayout");
            const auto& slots = model.at("slots");
            const auto source = model.at("source").get<std::string>();
            if (model.at("id") != "ecs-system-dependencies-v1" || source.empty() || !slots.is_array() || slots.size() != 6)
                throw std::runtime_error("invalid system dependency model");
            auto output = nlohmann::json::array();
            std::uint32_t offsets{};
            for (const auto& slot : slots) {
                const auto position = slot.at("offset").get<unsigned>();
                const auto role = slot.at("storageRole").get<std::string>();
                const auto access = slot.at("accessMode").get<std::string>();
                if (position < 24 || position > 104 || (position - 24) % 16 ||
                    (offsets & (1u << ((position - 24) / 16))) ||
                    (role != "runtime" && role != "template") ||
                    (role == "template" ? access != "configuration-read" : (access != "read" && access != "write")))
                    throw std::runtime_error("invalid or duplicate system dependency slot");
                offsets |= 1u << ((position - 24) / 16);
                output.push_back({{"offset",position},{"storage_role",role},{"access_mode",access},
                    {"system_record_mask_offset",offset(slot,"systemRecordMaskOffset",0x10000)}});
            }
            dependencies = {{"id",model.at("id")},{"source",source},{"validated",true},
                {"image",{{"target",image_target},{"sha256",image_sha256},{"timestamp",image_timestamp},{"size",image_size}}},
                {"record_builder_rva",offset(model,"recordBuilderRva",image_size - 1)},
                {"access_set_pack_rva",offset(model,"accessSetPackRva",image_size - 1)},
                {"compatibility_predicate_rva",offset(model,"compatibilityPredicateRva",image_size - 1)},
                {"optionality",nullptr},{"slots",std::move(output)}};
        }
        if (profile.contains("ecsReadLayout")) {
            if (!profile.at("provenance").value("ecsReadValidated", false))
                throw std::runtime_error("ECS owner layout lacks read validation");
            const auto& ecs = profile.at("ecsReadLayout");
            layout.execution_owner_offset = offset(ecs, "mainExecutionOwnerOffset");
            layout.registry_owner_offset = offset(ecs, "executionRegistryOwnerOffset");
            layout.entity_manager_offset = offset(ecs, "executionEntityManagerOffset");
            layout.manager_owner_offset = offset(ecs, "entityManagerOwnerOffset");
            auto& e = layout.entities;
            e.manager_count = offset(ecs, "entityManagerCount"); e.manager_table = offset(ecs, "entityManagerTable");
            if (ecs.contains("entityManagerOccupancy") != ecs.contains("entityManagerKeys"))
                throw std::runtime_error("ECS entity hash membership needs both occupancy and keys offsets");
            if (ecs.contains("entityManagerOccupancy")) {
                e.manager_occupancy = offset(ecs, "entityManagerOccupancy");
                e.manager_keys = offset(ecs, "entityManagerKeys");
                if (!e.manager_occupancy || !e.manager_keys)
                    throw std::runtime_error("ECS entity hash membership offsets are zero");
            }
            e.entity_id = offset(ecs, "entityId"); e.entity_generation = offset(ecs, "entityGeneration");
            e.entity_layout = offset(ecs, "entityLayout"); e.entity_storage = offset(ecs, "entityStorage");
            e.entity_definition = offset(ecs, "entityDefinitionPointer"); e.entity_row = offset(ecs, "entityRow");
            e.entity_definition_uuid_validated = ecs.contains("entityDefinitionUuidOffset");
            if (e.entity_definition_uuid_validated)
                e.entity_definition_uuid = offset(ecs, "entityDefinitionUuidOffset", 65536);
            e.component_bits = offset(ecs, "componentBits"); e.component_offsets = offset(ecs, "componentOffsets");
            e.component_strides = offset(ecs, "componentStrides");
            e.maximum_entities = offset(ecs, "maximumEntities", 1u << 20);
            if (!e.maximum_entities) throw std::runtime_error("engine entity limit is zero");
            if (ecs.contains("clientSceneOwnerOffset")) {
                if (!profile.at("provenance").value("ecsAuthorityReadValidated", false))
                    throw std::runtime_error("local authority owner layout lacks read validation");
                layout.client_scene_offset = offset(ecs, "clientSceneOwnerOffset");
                layout.scene_entries_offset = offset(ecs, "sceneEntriesPointerOffset");
                layout.scene_count_offset = offset(ecs, "sceneEntriesCountOffset");
                layout.scene_capacity_offset = offset(ecs, "sceneEntriesCapacityOffset");
                layout.entry_stride = offset(ecs, "sceneEntryStride", 0x10000);
                layout.entry_context_offset = offset(ecs, "sceneEntryContextOffset");
                layout.entry_uuid_offset = offset(ecs, "sceneEntryUuidOffset");
                layout.entry_state_offset = offset(ecs, "sceneEntryStateOffset");
                layout.entry_ready_state = static_cast<std::uint8_t>(offset(ecs, "sceneEntryReadyState", 255));
                layout.maximum_scenes = offset(ecs, "maximumSceneEntries", 4096);
                if (!layout.maximum_scenes || layout.entry_stride < 24 ||
                    layout.entry_context_offset > layout.entry_stride - 8 || layout.entry_uuid_offset > layout.entry_stride - 16 ||
                    layout.entry_state_offset >= layout.entry_stride) throw std::runtime_error("invalid active scene entry layout");
                layout.authority = true;
            }
            layout.ecs = true;
        }
        if (profile.contains("ecsWriteLayout")) {
            const auto& model = profile.at("ecsWriteLayout");
            const auto& provenance = profile.at("provenance");
            if (!layout.authority || model.at("id") != "ecs-native-thread-scene-gate-v1" ||
                !provenance.value("ecsWriteGateValidated", false))
                throw std::runtime_error("ECS write gate lacks original authority/lifetime validation");
            auto& w = layout.write;
            w.enabled = model.value("enabled", false);
            if (w.enabled && !provenance.value("ecsChangeTrackingValidated", false))
                throw std::runtime_error("ECS writes lack change tracking validation");
            if (w.enabled && !model.value("phaseValidated", false))
                throw std::runtime_error("ECS writes lack original command/epoch phase validation");
            if (model.value("phasePolicy", std::string{}) != "untracked-components")
                throw std::runtime_error("ECS write phase policy must be untracked-components");
            w.registry_lock = offset(model, "threadRegistryLockRva", image_size - sizeof(LONG));
            w.registry_entries = offset(model, "threadRegistryEntriesRva", image_size - 1);
            w.registry_count = offset(model, "threadRegistryCountRva", image_size - sizeof(std::uint16_t));
            w.callback = offset(model, "threadCallbackRva", image_size - 1);
            w.capacity = offset(model, "threadRegistryCapacity", 256);
            w.stride = offset(model, "threadRegistryEntryStride", 256);
            w.row_handle = offset(model, "registryEntryHandleOffset", 256);
            w.row_name = offset(model, "registryEntryNamePointerOffset", 256);
            w.row_length = offset(model, "registryEntryNameLengthOffset", 256);
            w.inline_name = offset(model, "threadInlineNameOffset", 65536);
            w.scene_thread = offset(model, "sceneThreadOffset", 65536);
            w.name_pointer = offset(model, "threadNamePointerOffset", 65536);
            w.name_length = offset(model, "threadNameLengthOffset", 65536);
            w.name_capacity = offset(model, "threadNameCapacityOffset", 65536);
            w.name_capacity_value = offset(model, "threadNameCapacity", w.name.size());
            w.thread_handle = offset(model, "threadHandleOffset", 65536);
            w.thread_context = offset(model, "threadContextOffset", 65536);
            w.thread_stop = offset(model, "threadStopOffset", 65536);
            w.thread_callback = offset(model, "threadCallbackOffset", 65536);
            w.scene_context = offset(model, "sceneContextOffset", 65536);
            w.scene_lock = offset(model, "sceneLockOffset", 65536);
            w.scene_state = offset(model, "sceneStateOffset", 65536);
            w.running_state = static_cast<std::uint8_t>(offset(model, "sceneRunningState", 255));
            w.execution_mode = offset(model, "executionModeOffset");
            w.authority_mode = static_cast<std::uint8_t>(offset(model, "executionAuthorityMode", 255));
            w.maximum_bytes = offset(model, "maximumWriteBytes", 65536);
            w.tracker = offset(model, "changeTrackerRva", image_size - w.tracker_bytes.size());
            w.tracking_context = offset(model, "changeTrackerContextOffset");
            w.tracking_manager = offset(model, "changeTrackerManagerOffset", 65536);
            w.tracking_queue_b = offset(model, "changeTrackerQueueBOffset", 65536);
            w.tracking_registry_a = offset(model, "changeTrackerRegistryAOffset", 65536);
            w.tracking_registry_b = offset(model, "changeTrackerRegistryBOffset", 65536);
            w.maximum_tracking_capacity = offset(model, "maximumTrackingQueueCapacity", 1u << 20);
            const auto tracker_bytes = model.at("changeTrackerBytes").get<std::string>();
            if (tracker_bytes.size() != w.tracker_bytes.size() * 2 || !w.tracker || !w.maximum_tracking_capacity ||
                !w.tracking_registry_a || !w.tracking_registry_b || w.tracking_registry_a == w.tracking_registry_b)
                throw std::runtime_error("invalid original ECS change tracker");
            auto nibble = [](char character) -> unsigned {
                if (character >= '0' && character <= '9') return character - '0';
                if (character >= 'a' && character <= 'f') return character - 'a' + 10;
                throw std::runtime_error("invalid change tracker guard bytes");
            };
            for (std::size_t index = 0; index < w.tracker_bytes.size(); ++index)
                w.tracker_bytes[index] = static_cast<std::uint8_t>((nibble(tracker_bytes[index * 2]) << 4) |
                    nibble(tracker_bytes[index * 2 + 1]));
            const auto name = model.at("threadName").get<std::string>();
            if (!w.capacity || w.stride < 24 || !w.maximum_bytes || !w.registry_lock || w.registry_lock % alignof(LONG) ||
                w.capacity * w.stride > image_size || w.registry_entries > image_size - w.capacity * w.stride ||
                w.row_handle > w.stride - 8 || w.row_name > w.stride - 8 || w.row_length > w.stride - 8 ||
                name.empty() || name.size() > w.name_capacity_value || name.find('\0') != std::string::npos ||
                w.name_capacity_value > w.name.size() || !w.inline_name)
                throw std::runtime_error("invalid native-thread ECS write gate layout");
            std::copy(name.begin(), name.end(), w.name.begin());
            w.name_size = name.size();
        }
        if (version == 2 && layout.authority && layout.write.enabled &&
            (!profile.contains("entityCommandQueueLayout") || !profile.contains("buildingPropSpawnLayout")))
            throw std::runtime_error("version 2 profile requires explicit queue and spawn layouts");
        if (version == 1 && layout.authority && layout.write.enabled && profile.value("kfcFormat", 0) == 4) {
            // Preserve the original V1 data ABI without rewriting historical profiles.
            layout.entity_command_queue_layout = EntityCommandQueueLayout{
                .task_state=0x538, .task_records=0x528, .task_count=0x530,
                .queue_records=0xeb9f08, .queue_count=0xeb9f10, .task_record_stride=0xe0,
                .entity_id_counter=0x498, .active_tracking_contexts=0xebbfc0, .shared_tracker=0xebc030,
                .manager_owner=0, .execution_mode=layout.entity_manager_offset + 8,
                .buffer_capacity=0x100000, .authority_mode=0};
            layout.building_prop_spawn_layout = BuildingPropSpawnLayout{2088, 0};
        }
        if (version == 2 && profile.contains("entityCommandQueueLayout")) {
            const auto& model = profile.at("entityCommandQueueLayout");
            if (!layout.ecs || !layout.authority || !layout.write.enabled || !model.is_object() || model.size() != 10 ||
                model.at("id") != "ecs-entity-command-queue-v1")
                throw std::runtime_error("invalid original entity command queue layout");
            const auto number = [&](const char* key, std::uint64_t maximum = 0x4000000) {
                const auto& value = model.at(key);
                if (!value.is_number_integer() ||
                    (!value.is_number_unsigned() && value.get<std::int64_t>() < 0) ||
                    !value.get<std::uint64_t>() || value.get<std::uint64_t>() > maximum)
                    throw std::runtime_error(std::string("invalid entity command queue value: ") + key);
                return value.get<std::size_t>();
            };
            EntityCommandQueueLayout queue;
            queue.task_state = number("taskStateOffset");
            queue.task_records = number("taskRecordsOffset");
            queue.task_count = number("taskCountOffset");
            queue.queue_records = number("queueRecordsOffset");
            queue.queue_count = number("queueCountOffset");
            queue.task_record_stride = number("taskRecordStride", 0x10000);
            queue.entity_id_counter = number("entityIdCounterOffset");
            queue.active_tracking_contexts = number("activeTrackingContextsOffset");
            queue.buffer_capacity = number("bufferCapacity", std::numeric_limits<std::uint32_t>::max());
            // Cover the largest command snapshot used by the admitted adapters.
            if (queue.task_record_stride < 3 * sizeof(std::uintptr_t) || queue.buffer_capacity < 160)
                throw std::runtime_error("invalid original entity command queue extent");
            queue.shared_tracker = layout.write.tracking_context;
            queue.manager_owner = layout.manager_owner_offset;
            queue.execution_mode = layout.write.execution_mode;
            queue.authority_mode = layout.write.authority_mode;
            layout.entity_command_queue_layout = queue;
        }
        if (version == 2 && profile.contains("buildingPropSpawnLayout")) {
            const auto& model = profile.at("buildingPropSpawnLayout");
            if (!layout.entity_command_queue_layout || !model.is_object() || model.size() != 3 ||
                model.at("id") != "building-prop-spawn-v1")
                throw std::runtime_error("invalid original building prop spawn layout");
            const auto number = [&](const char* key, std::uint64_t minimum, std::uint64_t maximum) {
                const auto& value = model.at(key);
                if (!value.is_number_integer() ||
                    (!value.is_number_unsigned() && value.get<std::int64_t>() < 0) ||
                    value.get<std::uint64_t>() < minimum || value.get<std::uint64_t>() > maximum)
                    throw std::runtime_error(std::string("invalid building prop spawn value: ") + key);
                return value.get<std::uint64_t>();
            };
            BuildingPropSpawnLayout spawn;
            spawn.source_entity_ids = static_cast<std::size_t>(number("sourceEntityIdsOffset", 1, 0x4000000));
            spawn.allocation_failure_marker = static_cast<std::uint32_t>(number("allocationFailureMarkerValue", 0,
                std::numeric_limits<std::uint32_t>::max()));
            layout.building_prop_spawn_layout = spawn;
        }
        if (profile.contains("resourceWriteLayout")) {
            const auto& model = profile.at("resourceWriteLayout");
            auto& r = layout.resource_write;
            if (model.at("id") != "resource-atomic-scalar-v1" || !profile.contains("ecsWriteLayout") ||
                !profile.at("provenance").value("resourceAtomicScalarWriteValidated", false))
                throw std::runtime_error("resource scalar writes lack original worker lifetime validation");
            r.enabled = model.value("enabled", false);
            r.service_thread = offset(model, "serviceThreadOffset", 65536);
            r.callback = offset(model, "threadCallbackRva", image_size - 1);
            r.initialized = offset(model, "serviceInitializedOffset", 65536);
            r.initialized_value = model.at("serviceInitializedValue").get<std::uint32_t>();
            r.reader = offset(profile.at("resourceLayout"), "resourceServiceReaderOffset", 65536);
            r.payload_pointer = offset(model, "servicePayloadPointerOffset", 65536);
            r.payload_size = offset(model, "servicePayloadSizeOffset", 65536);
            const auto name = model.at("threadName").get<std::string>();
            if (!r.service_thread || !r.callback || r.initialized_value != 1 || name.empty() ||
                name.size() > layout.write.name_capacity_value || name.find('\0') != std::string::npos)
                throw std::runtime_error("invalid resource content worker layout");
            std::copy(name.begin(), name.end(), r.name.begin());
            r.name_size = name.size();
        }
        if (layout.dedicated_server && (!layout.ecs || !layout.authority ||
            !profile.contains("ecsWriteLayout") || layout.root_rva || layout.root_vtable || layout.client_offset))
            throw std::runtime_error("dedicated server requires a validated registered Scene owner layout");
        auto services = profile.value("serviceLayout", nlohmann::json{});
        if (!services.is_null() &&
            (!layout.write.enabled || !profile.at("provenance").value("servicePlainDataValidated", false)))
            throw std::runtime_error("named service data lacks original mutable/lifetime validation");
        std::scoped_lock lock(context_mutex);
        active_layout = layout;
        system_dependency_layout = std::move(dependencies);
        service_layout = std::move(services);
        configured = true;
        error.clear();
        return true;
    } catch (const std::exception& exception) {
        Shutdown();
        error = exception.what();
        return false;
    }
}
}
