//! Lua attribute bindings. Resolution lives in runtime_resolution.
use super::{AppState, loader, registry};
pub(crate) use crate::runtime_resolution::attributes::Catalog;
use crate::runtime_resolution::attributes::uint;
use kfc::guid::Guid;
use mlua::{Lua, Table, Value as LuaValue};
use mod_loader::Mod;
use serde_json::{Value, json};

impl Catalog {
    pub(crate) fn get(&self, selector: &LuaValue) -> Result<&Value, String> {
        match selector {
            LuaValue::Integer(id) => self
                .entries
                .get(&u32::try_from(*id).map_err(|_| "attribute ID outside u32")?)
                .ok_or_else(|| "attribute ID not found".into()),
            LuaValue::String(name) => {
                let name = name.to_str().map_err(|e| e.to_string())?;
                let found: Vec<_> = self
                    .entries
                    .values()
                    .filter(|v| v["name"] == name.as_ref())
                    .collect();
                match found.as_slice() {
                    [value] => Ok(value),
                    [] => Err(format!("attribute not found: {name}")),
                    _ => Err("ambiguous attribute name; use its ID".into()),
                }
            }
            _ => Err("expected original attribute name or unsigned attribute ID".into()),
        }
    }
}

/// Work on an owned snapshot. Reject relocated/external storage and mismatched
/// definitions instead of dereferencing any engine pointer from Lua.
fn storage_range(
    descriptor: &Value,
    component: &Value,
    bytes: &[u8],
) -> Result<std::ops::Range<usize>, String> {
    if bytes.len() != uint(&component["size"])? as usize {
        return Err("attribute component size changed".into());
    }
    let at = |name: &str, width: usize| -> Result<&[u8], String> {
        let offset = uint(&component["header"][name])? as usize;
        bytes
            .get(offset..offset + width)
            .ok_or_else(|| "attribute header outside component".into())
    };
    let definition = uint(&component["definition_offset"])? as usize;
    if u32::from_le_bytes(at("rootId", 4)?.try_into().unwrap()) != uint(&descriptor["root_hash"])? {
        return Err("live attribute root ID differs from KFC definition".into());
    }
    let expected = Guid::parse(
        component["definition_guid"]
            .as_str()
            .ok_or("missing GUID")?,
    )
    .ok_or("invalid GUID")?;
    if bytes.get(definition..definition + 16) != Some(expected.data().as_slice()) {
        return Err("live attribute definition differs from KFC definition".into());
    }
    let offset = u32::from_le_bytes(at("storageOffset", 4)?.try_into().unwrap()) as usize;
    // storageSize is the element count, not a byte count (checked against the reflected array).
    let count = u32::from_le_bytes(at("storageSize", 4)?.try_into().unwrap()) as usize;
    let flags = u16::from_le_bytes(at("flags", 2)?.try_into().unwrap());
    if flags & 1 == 0
        || offset != uint(&component["storage_offset"])? as usize
        || count.checked_mul(4) != Some(uint(&component["storage_size"])? as usize)
    {
        return Err("attribute is uninitialized or uses an unverified storage layout".into());
    }
    let start = offset
        .checked_add(uint(&descriptor["index"])? as usize * 4)
        .ok_or("attribute offset overflow")?;
    if start + 4 > bytes.len() || start + 4 > offset + count * 4 {
        return Err("attribute element outside storage".into());
    }
    Ok(start..start + 4)
}

pub(crate) fn attach(lua: &Lua, ecs: &Table, r#mod: &Mod) -> mlua::Result<()> {
    ecs.raw_set(
        "evaluate_attributes",
        lua.create_function(|lua, (selector, values): (LuaValue, Table)| {
            let execute = || -> Result<LuaValue, String> {
                let state = lua.app_data_ref::<AppState>().unwrap();
                let catalog = state.attribute_catalog()?;
                let descriptor = catalog.get(&selector)?;
                let root = uint(&descriptor["root_hash"])?;
                let entries = catalog.root_entries(root)?;
                let scalar = descriptor["calculation_scalar_type"]
                    .as_str()
                    .ok_or("unknown scalar type")?;
                let mut input = Vec::new();
                for entry in &entries {
                    let name = entry["name"].as_str().unwrap();
                    let value = values
                        .raw_get::<LuaValue>(name)
                        .map_err(|e| e.to_string())?;
                    input.push(u32::from_le_bytes(
                        encode_scalar(scalar, &value).map_err(|e| format!("{name}: {e}"))?,
                    ));
                }
                if values.pairs::<LuaValue, LuaValue>().count() != entries.len() {
                    return Err("unexpected attribute names in root values".into());
                }
                let (result, trace) = catalog.recalculate(root, &input)?;
                registry::json_to_lua(lua, &root_result(&entries, scalar, &result, trace)?)
                    .map_err(|e| e.to_string())
            };
            Ok(match execute() {
                Ok(v) => (v, None),
                Err(e) => (LuaValue::Nil, Some(e)),
            })
        })?,
    )?;
    for update in [false, true] {
        let r#mod = r#mod.clone();
        ecs.raw_set(if update { "update_attribute" } else { "read_attributes" }, lua.create_function(move |lua, args: mlua::MultiValue| {
            let execute = || -> Result<LuaValue, String> {
                let state = lua.app_data_ref::<AppState>().unwrap();
                let feature = if update { "runtime.ecs.write" } else { "runtime.ecs.read" };
                if let Some(reason) = loader::runtime_denial_reason(&state, &r#mod, feature) { return Err(reason); }
                if !loader::available(&state, &r#mod, feature) { return Err(format!("{feature} unavailable")); }
                if args.len() != if update { 3 } else { 2 } { return Err("expected entity, attribute name/ID, and (for updates) scalar value".into()); }
                let LuaValue::Integer(entity) = args[0] else { return Err("expected entity integer".into()); };
                let entity = u32::try_from(entity).map_err(|_| "entity outside u32")?;
                let catalog = state.attribute_catalog()?;
                let descriptor = catalog.get(&args[1])?;
                let components = descriptor["components"].as_array().unwrap();
                let [component] = components.as_slice() else { return Err("attribute has no unique component storage".into()); };
                let name = component["qualified_name"].as_str().unwrap();
                let scalar = component["storage_scalar_type"].as_str().unwrap();
                let layout = loader::runtime_provider::resolve(name).ok_or("live attribute component unavailable")?;
                if layout.size != uint(&component["size"])? { return Err("attribute component size changed".into()); }
                let before = loader::runtime_provider::read(entity, name, layout.size).ok_or("attribute read failed")?;
                storage_range(descriptor, component, &before)?;
                let root = uint(&descriptor["root_hash"])?;
                let entries = catalog.root_entries(root)?;
                let start = uint(&component["storage_offset"])? as usize;
                let end = start + uint(&component["storage_size"])? as usize;
                let mut values: Vec<u32> = before[start..end].chunks_exact(4).map(|v| u32::from_le_bytes(v.try_into().unwrap())).collect();
                if entries.len() != values.len() { return Err("attribute root storage count changed".into()); }
                let mut trace = Vec::new();
                if update {
                    if descriptor["storage_writable"] != true { return Err(descriptor["storage_reason"].as_str().unwrap_or("unverified scalar layout").into()); }
                    if loader::runtime_provider::functions()?["attribute_calculation_model"] != "attribute-command-v1" {
                        return Err("attribute calculations are not validated for this executable profile".into());
                    }
                    if descriptor["calculation_writable"] != true { return Err(descriptor["calculation_reason"].as_str().unwrap_or("attribute calculation type unresolved").into()); }
                    values[uint(&descriptor["index"])? as usize] = u32::from_le_bytes(encode_attribute_scalar(descriptor, scalar, &args[2])?);
                    (values, trace) = catalog.recalculate(root, &values)?;
                    let mut after = before.clone();
                    let mut mask = vec![0; before.len()];
                    for (index, value) in values.iter().enumerate() {
                        let at = start + index * 4;
                        after[at..at+4].copy_from_slice(&value.to_le_bytes());
                        if after[at..at+4] != before[at..at+4] { mask[at..at+4].fill(1); }
                    }
                    loader::runtime_provider::compare_exchange(entity, name, &before, &mask, &after)?;
                }
                let mut result = root_result(&entries, scalar, &values, trace)?;
                result["applied"] = json!(update);
                result["effect_scope"] = json!("local process; no automatic client/server RPC");
                registry::json_to_lua(lua, &result).map_err(|e| e.to_string())
            };
            Ok(match execute() { Ok(v) => (v, None), Err(e) => (LuaValue::Nil, Some(e)) })
        })?)?;
    }
    ecs.raw_set("get_attributes", lua.create_function(|lua, ()| {
        let state = lua.app_data_ref::<AppState>().unwrap();
        let catalog = match state.attribute_catalog() { Ok(c) => c, Err(reason) => return Ok((LuaValue::Nil, Some(reason))) };
        let result = json!({"version":state.type_registry().version,"count":catalog.entries.len(),
            "entries":catalog.entries.values().collect::<Vec<_>>()});
        Ok((registry::json_to_lua(lua, &result)?, None))
    })?)?;
    ecs.raw_set(
        "get_attribute",
        lua.create_function(|lua, selector: LuaValue| {
            let state = lua.app_data_ref::<AppState>().unwrap();
            let descriptor = state.attribute_catalog().and_then(|c| c.get(&selector));
            match descriptor {
                Ok(d) => Ok((registry::json_to_lua(lua, d)?, None)),
                Err(e) => Ok((LuaValue::Nil, Some(e))),
            }
        })?,
    )?;
    for write in [false, true] {
        let r#mod = r#mod.clone();
        ecs.raw_set(
            if write {
                "write_attribute_storage"
            } else {
                "read_attribute"
            },
            lua.create_function(move |lua, args: mlua::MultiValue| {
                let state = lua.app_data_ref::<AppState>().unwrap();
                let execute = || -> Result<LuaValue, String> {
                    let feature = if write {
                        "runtime.ecs.write"
                    } else {
                        "runtime.ecs.read"
                    };
                    if let Some(reason) = loader::runtime_denial_reason(&state, &r#mod, feature) {
                        return Err(reason);
                    }
                    if !loader::available(&state, &r#mod, feature) {
                        return Err(format!("{feature} unavailable"));
                    }
                    if args.len() != if write { 3 } else { 2 } {
                        return Err(
                            "expected entity, attribute name/ID, and (for writes) scalar value"
                                .into(),
                        );
                    }
                    let entity = match &args[0] {
                        LuaValue::Integer(v) => {
                            u32::try_from(*v).map_err(|_| "entity outside u32")?
                        }
                        _ => return Err("expected entity integer".into()),
                    };
                    let descriptor = state.attribute_catalog()?.get(&args[1])?;
                    let components = descriptor["components"].as_array().unwrap();
                    let [component] = components.as_slice() else {
                        return Err("attribute has no unique reflected component storage".into());
                    };
                    let name = component["qualified_name"].as_str().unwrap();
                    let layout = loader::runtime_provider::resolve(name)
                        .ok_or("live attribute component is unavailable")?;
                    if layout.size != uint(&component["size"])? {
                        return Err("live attribute component size mismatch".into());
                    }
                    let mut bytes = loader::runtime_provider::read(entity, name, layout.size)
                        .ok_or("attribute component read failed")?;
                    let range = storage_range(descriptor, component, &bytes)?;
                    let scalar = component["storage_scalar_type"]
                        .as_str()
                        .ok_or("unresolved attribute scalar type")?;
                    if write {
                        if descriptor["storage_writable"] != true {
                            return Err(descriptor["storage_reason"]
                                .as_str()
                                .unwrap_or("unverified attribute write layout")
                                .into());
                        }
                        let encoded = encode_attribute_scalar(descriptor, scalar, &args[2])?;
                        let mut mask = vec![0u8; bytes.len()];
                        mask[range.clone()].fill(1);
                        bytes[range].copy_from_slice(&encoded);
                        if !loader::runtime_provider::write(entity, name, &mask, &bytes) {
                            return Err("attribute storage write failed".into());
                        }
                        Ok(LuaValue::Boolean(true))
                    } else {
                        let data: [u8; 4] = bytes[range].try_into().unwrap();
                        match scalar {
                            "keen::sint32" => {
                                Ok(LuaValue::Integer(i32::from_le_bytes(data) as i64))
                            }
                            "keen::uint32" => {
                                Ok(LuaValue::Integer(u32::from_le_bytes(data) as i64))
                            }
                            "keen::float32" => {
                                Ok(LuaValue::Number(f32::from_le_bytes(data) as f64))
                            }
                            _ => Err("unsupported attribute scalar".into()),
                        }
                    }
                };
                match execute() {
                    Ok(value) => Ok((value, None)),
                    Err(e) => Ok((
                        if write {
                            LuaValue::Boolean(false)
                        } else {
                            LuaValue::Nil
                        },
                        Some(e),
                    )),
                }
            })?,
        )?;
    }
    Ok(())
}

fn root_result(
    entries: &[&Value],
    scalar: &str,
    values: &[u32],
    trace: Vec<Value>,
) -> Result<Value, String> {
    let mut named = serde_json::Map::new();
    for (entry, bits) in entries.iter().zip(values) {
        let value = match scalar {
            "keen::sint32" => json!(*bits as i32),
            "keen::uint32" => json!(*bits),
            "keen::float32" => {
                let value = f32::from_bits(*bits);
                if !value.is_finite() {
                    return Err("non-finite attribute value".into());
                }
                json!(value)
            }
            _ => return Err("unresolved attribute scalar".into()),
        };
        named.insert(entry["name"].as_str().unwrap().to_owned(), value);
    }
    Ok(
        json!({"root_hash":entries[0]["root_hash"],"values":named,"trace":trace,
        "calculation_model":"attribute-command-v1","scalar_type":scalar}),
    )
}

fn encode_scalar(kind: &str, value: &LuaValue) -> Result<[u8; 4], String> {
    match (kind, value) {
        ("keen::sint32", LuaValue::Integer(v)) => Ok(i32::try_from(*v)
            .map_err(|_| "value outside sint32")?
            .to_le_bytes()),
        ("keen::uint32", LuaValue::Integer(v)) => Ok(u32::try_from(*v)
            .map_err(|_| "value outside uint32")?
            .to_le_bytes()),
        ("keen::float32", LuaValue::Number(v)) if v.is_finite() && (*v as f32).is_finite() => {
            Ok((*v as f32).to_le_bytes())
        }
        ("keen::float32", LuaValue::Integer(v)) => Ok((*v as f32).to_le_bytes()),
        _ => Err(format!("expected finite scalar of type {kind}")),
    }
}

fn encode_attribute_scalar(
    descriptor: &Value,
    kind: &str,
    value: &LuaValue,
) -> Result<[u8; 4], String> {
    if descriptor["write_value_domain"] == "integer-common-range-0-to-2147483647"
        && !matches!(value, LuaValue::Integer(v) if (0..=i32::MAX as i64).contains(v))
    {
        return Err(
            "conflicting signedness: only the common integer range 0..2147483647 is validated"
                .into(),
        );
    }
    encode_scalar(kind, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_resolution::attributes::decode_program;
    use kfc::{
        container::{KFCFile, KFCReader},
        reflection::TypeRegistry,
    };
    #[test]
    #[ignore = "set SHROUDFORGE_TEST_EXE to the installed client/server executable"]
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
        assert_eq!(catalog.entries.len(), 304);
        assert!(
            catalog
                .entries
                .values()
                .all(|entry| entry["components"].as_array().unwrap().len() == 1)
        );
        let mana = catalog
            .entries
            .values()
            .find(|entry| entry["name"] == "ManaRechargeMod")
            .unwrap();
        assert!(mana["resource_ids"].as_array().unwrap().is_empty());
        assert_eq!(
            mana["components"][0]["qualified_name"],
            "keen::ecs::ManaRechargeMod"
        );
        let freeze = catalog
            .entries
            .values()
            .find(|entry| entry["name"] == "FreezingResistance")
            .unwrap();
        assert_eq!(freeze["calculation_writable"], true);
        assert!(encode_attribute_scalar(freeze, "keen::sint32", &LuaValue::Integer(-1)).is_err());
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
        assert!(mapped >= 290);
        let output = exe.file_stem().unwrap().to_str().unwrap();
        let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target");
        let path = artifacts.join(format!("{output}-attribute-catalog.json"));
        std::fs::write(
            path,
            serde_json::to_vec_pretty(
                &json!({"entries":catalog.entries.values().collect::<Vec<_>>()}),
            )
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
                let bytes =
                    super::super::buffer_transform::unhex(snapshot["bytes"].as_str().unwrap())
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
                "{} read-only live component snapshots accepted; corrupt headers rejected",
                snapshots.len()
            );
        }
        println!(
            "{} original attribute IDs, {mapped} with unique reflected component storage",
            catalog.entries.len()
        );
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
}
