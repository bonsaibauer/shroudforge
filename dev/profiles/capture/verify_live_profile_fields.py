"""Read-only live validation for unresolved candidate fields in a profile rebase report."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import itertools
import json
import struct
import sys
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[3]
PYTHON_ROOT = REPOSITORY / "modules/runtime-diagnostics/diagnostics-jobs"
sys.path.insert(0, str(PYTHON_ROOT))

from game_inspection.capture.read_process_memory import Process  # noqa: E402
from game_inspection.inspect.read_pe_image import PE  # noqa: E402


class ValidationError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def set_pointer(root: dict, pointer: str, value) -> None:
    parts = [part.replace("~1", "/").replace("~0", "~") for part in pointer.strip("/").split("/")]
    node = root
    for part in parts[:-1]:
        node = node[int(part)] if isinstance(node, list) else node[part]
    if isinstance(node, list):
        node[int(parts[-1])] = value
    else:
        node[parts[-1]] = value


class LiveProbe:
    def __init__(self, process: Process, base: int):
        self.process = process
        self.base = base

    def bytes(self, address: int, size: int) -> bytes:
        return self.process.read(address, size)

    def u8(self, address: int) -> int:
        return struct.unpack("<B", self.bytes(address, 1))[0]

    def u16(self, address: int) -> int:
        return struct.unpack("<H", self.bytes(address, 2))[0]

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.bytes(address, 4))[0]

    def u64(self, address: int) -> int:
        return struct.unpack("<Q", self.bytes(address, 8))[0]

    def rel32(self, address: int) -> int:
        return address + struct.unpack("<i", self.bytes(address, 4))[0]


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValidationError(reason)


def unique_thread_layout(probe: LiveProbe, layout: dict) -> tuple[int, int, int, int, int, list[dict]]:
    entries = probe.base + layout["threadRegistryEntriesRva"]
    count = probe.u16(probe.base + layout["threadRegistryCountRva"])
    require(0 < count <= layout["threadRegistryCapacity"], "thread-registry-count-invalid")
    names: dict[tuple[int, int], bool] = {}

    def valid_name(pointer: int, length: int) -> bool:
        key = (pointer, length)
        if key in names:
            return names[key]
        if not pointer or not 1 <= length <= 48:
            names[key] = False
            return False
        try:
            raw = probe.bytes(pointer, length)
            names[key] = all(
                65 <= value <= 90 or 97 <= value <= 122 or 48 <= value <= 57 or value in b"_-."
                for value in raw
            ) and any(65 <= value <= 90 or 97 <= value <= 122 for value in raw)
        except Exception:
            names[key] = False
        return names[key]

    candidates = []
    for stride in range(16, 65, 8):
        positions = range(0, stride - 7, 8)
        for handle_offset, name_offset, length_offset in itertools.permutations(positions, 3):
            rows = []
            for index in range(count):
                row = entries + index * stride
                handle = probe.u64(row + handle_offset)
                name = probe.u64(row + name_offset)
                length = probe.u64(row + length_offset)
                if not (handle and 0x10000000000 <= name < 0x800000000000 and valid_name(name, length)):
                    break
                rows.append({"index": index, "handle": handle, "name": name, "length": length})
            else:
                candidates.append((stride, handle_offset, name_offset, length_offset, count, rows))

    require(len(candidates) == 1, f"thread-registry-layout-ambiguous:{len(candidates)}")
    return candidates[0]


def ready_scene(probe: LiveProbe, scene_owner: int, ecs: dict) -> tuple[int, dict]:
    entries = probe.u64(scene_owner + ecs["sceneEntriesPointerOffset"])
    count = probe.u64(scene_owner + ecs["sceneEntriesCountOffset"])
    capacity = probe.u64(scene_owner + ecs["sceneEntriesCapacityOffset"])
    require(count <= capacity <= ecs["maximumSceneEntries"], "scene-list-bounds-invalid")
    require(not count or entries, "scene-list-pointer-missing")
    ready = []
    for index in range(count):
        entry = entries + index * ecs["sceneEntryStride"]
        if probe.u8(entry + ecs["sceneEntryStateOffset"]) != ecs["sceneEntryReadyState"]:
            continue
        context = probe.u64(entry + ecs["sceneEntryContextOffset"])
        uuid = probe.bytes(entry + ecs["sceneEntryUuidOffset"], 16)
        require(context and any(uuid), "ready-scene-context-invalid")
        ready.append((entry, context, uuid))
    require(len(ready) == 1, f"ready-scene-count:{len(ready)}")
    require(probe.u64(scene_owner + ecs["sceneEntriesPointerOffset"]) == entries and
            probe.u64(scene_owner + ecs["sceneEntriesCountOffset"]) == count and
            probe.u64(scene_owner + ecs["sceneEntriesCapacityOffset"]) == capacity,
            "scene-list-changed-during-read")
    entry, context, uuid = ready[0]
    return context, {"entry": hex(entry), "uuid": list(struct.unpack("<QQ", uuid)), "count": count}


def validate_entity_table(probe: LiveProbe, manager: int, registry_owner: int, ecs: dict) -> dict:
    require(probe.u64(manager + ecs["entityManagerOwnerOffset"]) == registry_owner,
            "entity-manager-registry-owner-mismatch")
    count = probe.u64(manager + ecs["entityManagerCount"])
    table = probe.u64(manager + ecs["entityManagerTable"])
    occupancy = probe.u64(manager + ecs["entityManagerOccupancy"])
    keys = probe.u64(manager + ecs["entityManagerKeys"])
    require(count and count <= ecs["maximumEntities"] and not count & (count - 1),
            "entity-manager-count-invalid")
    require(table and occupancy and keys, "entity-manager-array-missing")
    table_bytes = probe.bytes(table, count * 8)
    occupancy_bytes = probe.bytes(occupancy, ((count + 63) // 64) * 8)
    key_bytes = probe.bytes(keys, count * 4)
    occupied = 0
    id_matches = 0
    bad = 0
    generations = []
    pointers = set()
    samples = []
    sampled_rows = []
    for index in range(count):
        bits = struct.unpack_from("<Q", occupancy_bytes, (index // 64) * 8)[0]
        if not (bits >> (index % 64)) & 1:
            continue
        occupied += 1
        key = struct.unpack_from("<I", key_bytes, index * 4)[0]
        pointer = struct.unpack_from("<Q", table_bytes, index * 8)[0]
        if not key or not pointer:
            bad += 1
            continue
        try:
            header = probe.bytes(pointer, 64)
        except Exception:
            bad += 1
            continue
        entity_id = struct.unpack_from("<I", header, ecs["entityId"])[0]
        generation = struct.unpack_from("<I", header, ecs["entityGeneration"])[0]
        entity_layout = struct.unpack_from("<Q", header, ecs["entityLayout"])[0]
        storage = struct.unpack_from("<Q", header, ecs["entityStorage"])[0]
        definition = struct.unpack_from("<Q", header, ecs["entityDefinitionPointer"])[0]
        row = struct.unpack_from("<I", header, ecs["entityRow"])[0]
        if entity_id == key:
            id_matches += 1
        else:
            bad += 1
        if not entity_layout or not storage or not definition or row > (1 << 24) or pointer in pointers:
            bad += 1
        pointers.add(pointer)
        generations.append(generation)
        if len(samples) < 8:
            samples.append({"slot": index, "id": key, "generation": generation, "row": row})
        sampled_rows.append((index, pointer, key, generation))

    require(occupied and len(pointers) == occupied and id_matches == occupied and bad == 0,
            f"entity-table-join-failed:{occupied}:{id_matches}:{bad}")
    stable_count = 0
    step = max(1, len(sampled_rows) // 512)
    for index, pointer, key, generation in sampled_rows[::step][:512]:
        before = probe.bytes(pointer + ecs["entityId"], 8)
        after = probe.bytes(pointer + ecs["entityId"], 8)
        stable_count += before == after and struct.unpack_from("<I", before)[0] == key and \
            struct.unpack_from("<I", before, 4)[0] == generation
    require(stable_count == min(512, len(sampled_rows[::step])), "entity-generation-readback-changed")
    require(probe.u64(manager + ecs["entityManagerCount"]) == count and
            probe.u64(manager + ecs["entityManagerTable"]) == table and
            probe.u64(manager + ecs["entityManagerOccupancy"]) == occupancy and
            probe.u64(manager + ecs["entityManagerKeys"]) == keys,
            "entity-manager-changed-during-read")
    return {"capacity": count, "occupied": occupied, "idKeyMatches": id_matches,
            "stableSamples": stable_count, "generationOffset": ecs["entityGeneration"],
            "generationZero": sum(value == 0 for value in generations),
            "generationMin": min(generations), "generationMax": max(generations), "samples": samples}


def validate_queues(probe: LiveProbe, manager: int, layout: dict) -> dict:
    context = manager + layout["changeTrackerContextOffset"]
    back_pointer = probe.u64(context + layout["changeTrackerManagerOffset"])
    require(back_pointer == manager, "change-tracker-owner-mismatch")
    queues = []
    for offset in range(0, 640, 40):
        count = probe.u64(context + offset + 8)
        capacity = probe.u64(context + offset + 16)
        callback = probe.u64(context + offset + 24)
        data = probe.u64(context + offset)
        if data and capacity and capacity & (capacity - 1) == 0 and count <= capacity:
            queues.append((offset, count, capacity, callback))
    require(len(queues) == 16, f"change-tracker-queue-count:{len(queues)}")
    require([entry[2] for entry in queues[:8]] == [16384] * 8 and
            [entry[2] for entry in queues[8:]] == [4096] * 8,
            "change-tracker-queue-groups-invalid")
    require(len({entry[3] for entry in queues}) == 1 and all(entry[3] >= probe.base and
            entry[3] < probe.base + 0x10000000 for entry in queues), "change-tracker-callback-mismatch")
    require(layout["changeTrackerQueueBOffset"] == queues[8][0], "change-tracker-queue-b-offset-mismatch")
    require(probe.u64(context + layout["changeTrackerManagerOffset"]) == manager,
            "change-tracker-owner-changed")
    return {"queueCount": len(queues), "categoryAOffset": queues[0][0],
            "categoryACapacity": queues[0][2], "categoryBOffset": queues[8][0],
            "categoryBCapacity": queues[8][2], "counts": [entry[1] for entry in queues]}


def validate_systems(probe: LiveProbe, registry_owner: int, layout: dict, base: int) -> dict:
    count = probe.u64(registry_owner + layout["systemCountOffset"])
    require(0 < count <= layout["maximumSystems"], "system-registry-count-invalid")
    matched = 0
    contract_rows = []
    for index in range(count):
        record = registry_owner + layout["systemsOffset"] + index * layout["systemStride"]
        descriptor = probe.u64(record)
        if not descriptor:
            continue
        function = probe.u64(record + 0x28)
        reads = probe.u64(descriptor + 0xA0)
        writes = probe.u64(descriptor + 0xB8)
        normal = probe.u8(record + layout["systemNormalCountOffset"])
        if reads + writes == normal:
            matched += 1
        for contract in layout.get("contracts", []):
            if descriptor == base + contract["mutableSystemDescriptorRva"]:
                contract_rows.append({"index": index, "key": contract["key"],
                    "functionMatches": function == base + contract["mutableSystemFunctionRva"],
                    "normalCount": normal, "readCount": reads, "writeCount": writes})
    require(matched == count, f"system-normal-count-mismatch:{matched}:{count}")
    require(len(contract_rows) == 1 and contract_rows[0]["functionMatches"],
            "named-system-contract-not-unique")
    normal_offset = layout["systemNormalCountOffset"]
    require(layout["systemNormalBindingsOffset"] + 1024 == normal_offset and
            normal_offset + 8 == layout["systemAlternateBindingsOffset"] and
            layout["systemAlternateBindingsOffset"] + 1024 == layout["systemAlternateCountOffset"] and
            layout["systemAlternateCountOffset"] + 8 == layout["systemStride"],
            "system-binding-count-position-mismatch")
    require(probe.u64(registry_owner + layout["systemCountOffset"]) == count,
            "system-registry-changed-during-read")
    return {"systemCount": count, "normalCountsMatched": matched,
            "contract": contract_rows[0], "normalCountOffset": normal_offset,
            "alternateCountOffset": layout["systemAlternateCountOffset"]}


def validate_resource_map(probe: LiveProbe, service: int, registry: int, layout: dict) -> tuple[dict, list[tuple[int, int]]]:
    resource_map = layout["map"]
    reader = probe.u64(service + layout["resourceServiceReaderOffset"])
    metadata = probe.u64(reader + layout["readerMetadataOffset"])
    header = metadata + layout["metadataMapOffset"]
    bucket_pointer = probe.rel32(header + resource_map["bucketRel32Offset"])
    keys_pointer = probe.rel32(header + resource_map["keysRel32Offset"])
    values_pointer = probe.rel32(header + resource_map["valuesRel32Offset"])
    bucket_count = probe.u32(header + resource_map["bucketCountOffset"])
    key_count = probe.u32(header + resource_map["keyCountOffset"])
    value_count = probe.u32(header + resource_map["valueCountOffset"])
    require(bucket_count and not bucket_count & (bucket_count - 1) and
            key_count and key_count == value_count and key_count <= layout["maximumEntries"],
            "resource-map-counts-invalid")

    chunks = []
    chunk_table = probe.u64(service + layout["resourceServiceChunkTableOffset"])
    chunk_count = probe.u32(service + layout["resourceServiceChunkCountOffset"])
    require(chunk_table and 0 < chunk_count <= layout["maximumChunks"], "resource-chunk-table-invalid")
    for index in range(chunk_count):
        entry = chunk_table + index * layout["chunkEntry"]["stride"]
        chunk_base = probe.u64(entry + layout["chunkEntry"]["basePointerOffset"])
        chunk_size = probe.u64(entry + layout["chunkEntry"]["sizeOffset"])
        require(chunk_base and chunk_size, "resource-chunk-entry-invalid")
        chunks.append((chunk_base, chunk_size))

    bucket_bytes = probe.bytes(bucket_pointer, bucket_count * resource_map["bucketStride"])
    intervals = []
    bucket_sum = 0
    invalid_buckets = 0
    for index in range(bucket_count):
        first, length = struct.unpack_from("<II", bucket_bytes, index * resource_map["bucketStride"])
        bucket_sum += length
        if first > key_count or length > key_count - first:
            invalid_buckets += 1
        elif length:
            intervals.append((first, first + length))
    intervals.sort()
    cursor = gaps = overlaps = 0
    for first, end in intervals:
        if first > cursor:
            gaps += first - cursor
        if first < cursor:
            overlaps += cursor - first
        cursor = max(cursor, end)
    if cursor < key_count:
        gaps += key_count - cursor
    require(not invalid_buckets and not gaps and not overlaps and bucket_sum == key_count,
            "resource-bucket-ranges-do-not-partition-keys")

    value_bytes = probe.bytes(values_pointer, value_count * resource_map["valueStride"])
    valid_values = 0
    bad_values = 0
    mapped_payload = None
    registry_handle = probe.u32(registry + layout["itemRegistryHandleOffset"])
    require(0 < registry_handle <= value_count, "item-registry-handle-out-of-range")
    for index in range(value_count):
        offset = index * resource_map["valueStride"]
        resource_offset = struct.unpack_from("<I", value_bytes, offset + layout["valueEntry"]["resourceOffsetOffset"])[0]
        packed = int.from_bytes(value_bytes[offset + layout["valueEntry"]["packedSizeOffset"]:
            offset + layout["valueEntry"]["packedSizeOffset"] + 3], "little")
        chunk_id = value_bytes[offset + layout["valueEntry"]["chunkIndexOffset"]]
        size = packed & layout["valueEntry"]["sizeMask"]
        if chunk_id >= len(chunks) or resource_offset + size > chunks[chunk_id][1]:
            bad_values += 1
        else:
            valid_values += 1
        if index == registry_handle - 1 and chunk_id < len(chunks):
            mapped_payload = chunks[chunk_id][0] + resource_offset
    registry_payload = probe.u64(registry + layout["itemRegistryResourcePointerOffset"])
    require(valid_values == value_count and not bad_values and mapped_payload == registry_payload,
            "resource-values-or-item-registry-handle-mismatch")

    key_bytes = probe.bytes(keys_pointer, key_count * resource_map["keyStride"])
    require(resource_map["keyStride"] == 32 and resource_map["resourceKeySize"] == 24,
            "resource-key-contract-not-supported")
    nonzero_keys = 0
    zero_padding = 0
    for index in range(key_count):
        offset = index * resource_map["keyStride"]
        if struct.unpack_from("<I", key_bytes, offset)[0]:
            nonzero_keys += 1
        if key_bytes[offset + 24:offset + 32] == bytes(8):
            zero_padding += 1
    require(nonzero_keys == key_count and zero_padding == key_count,
            "resource-key-stride-or-size-mismatch")

    evidence = {"bucketCount": bucket_count, "keyCount": key_count, "valueCount": value_count,
            "bucketRanges": len(intervals), "bucketSum": bucket_sum, "invalidBuckets": invalid_buckets,
            "gaps": gaps, "overlaps": overlaps, "validValues": valid_values,
            "chunkCount": chunk_count, "mappedPayloadMatches": mapped_payload == registry_payload,
            "keyRecords": key_count, "keyStride": resource_map["keyStride"],
            "keyPayloadBytes": resource_map["resourceKeySize"], "zeroPaddingRecords": zero_padding,
            "chunkTableOffset": layout["resourceServiceChunkTableOffset"]}
    return evidence, chunks


def validate_item_map(probe: LiveProbe, registry: int, resource_layout: dict, chunks: list[tuple[int, int]]) -> dict:
    item = resource_layout["itemIdMap"]
    map_address = registry + item["mapOffset"]
    bits = probe.u64(map_address + item["bitsPointerOffset"])
    capacity = probe.u64(map_address + item["capacityOffset"])
    keys = probe.u64(map_address + item["keysPointerOffset"])
    values = probe.u64(map_address + item["valuesPointerOffset"])
    count = probe.u32(map_address + item["countOffset"])
    require(capacity and capacity <= item["maximumCapacity"] and not capacity & (capacity - 1) and
            count <= capacity and bits and keys and values, "item-map-header-invalid")
    bit_bytes = probe.bytes(bits, ((capacity + 63) // 64) * 8)
    key_bytes = probe.bytes(keys, capacity * 4)
    value_bytes = probe.bytes(values, capacity * 8)
    occupied = 0
    key_matches = 0
    in_chunks = 0
    unique_ids = set()
    invalid = 0
    for slot in range(capacity):
        if not (struct.unpack_from("<Q", bit_bytes, (slot // 64) * 8)[0] >> (slot % 64)) & 1:
            continue
        occupied += 1
        key = struct.unpack_from("<I", key_bytes, slot * 4)[0]
        pointer = struct.unpack_from("<Q", value_bytes, slot * 8)[0]
        if key in unique_ids:
            invalid += 1
        unique_ids.add(key)
        if probe.u32(pointer + item["itemIdOffset"]) == key:
            key_matches += 1
        if any(base <= pointer and pointer + item["targetSize"] <= base + size for base, size in chunks):
            in_chunks += 1
        else:
            invalid += 1
    require(occupied == count and len(unique_ids) == count and key_matches == count and
            in_chunks == count and invalid == 0,
            f"item-map-entries-do-not-match-resource-items:occupied={occupied},count={count},"
            f"unique={len(unique_ids)},keyMatches={key_matches},inChunks={in_chunks},invalid={invalid},"
            f"capacity={capacity},mapOffset={item['mapOffset']},countOffset={item['countOffset']}")
    return {"mapOffset": item["mapOffset"], "capacity": capacity, "count": count,
            "occupied": occupied, "uniqueIds": len(unique_ids), "keyMatchesPayloadId": key_matches,
            "payloadsInResourceChunks": in_chunks}


def run(args) -> dict:
    executable = args.exe.resolve(strict=True)
    baseline_path = args.baseline.resolve(strict=True)
    static_path = args.static_report.resolve(strict=True)
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    static = json.loads(static_path.read_text(encoding="utf-8"))
    require(static.get("complete") is False, "static-report-already-complete-or-invalid")
    require(static.get("source") and static.get("target") and static.get("evidence", {}).get("unresolved"),
            "static-report-shape-invalid")
    target = static["target"]
    target_kind = "client" if target["target"] == "enshrouded.exe" else "server"
    require(target_kind == args.target, "target-kind-mismatch")
    pe = PE(executable)
    identity = pe.identity()
    require(identity["sha256"] == target["sha256"] and identity["timestamp"] == target["timestamp"] and
            identity["size"] == target["size"], "executable-does-not-match-static-rebase-target")
    require(baseline["image"]["sha256"] == static["source"]["sha256"] and
            baseline["image"]["timestamp"] == static["source"]["timestamp"] and
            baseline["image"]["size"] == static["source"]["size"], "baseline-does-not-match-static-rebase-source")

    profile = json.loads(json.dumps(baseline))
    for field in static["evidence"].get("fields", []):
        if field.get("path") and "after" in field:
            set_pointer(profile, field["path"], field["after"])
    candidates = {field["path"]: field.get("after") for field in static["evidence"]["unresolved"]
                  if field.get("path") and "after" in field}
    require(len(candidates) == len(static["evidence"]["unresolved"]), "unresolved-field-list-incomplete")
    for path, value in candidates.items():
        set_pointer(profile, path, value)
    profile["image"] = {key: target[key] for key in ("sha256", "timestamp", "size")}

    process = Process(args.pid, pe)
    try:
        probe = LiveProbe(process, process.base)
        fields: dict[str, dict] = {}

        def record(paths: list[str], method: str, observations: dict) -> None:
            for path in paths:
                require(path in candidates, f"unexpected-dynamic-field:{path}")
                fields[path] = {"value": candidates[path], "method": method, "observations": observations}

        write_layout = profile["ecsWriteLayout"]
        stride, handle_offset, name_offset, length_offset, thread_count, thread_rows = unique_thread_layout(probe, write_layout)
        require((stride, handle_offset, name_offset, length_offset) ==
                (write_layout["threadRegistryEntryStride"], write_layout["registryEntryHandleOffset"],
                 write_layout["registryEntryNamePointerOffset"], write_layout["registryEntryNameLengthOffset"]),
                "thread-layout-differs-from-profile-candidate")
        record(["/ecsWriteLayout/threadRegistryEntryStride", "/ecsWriteLayout/registryEntryHandleOffset",
                "/ecsWriteLayout/registryEntryNamePointerOffset", "/ecsWriteLayout/registryEntryNameLengthOffset"],
               "unique-live-thread-registry-row-shape", {"rows": thread_count, "validTupleCount": 1,
                "stride": stride, "handleOffset": handle_offset, "nameOffset": name_offset,
                "lengthOffset": length_offset})

        ecs = profile["ecsReadLayout"]
        owner = profile["ownerLayout"]
        if args.target == "client":
            root = probe.base + owner["rootObjectRva"]
            root_vtable = probe.u64(root)
            client = probe.u64(root + owner["clientPointerOffset"])
            controller = probe.u64(client + owner["controllerPointerOffset"])
            main_owner = probe.u64(controller + owner["controllerMainOwnerOffset"])
            require(root_vtable == probe.base + owner["rootVtableRva"] and
                    probe.u64(client) == probe.base + owner["clientVtableRva"] and
                    probe.u64(controller) == probe.base + owner["controllerVtableRva"] and
                    probe.u64(controller + owner["controllerClientBackPointerOffset"]) == client,
                    "client-owner-chain-mismatch")
            scene_owner = probe.u64(client + ecs["clientSceneOwnerOffset"])
            resource_owner = main_owner
        else:
            named = [row for row in thread_rows if probe.bytes(row["name"], row["length"]).decode("ascii") ==
                     write_layout["threadName"]]
            require(len(named) == 1, "dedicated-server-thread-not-unique")
            selected = named[0]
            thread_object = selected["name"] - write_layout["threadInlineNameOffset"]
            scene_owner = thread_object - write_layout["sceneThreadOffset"]
            require(probe.u64(thread_object + write_layout["threadNamePointerOffset"]) == selected["name"] and
                    probe.u64(thread_object + write_layout["threadNameLengthOffset"]) == len(write_layout["threadName"]) and
                    probe.u64(thread_object + write_layout["threadNameCapacityOffset"]) == write_layout["threadNameCapacity"] and
                    probe.u64(thread_object + write_layout["threadHandleOffset"]) == selected["handle"] and
                    probe.u64(thread_object + write_layout["threadContextOffset"]) == scene_owner + write_layout["sceneContextOffset"] and
                    probe.u32(thread_object + write_layout["threadStopOffset"]) == 0 and
                    probe.u64(thread_object + write_layout["threadCallbackOffset"]) == probe.base + write_layout["threadCallbackRva"] and
                    probe.u8(scene_owner + write_layout["sceneStateOffset"]) == write_layout["sceneRunningState"],
                    "dedicated-server-thread-owner-contract-mismatch")
            resource_owner = None

        scene_context, scene_evidence = ready_scene(probe, scene_owner, ecs)
        if args.target == "server":
            resource_owner = scene_context
        execution_owner = probe.u64(scene_context + ecs["mainExecutionOwnerOffset"])
        registry_owner = execution_owner + ecs["executionRegistryOwnerOffset"]
        manager = execution_owner + ecs["executionEntityManagerOffset"]
        entity_evidence = validate_entity_table(probe, manager, registry_owner, ecs)
        record(["/ecsReadLayout/entityGeneration"], "live-entity-table-key-generation-join",
               {"readyScene": scene_evidence, **entity_evidence})

        queue_evidence = validate_queues(probe, manager, write_layout)
        record(["/ecsWriteLayout/changeTrackerQueueBOffset"], "live-change-tracker-queue-groups", queue_evidence)

        system_layout = profile["serviceLayout"]
        system_evidence = validate_systems(probe, registry_owner, system_layout, probe.base)
        if "/serviceLayout/systemNormalCountOffset" in candidates:
            record(["/serviceLayout/systemNormalCountOffset"], "live-named-system-binding-counts", system_evidence)

        resource_layout = profile["resourceLayout"]
        active_context = probe.u64(resource_owner + resource_layout["mainActiveContextOffset"])
        registry = probe.u64(active_context + resource_layout["contextItemRegistryOffset"])
        service = probe.u64(registry + resource_layout["itemRegistryServiceOffset"])
        id_callback = probe.u64(service + resource_layout["resourceServiceIdCallbackOffset"])
        handle_callback = probe.u64(service + resource_layout["resourceServiceHandleCallbackOffset"])
        require(id_callback == probe.base + resource_layout["resourceServiceIdCallbackRva"] and
                handle_callback == probe.base + resource_layout["resourceServiceHandleCallbackRva"],
                "registered-resource-service-callback-mismatch")

        resource_evidence, resource_chunks = validate_resource_map(probe, service, registry, resource_layout)
        item_evidence = validate_item_map(probe, registry, resource_layout, resource_chunks)

        # The owner offsets, item-map tuple and resource-map fields are checked as one live chain.
        item_fields = [path for path in candidates if path.startswith("/resourceLayout/itemRegistry") or
                       path.startswith("/resourceLayout/itemIdMap/")]
        record(item_fields, "live-item-registry-resource-join",
               {"callbacksMatch": True, "registryHandle": probe.u32(registry + resource_layout["itemRegistryHandleOffset"]),
                "registryPayloadMatchesResourceIndex": resource_evidence["mappedPayloadMatches"],
                **item_evidence})
        map_fields = [path for path in candidates if path.startswith("/resourceLayout/map/") or
                      path == "/resourceLayout/resourceServiceChunkTableOffset"]
        record(map_fields, "live-resource-map-chunk-and-key-validation", resource_evidence)

        require(set(fields) == set(candidates),
                f"dynamic-field-coverage:{len(fields)}:{len(candidates)}")
        return {"schemaVersion": 1, "complete": True, "target": target,
                "baseline": {"id": baseline["id"], "sha256": sha256_file(baseline_path)},
                "staticReport": {"path": str(static_path), "sha256": sha256_file(static_path)},
                "executablePath": str(executable), "pid": args.pid, "moduleBase": hex(process.base),
                "capturedAt": dt.datetime.now(dt.timezone.utc).isoformat(),
                "method": "read-only-exact-image-live-profile-layout-validation",
                "fields": [{"path": path, "value": candidates[path], "verified": True,
                            "method": evidence["method"], "observations": evidence["observations"]}
                           for path, evidence in sorted(fields.items())],
                "limitations": ["live memory validation does not prove native call ABI or gameplay effects",
                                "the process was not suspended, so this is a stable sampled view, not an atomic snapshot"]}
    finally:
        process.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=("client", "server"), required=True)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--static-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit("output path already exists")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"schemaVersion": 1, "complete": False, "targetKind": args.target,
              "capturedAt": dt.datetime.now(dt.timezone.utc).isoformat(), "fields": [], "limitations": []}
    error = None
    try:
        report = run(args)
    except Exception as exc:
        error = f"{type(exc).__name__}: {exc}"
        report["error"] = error
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(json.dumps({"complete": report.get("complete", False), "fields": len(report.get("fields", [])),
                      "error": error, "output": str(args.output)}, separators=(",", ":")))
    return 0 if report.get("complete") else 2


if __name__ == "__main__":
    raise SystemExit(main())
