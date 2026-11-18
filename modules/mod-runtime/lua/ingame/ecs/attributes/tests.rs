use super::*;
use crate::runtime_resolution::attributes::decode_program;
use crate::runtime_resolution::attributes::{header_layout, tests::header_fixture};
use kfc::{
    container::{KFCFile, KFCReader},
    reflection::{TypeIndex, TypeRegistry},
};

fn raw_update_fixture(compact: bool, scalar: &str) -> (Catalog, Vec<u8>, usize) {
    let registry = header_fixture(compact);
    let header = header_layout(&registry, TypeIndex::new(8)).unwrap();
    let start = if compact { 8 } else { 36 };
    let root = 0x04b6aa8bu32;
    let mut catalog = Catalog::default();
    let names = [
        "Current", "Min", "Max", "Rampup", "Timeout", "Recharge", "Base", "Adder",
    ];
    for (index, name) in names.into_iter().enumerate() {
        let words = if index == 0 {
            vec![0x20000, 0x20001, 0x20002, 0xcffff]
        } else if index == 2 && scalar == "keen::sint32" {
            vec![
                0x30000, 0x20001, 0x20002, 0x20006, 0x20007, 0x6ffff, 0xdffff,
            ]
        } else {
            Vec::new()
        };
        catalog.entries.insert(
            root + index as u32,
            json!({
                "name":name,"hash":root + index as u32,"root_hash":root,"index":index,
                "scalar_type":scalar,"calculation_scalar_type":scalar,
                "calculation_sign_independent":false,"calculation_encoding_verified":true,
                "calculation":{"decoded":true},"calculation_words":words
            }),
        );
    }
    let mut component = json!({"qualified_name":"keen::ecs::Stamina","size":start+32,
            "header":header,"storage_offset":start,"storage_size":32,"storage_scalar_type":scalar});
    let mut bytes = vec![0; start + 32];
    bytes[..4].copy_from_slice(&root.to_le_bytes());
    if compact {
        bytes[4..6].copy_from_slice(&(start as u16).to_le_bytes());
        bytes[6] = 1;
        bytes[7] = 8;
    } else {
        bytes[8..12].copy_from_slice(&(start as u32).to_le_bytes());
        bytes[12..16].copy_from_slice(&8u32.to_le_bytes());
        bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
        let guid = Guid::parse("f552f8bc-ada0-4a58-8739-4081ce0d3873").unwrap();
        bytes[20..36].copy_from_slice(guid.data());
        component["definition_offset"] = json!(20);
        component["definition_guid"] = json!(guid.to_string());
    }
    for (index, value) in [50u32, 0, 100, 0, 0, 0, 100, 0].into_iter().enumerate() {
        let bits = if scalar == "keen::float32" {
            (value as f32).to_bits()
        } else {
            value
        };
        bytes[start + index * 4..start + (index + 1) * 4].copy_from_slice(&bits.to_le_bytes());
    }
    catalog.layouts.push(component);
    (catalog, bytes, start)
}

#[test]
fn raw_attribute_write_recalculates_derived_values_and_refreshes_complete_scalar_masks() {
    for compact in [false, true] {
        let (catalog, before, start) = raw_update_fixture(compact, "keen::sint32");
        let mut after = before.clone();
        let mut mask = vec![0; before.len()];
        after[start + 24..start + 28].copy_from_slice(&200i32.to_le_bytes());
        mask[start + 24] = 1;
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1",
        )
        .unwrap();
        assert_eq!(
            i32::from_le_bytes(after[start..start + 4].try_into().unwrap()),
            100
        );
        assert_eq!(
            i32::from_le_bytes(after[start + 8..start + 12].try_into().unwrap()),
            200
        );
        assert_eq!(&after[..start], &before[..start]);
        for index in 0..8 {
            assert_eq!(
                &mask[start + index * 4..start + (index + 1) * 4],
                if [0, 2, 6].contains(&index) {
                    &[1, 1, 1, 1]
                } else {
                    &[0, 0, 0, 0]
                }
            );
        }
        assert!(mask[..start].iter().all(|byte| *byte == 0));
    }
}

#[test]
fn raw_attribute_write_rejects_headers_uninitialized_storage_and_unmasked_changes() {
    for compact in [false, true] {
        let (catalog, before, start) = raw_update_fixture(compact, "keen::sint32");
        let header_offsets = if compact {
            vec![0, 4, 6, 7]
        } else {
            vec![0, 8, 12, 16, 20]
        };
        for offset in header_offsets {
            let mut after = before.clone();
            let mut mask = vec![0; before.len()];
            after[offset] ^= 1;
            mask[offset] = 1;
            let candidate = after.clone();
            let selected = mask.clone();
            assert!(
                recalculate_bound_component_update(
                    &catalog,
                    "keen::ecs::Stamina",
                    &before,
                    &mut after,
                    &mut mask,
                    "attribute-command-v1"
                )
                .unwrap_err()
                .contains("read-only")
            );
            assert_eq!((after, mask), (candidate, selected));
        }
        let mut after = before.clone();
        let mut mask = vec![0; before.len()];
        mask[0] = 1;
        assert!(
            recalculate_bound_component_update(
                &catalog,
                "keen::ecs::Stamina",
                &before,
                &mut after,
                &mut mask,
                "attribute-command-v1"
            )
            .is_err()
        );
        mask.fill(0);
        after[start] ^= 1;
        assert!(
            recalculate_bound_component_update(
                &catalog,
                "keen::ecs::Stamina",
                &before,
                &mut after,
                &mut mask,
                "attribute-command-v1"
            )
            .unwrap_err()
            .contains("write mask")
        );
        let mut uninitialized = before.clone();
        uninitialized[if compact { 6 } else { 16 }] = 0;
        let mut after = uninitialized.clone();
        assert!(
            recalculate_bound_component_update(
                &catalog,
                "keen::ecs::Stamina",
                &uninitialized,
                &mut after,
                &mut mask,
                "attribute-command-v1"
            )
            .unwrap_err()
            .contains("uninitialized")
        );
    }
}

#[test]
fn raw_attribute_write_requires_verified_model_programs_and_scalar_domain() {
    let (mut catalog, before, start) = raw_update_fixture(true, "keen::sint32");
    let mut after = before.clone();
    let mut mask = vec![0; before.len()];
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "unknown-model"
        )
        .unwrap_err()
        .contains("not validated")
    );
    catalog.entries.values_mut().next().unwrap()["calculation_words"] = json!([0xffff0000u32]);
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .is_err()
    );
    assert_eq!(after, before);
    assert!(mask.iter().all(|byte| *byte == 0));

    let (mut conflict, before, _) = raw_update_fixture(true, "keen::sint32");
    for entry in conflict.entries.values_mut() {
        entry["scalar_type"] = json!("keen::uint32");
    }
    let mut after = before.clone();
    assert!(
        recalculate_bound_component_update(
            &conflict,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .unwrap_err()
        .contains("signedness-dependent")
    );

    let (mut catalog, before, start2) = raw_update_fixture(true, "keen::sint32");
    assert_eq!(start, start2);
    for entry in catalog.entries.values_mut() {
        entry["scalar_type"] = json!("keen::uint32");
        entry["calculation_scalar_type"] = json!("keen::uint32");
        entry["calculation_sign_independent"] = json!(true);
        entry["calculation_words"] = json!([]);
    }
    let mut after = before.clone();
    after[start + 28..start + 32].copy_from_slice(&u32::MAX.to_le_bytes());
    mask[start + 28..start + 32].fill(1);
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .unwrap_err()
        .contains("common integer range")
    );

    let (catalog, before, start) = raw_update_fixture(true, "keen::float32");
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut after = before.clone();
        let mut mask = vec![0; before.len()];
        after[start + 28..start + 32].copy_from_slice(&invalid.to_bits().to_le_bytes());
        mask[start + 28..start + 32].fill(1);
        assert!(
            recalculate_bound_component_update(
                &catalog,
                "keen::ecs::Stamina",
                &before,
                &mut after,
                &mut mask,
                "attribute-command-v1"
            )
            .unwrap_err()
            .contains("finite scalar")
        );
    }
}

#[test]
fn named_attribute_updates_validate_unselected_programs_and_derived_result_domains() {
    let (mut catalog, before, start) = raw_update_fixture(true, "keen::sint32");
    let sibling = 0x04b6aa8bu32 + 7;
    catalog.entries.get_mut(&sibling).unwrap()["calculation"]["decoded"] = json!(false);
    let mut after = before.clone();
    let mut mask = vec![0; before.len()];
    after[start..start + 4].copy_from_slice(&75i32.to_le_bytes());
    mask[start..start + 4].fill(1);
    let input: Vec<_> = after[start..]
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    // Arithmetic alone accepts the root. A live write must also validate
    // the untouched sibling's program evidence before reaching native CAS.
    assert!(catalog.recalculate(0x04b6aa8b, &input).is_ok());
    let candidate = after.clone();
    let selected = mask.clone();
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .unwrap_err()
        .contains("unverified")
    );
    assert_eq!((&after, &mask), (&candidate, &selected));

    let (mut catalog, before, start) = raw_update_fixture(true, "keen::sint32");
    let sibling = catalog.entries.get_mut(&sibling).unwrap();
    sibling["scalar_type"] = json!("keen::uint32");
    sibling["calculation_sign_independent"] = json!(true);
    sibling["calculation_words"] = json!([0x5ffffu32, u32::MAX]);
    let mut after = before.clone();
    let mut mask = vec![0; before.len()];
    after[start..start + 4].copy_from_slice(&75i32.to_le_bytes());
    mask[start..start + 4].fill(1);
    let input: Vec<_> = after[start..]
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    assert_eq!(
        catalog.recalculate(0x04b6aa8b, &input).unwrap().0[7],
        u32::MAX
    );
    let candidate = after.clone();
    let selected = mask.clone();
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .unwrap_err()
        .contains("common integer range")
    );
    assert_eq!((after, mask), (candidate, selected));
}

#[test]
fn raw_attribute_policy_follows_actual_reflection_inheritance_and_exact_storage() {
    let registry = header_fixture(true);
    assert!(is_attribute_component(&registry, "keen::ecs::Stamina").unwrap());
    assert!(!is_attribute_component(&registry, "keen::HashKey32").unwrap());
    let (mut catalog, before, _) = raw_update_fixture(true, "keen::sint32");
    catalog.layouts.clear();
    let mut after = before.clone();
    let mut mask = vec![0; before.len()];
    assert!(
        recalculate_bound_component_update(
            &catalog,
            "keen::ecs::Stamina",
            &before,
            &mut after,
            &mut mask,
            "attribute-command-v1"
        )
        .unwrap_err()
        .contains("no verified")
    );
}

#[test]
fn attribute_snapshot_accepts_compact_header_without_reading_storage_as_header() {
    for compact in [false, true] {
        let registry = header_fixture(compact);
        let header = header_layout(&registry, TypeIndex::new(8)).unwrap();
        let start = if compact { 8 } else { 36 };
        let mut component =
            json!({"size":start+32,"header":header,"storage_offset":start,"storage_size":32});
        let root = 0x04b6aa8bu32;
        let descriptor = json!({"root_hash":root,"index":0});
        let mut bytes = vec![0; start + 32];
        bytes[..4].copy_from_slice(&root.to_le_bytes());
        if compact {
            bytes[4..6].copy_from_slice(&(start as u16).to_le_bytes());
            bytes[6] = 1;
            bytes[7] = 8;
        } else {
            bytes[8..12].copy_from_slice(&(start as u32).to_le_bytes());
            bytes[12..16].copy_from_slice(&8u32.to_le_bytes());
            bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
            let guid = Guid::parse("f552f8bc-ada0-4a58-8739-4081ce0d3873").unwrap();
            bytes[20..36].copy_from_slice(guid.data());
            component["definition_offset"] = json!(20);
            component["definition_guid"] = json!(guid.to_string());
        }
        bytes[start..start + 4].copy_from_slice(&50u32.to_le_bytes());
        assert_eq!(
            storage_range(&descriptor, &component, &bytes).unwrap(),
            start..start + 4
        );
        let mut corrupt = bytes.clone();
        corrupt[0] ^= 1;
        assert!(storage_range(&descriptor, &component, &corrupt).is_err());
        let flags = component["header"]["flags"]["offset"].as_u64().unwrap() as usize;
        bytes[flags] = 0;
        assert!(storage_range(&descriptor, &component, &bytes).is_err());
    }
}

#[test]
fn attribute_entity_join_requires_one_initialized_root_match() {
    let registry = header_fixture(true);
    let header = header_layout(&registry, TypeIndex::new(8)).unwrap();
    let root = 0x04b6aa8bu32;
    let mut catalog = Catalog::default();
    for index in 0..8 {
        catalog.entries.insert(
            index,
            json!({"root_hash":root,"index":index,
                "scalar_type":"keen::uint32","calculation_sign_independent":true}),
        );
    }
    for name in ["keen::ecs::Stamina", "keen::ecs::DifferentName"] {
        catalog
            .layouts
            .push(json!({"qualified_name":name,"size":40,"header":header,
                "storage_offset":8,"storage_size":32,"storage_scalar_type":"keen::uint32"}));
    }
    let descriptor = catalog.entries[&0].clone();
    let mut valid = vec![0; 40];
    valid[..4].copy_from_slice(&root.to_le_bytes());
    valid[4..6].copy_from_slice(&8u16.to_le_bytes());
    valid[6] = 1;
    valid[7] = 8;
    let mut different_root = valid.clone();
    different_root[0] ^= 1;
    let (bound, component, _) = resolve_storage(&catalog, &descriptor, |component| {
        Some(if component["qualified_name"] == "keen::ecs::Stamina" {
            different_root.clone()
        } else {
            valid.clone()
        })
    })
    .unwrap();
    assert_eq!(component["qualified_name"], "keen::ecs::DifferentName");
    assert_eq!(bound["storage_writable"], true);
    assert!(
        resolve_storage(&catalog, &descriptor, |_| Some(valid.clone()))
            .unwrap_err()
            .contains("multiple")
    );
    valid[6] = 0;
    assert!(
        resolve_storage(&catalog, &descriptor, |_| Some(valid.clone()))
            .unwrap_err()
            .contains("no initialized component")
    );
}

#[test]
#[ignore = "set SHROUDFORGE_TEST_EXE to inspect current installed attribute definitions"]
fn installed_attribute_resolution() {
    let exe = std::path::PathBuf::from(std::env::var("SHROUDFORGE_TEST_EXE").unwrap());
    let registry = TypeRegistry::load_from_executable(&exe).unwrap();
    let file = KFCFile::from_path(exe.with_extension("kfc"), false).unwrap();
    let mut reader = KFCReader::new(
        exe.parent().unwrap(),
        exe.file_stem().unwrap().to_str().unwrap(),
    )
    .unwrap()
    .into_cursor()
    .unwrap();
    let catalog = Catalog::load(&registry, &file, &mut reader).unwrap();
    assert!(!catalog.layouts.is_empty());
    assert_eq!(catalog.entries[&0x04b6aa8b]["name"], "Stamina");
    assert_eq!(catalog.entries[&0x8eb84995]["name"], "Health");
    assert!(
        catalog
            .entries
            .values()
            .all(|entry| entry["calculation"]["decoded"] == true)
    );
    for layout in &catalog.layouts {
        let ty = registry
            .get_by_name(kfc::reflection::LookupKey::Qualified(
                layout["qualified_name"].as_str().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            layout["header"],
            header_layout(&registry, ty.index).unwrap()
        );
        assert_eq!(layout["header"]["flags"]["initialized_mask"], 1);
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/enshrouded-attribute-resolution.json");
    std::fs::write(path, serde_json::to_vec_pretty(&json!({"version":registry.version,
            "entries":catalog.entries.values().collect::<Vec<_>>(),"component_layouts":catalog.layouts})).unwrap()).unwrap();
    println!(
        "{} original attributes and {} reflected component storage layouts inspected, no live storage association assumed",
        catalog.entries.len(),
        catalog.layouts.len()
    );
    if let Ok(path) = std::env::var("SHROUDFORGE_TEST_ECS_SNAPSHOTS") {
        let capture: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let snapshots = capture["snapshots"].as_array().unwrap();
        let mut entities = std::collections::BTreeMap::<
            (u32, u32),
            std::collections::BTreeMap<String, Vec<u8>>,
        >::new();
        for snapshot in snapshots {
            let hex = snapshot["bytes"].as_str().unwrap();
            let bytes = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    ((pair[0] as char).to_digit(16).unwrap() * 16
                        + (pair[1] as char).to_digit(16).unwrap()) as u8
                })
                .collect();
            entities
                .entry((
                    uint(&snapshot["entityId"]).unwrap(),
                    uint(&snapshot["generation"]).unwrap(),
                ))
                .or_default()
                .insert(snapshot["component"].as_str().unwrap().to_owned(), bytes);
        }
        let lua = Lua::new();
        let mut roots_checked = 0;
        let mut root_types = std::collections::BTreeSet::new();
        let mut scalar_values_checked = 0;
        for snapshot in snapshots
            .iter()
            .filter(|snapshot| !snapshot["rootId"].is_null())
        {
            let root = uint(&snapshot["rootId"]).unwrap();
            let source = &entities[&(
                uint(&snapshot["entityId"]).unwrap(),
                uint(&snapshot["generation"]).unwrap(),
            )];
            let descriptor = &catalog.entries[&root];
            let (_, component, bytes) = resolve_storage(&catalog, descriptor, |component| {
                source
                    .get(component["qualified_name"].as_str().unwrap())
                    .cloned()
            })
            .unwrap();
            assert_eq!(component["qualified_name"], snapshot["component"]);
            let start = uint(&component["storage_offset"]).unwrap() as usize;
            let end = start + uint(&component["storage_size"]).unwrap() as usize;
            let values = bytes[start..end]
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            let entries = catalog.root_entries(root).unwrap();
            let scalar = component["storage_scalar_type"].as_str().unwrap();
            let result = root_result(&entries, scalar, &values, Vec::new()).unwrap();
            assert_eq!(
                result["values"].as_object().unwrap().len(),
                snapshot["values"].as_object().unwrap().len()
            );
            let LuaValue::Table(result) =
                crate::lua::convert_json::json_to_lua(&lua, &result).unwrap()
            else {
                panic!("root must become Lua table");
            };
            let values = result.raw_get::<Table>("values").unwrap();
            for (name, expected) in snapshot["values"].as_object().unwrap() {
                let value = values.raw_get::<LuaValue>(name.as_str()).unwrap();
                let actual = match value {
                    LuaValue::Integer(value) => value as f64,
                    LuaValue::Number(value) => value,
                    _ => panic!("attribute scalar did not become Lua number"),
                };
                if scalar == "keen::float32" {
                    assert_eq!(
                        (actual as f32).to_bits(),
                        (expected.as_f64().unwrap() as f32).to_bits(),
                        "{name}"
                    );
                } else {
                    assert_eq!(Some(actual), expected.as_f64(), "{name}");
                }
                scalar_values_checked += 1;
            }
            roots_checked += 1;
            root_types.insert(root);
        }
        let report = json!({"source_capture":path,"capture_execution":"external-read-only-process-snapshot",
                "new_dll_or_live_lua_verified":false,"root_snapshots_checked":roots_checked,
                "distinct_root_types_checked":root_types.len(),"scalar_values_compared":scalar_values_checked,
                "resolution":"production-per-entity-root-join-and-Lua-root-result"});
        std::fs::write(
            format!("{path}.attribute-codec.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!(
            "Production entity-root join and Lua results checked {roots_checked} external root snapshots, {} root types and {scalar_values_checked} scalar values",
            root_types.len()
        );
    }
}

#[test]
#[ignore = "set SHROUDFORGE_TEST_EXE to the KFC3 client/server executable used by this regression"]
fn installed_attributes() {
    let exe = std::path::PathBuf::from(std::env::var("SHROUDFORGE_TEST_EXE").unwrap());
    let types = TypeRegistry::load_from_executable(&exe).unwrap();
    let file = KFCFile::from_path(exe.with_extension("kfc"), false).unwrap();
    let mut reader = KFCReader::new(
        exe.parent().unwrap(),
        exe.file_stem().unwrap().to_str().unwrap(),
    )
    .unwrap()
    .into_cursor()
    .unwrap();
    let catalog = Catalog::load(&types, &file, &mut reader).unwrap();
    assert_eq!(catalog.entries[&0x04b6aa8b]["name"], "Stamina");
    assert_eq!(catalog.entries[&0xf443c410]["name"], "Stamina_Max");
    assert_eq!(catalog.entries[&0x8eb84995]["name"], "Health");
    assert!(catalog.entries.len() >= 304);
    for entry in catalog.entries.values() {
        let components = entry["components"].as_array().unwrap();
        let expected_resolution = if components.is_empty() {
            "per-entity-initialized-root-id"
        } else {
            "reflected-definition-guid"
        };
        assert_eq!(entry["storage_resolution"], expected_resolution);
    }
    let mana = catalog
        .entries
        .values()
        .find(|entry| entry["name"] == "ManaRechargeMod")
        .unwrap();
    assert!(mana["resource_ids"].as_array().unwrap().is_empty());
    let mana_components = mana["components"].as_array().unwrap();
    if mana_components.is_empty() {
        assert_eq!(mana["storage_resolution"], "per-entity-initialized-root-id");
    } else {
        assert!(mana_components.iter().any(|component| {
            component["qualified_name"] == "keen::ecs::ManaRechargeMod"
        }));
    }
    let freeze = catalog
        .entries
        .values()
        .find(|entry| entry["name"] == "FreezingResistance")
        .unwrap();
    if freeze["components"].as_array().unwrap().len() != 1 {
        assert_eq!(freeze["calculation_writable"], false);
    }
    assert!(encode_attribute_scalar(
        freeze,
        freeze["scalar_type"].as_str().unwrap(),
        &LuaValue::Integer(-1)
    )
    .is_err());
    let heat = catalog
        .entries
        .values()
        .find(|entry| entry["name"] == "BodyHeat")
        .unwrap();
    assert_eq!(heat["calculation_writable"], false);
    for entry in catalog.entries.values().filter(|e| e["index"] == 0) {
        let root = uint(&entry["root_hash"]).unwrap();
        let entries = catalog.root_entries(root).unwrap();
        catalog.recalculate(root, &vec![0; entries.len()]).unwrap();
    }
    // Derived maxima run before the root clamp and rescale the current
    // value through LoadRef, as the native descending recompute loop does.
    let (values, trace) = catalog
        .recalculate(0x04b6aa8b, &[50, 0, 100, 2000, 250, 95, 200, 0])
        .unwrap();
    assert_eq!((values[0], values[2]), (100, 200));
    assert!(!trace.is_empty());
    let mapped = catalog
        .entries
        .values()
        .filter(|v| v["storage_writable"] == true)
        .count();
    let unique_components = catalog
        .entries
        .values()
        .filter(|entry| entry["components"].as_array().unwrap().len() == 1)
        .count();
    assert!(mapped <= unique_components);
    if unique_components == catalog.entries.len() {
        assert!(mapped >= 290);
    }
    let output = exe.file_stem().unwrap().to_str().unwrap();
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let path = artifacts.join(format!("{output}-attribute-catalog.json"));
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&json!({"entries":catalog.entries.values().collect::<Vec<_>>()}))
            .unwrap(),
    )
    .unwrap();
    let target = if output.contains("server") {
        "server"
    } else {
        "client"
    };
    if let Ok(bytes) = std::fs::read(artifacts.join(format!("{target}-attribute-live.json"))) {
        let snapshots: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
        for snapshot in &snapshots {
            let lua = Lua::new();
            let selector = LuaValue::String(
                lua.create_string(snapshot["component"].as_str().unwrap())
                    .unwrap(),
            );
            let descriptor = catalog.get(&selector).unwrap();
            let bytes = crate::lua::ingame::prove_buffer_transform::unhex(
                snapshot["bytes"].as_str().unwrap(),
            )
            .unwrap();
            let component = &descriptor["components"][0];
            assert_eq!(
                storage_range(descriptor, component, &bytes).unwrap(),
                36..40
            );
            let mut corrupt = bytes.clone();
            corrupt[8] = 0xff;
            assert!(storage_range(descriptor, component, &corrupt).is_err());
            corrupt = bytes.clone();
            corrupt[0] ^= 1;
            assert!(storage_range(descriptor, component, &corrupt).is_err());
        }
        println!(
            "{} read-only live component snapshots accepted. Corrupt headers rejected",
            snapshots.len()
        );
    }
    println!(
        "{} original attribute IDs, {mapped} with unique reflected component storage",
        catalog.entries.len()
    );
}
#[test]
fn expected_attribute_values_reject_stale_bounds_and_other_roots() {
    let lua = Lua::new();
    let mut catalog = Catalog::default();
    for (id, name, root, index) in [
        (1, "Stamina", 1, 0),
        (2, "Stamina_Min", 1, 1),
        (3, "Stamina_Max", 1, 2),
        (4, "Health", 4, 0),
    ] {
        catalog
            .entries
            .insert(id, json!({"name":name,"root_hash":root,"index":index}));
    }
    let expected: Table = lua
        .load("return {Stamina_Min=0,Stamina_Max=100}")
        .eval()
        .unwrap();
    assert!(expect_root_values(&catalog, 1, "keen::sint32", &[50, 0, 100], &expected).is_ok());
    for changed in [[50, 1, 100], [50, 0, 200]] {
        assert!(
            expect_root_values(&catalog, 1, "keen::sint32", &changed, &expected)
                .unwrap_err()
                .contains("snapshot changed")
        );
    }
    let cross_root: Table = lua.load("return {Health=50}").eval().unwrap();
    assert!(
        expect_root_values(&catalog, 1, "keen::sint32", &[50, 0, 100], &cross_root)
            .unwrap_err()
            .contains("different root")
    );
    let empty = lua.create_table().unwrap();
    assert!(expect_root_values(&catalog, 1, "keen::sint32", &[50, 0, 100], &empty).is_err());
    let unknown: Table = lua.load("return {unknown=50}").eval().unwrap();
    assert!(expect_root_values(&catalog, 1, "keen::sint32", &[50, 0, 100], &unknown).is_err());
}

#[test]
fn scalar_range_checks() {
    assert!(encode_scalar("keen::sint32", &LuaValue::Integer(i32::MAX as i64 + 1)).is_err());
    assert!(encode_scalar("keen::uint32", &LuaValue::Integer(-1)).is_err());
    assert!(encode_scalar("keen::float32", &LuaValue::Number(f64::INFINITY)).is_err());
    assert!(encode_scalar("keen::sint32", &LuaValue::Number(1.5)).is_err());
}
#[test]
fn program_literals_are_not_misreported_as_opcodes() {
    let operations = [(2, "Load".into()), (5, "Push".into()), (12, "Clamp".into())]
        .into_iter()
        .collect();
    let program = decode_program(
        &[
            json!(0x20000),
            json!(0x5ffff),
            json!(0x20001),
            json!(0xcffff),
        ],
        &operations,
        &[json!({"value":123})],
        &[json!("original_name")],
    );
    assert_eq!(program["instructions"].as_array().unwrap().len(), 3);
    assert_eq!(program["instructions"][0]["attribute_hash"], 123);
    assert_eq!(program["instructions"][1]["immediate_bits"], 0x20001);
    assert_eq!(program["decoded"], true);
    assert_eq!(
        decode_program(&[json!(0x5ffff)], &operations, &[], &[])["decoded"],
        false
    );
    assert_eq!(
        decode_program(&[json!(0x20002)], &operations, &[], &[])["decoded"],
        false
    );
}
