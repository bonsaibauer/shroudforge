//! Attribute identities come from the active KFC container, never FNV(name).
//! Component ownership is joined through the reflected definition GUID.
use super::{
    calculation::{self, Scalar},
    types,
};
use kfc::{
    container::{KFCCursor, KFCFile, KFCReader},
    guid::Guid,
    reflection::{LookupKey, PrimitiveType, TypeRegistry},
    resource::value::Value as KfcValue,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Catalog {
    pub entries: BTreeMap<u32, Value>,
}

impl Catalog {
    pub fn load(
        registry: &TypeRegistry,
        file: &KFCFile,
        reader: &mut KFCCursor<KFCReader>,
    ) -> Result<Self, String> {
        let mut resources = Vec::new();
        for name in [
            "keen::AttributeContainerResource",
            "keen::BaseAttributeResource",
        ] {
            let Some(ty) = registry.get_by_name(LookupKey::Qualified(name)) else {
                continue;
            };
            for id in shroudforge_parser::kfc_format::resources_by_type(file, registry, name) {
                let mut bytes = Vec::new();
                if !reader
                    .read_resource_into(&id, &mut bytes)
                    .map_err(|e| e.to_string())?
                {
                    return Err(format!("missing attribute resource {id}"));
                }
                let value =
                    KfcValue::from_bytes(registry, ty, &bytes).map_err(|e| e.to_string())?;
                resources.push(json!({"id":id.guid().to_string(),"type":name,"value":value}));
            }
        }
        Self::from_resources(registry, &resources)
    }

    fn from_resources(types: &TypeRegistry, resources: &[Value]) -> Result<Self, String> {
        let mut roots: BTreeMap<u32, (Value, Vec<String>, Vec<String>)> = BTreeMap::new();
        for resource in resources {
            let container = resource["type"] == "keen::AttributeContainerResource";
            let values: Vec<_> = if container {
                resource["value"]["attributes"]
                    .as_array()
                    .ok_or("missing container attributes")?
                    .iter()
                    .collect()
            } else {
                vec![&resource["value"]]
            };
            for value in values {
                let root = uint(&value["ids"][0]["value"])?;
                let record = roots
                    .entry(root)
                    .or_insert_with(|| (value.clone(), Vec::new(), Vec::new()));
                if record.0 != *value {
                    return Err(format!(
                        "conflicting container/base definition for {root:#010x}"
                    ));
                }
                let ids = if container {
                    &mut record.2
                } else {
                    &mut record.1
                };
                ids.push(
                    resource["id"]
                        .as_str()
                        .ok_or("missing resource GUID")?
                        .to_owned(),
                );
            }
        }
        if roots.is_empty() {
            return Err("no attribute definitions in this KFC container".into());
        }
        let operations: BTreeMap<u32, String> = types
            .get_by_name(LookupKey::Qualified("keen::AttributeOps"))
            .map(|ty| {
                ty.enum_fields
                    .values()
                    .map(|field| (field.value as u32, field.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut catalog = Self::default();
        let encoding_verified = [
            (1, "Load0"),
            (2, "Load"),
            (3, "LoadRef"),
            (5, "Push"),
            (6, "Add"),
            (7, "Substract"),
            (8, "Multiply"),
            (12, "Clamp"),
            (13, "ScaleToNewMax"),
            (15, "Max"),
        ]
        .iter()
        .all(|(opcode, name)| operations.get(opcode).map(String::as_str) == Some(*name));
        for (root, (value, resource_ids, container_ids)) in roots {
            let ids = value["ids"].as_array().ok_or("missing attribute IDs")?;
            let names = value["debugNames"]
                .as_array()
                .ok_or("missing attribute names")?;
            let structure = value["structure"]
                .as_array()
                .ok_or("missing attribute structure")?;
            if ids.len() != names.len() || ids.len() != structure.len() || ids.len() > 65535 {
                return Err("attribute name/ID/structure lengths disagree".into());
            }
            let scalar_hash = uint(&value["type"]["value"])?;
            let scalar_types: Vec<_> = types
                .iter()
                .filter(|t| {
                    t.internal_hash == scalar_hash
                        && t.size == 4
                        && matches!(
                            t.primitive_type,
                            PrimitiveType::SInt32 | PrimitiveType::UInt32 | PrimitiveType::Float32
                        )
                })
                .collect();
            let scalar = match scalar_types.as_slice() {
                [t] => Some(*t),
                _ => None,
            };
            let mut components = Vec::new();
            for ty in types
                .iter()
                .filter(|t| t.qualified_name.starts_with("keen::ecs::"))
            {
                let fields = types::fields(types, ty.index)?;
                let field =
                    |name: &str| fields.iter().find(|(_, f)| f.name == name).map(|(_, f)| *f);
                let (Some(definition), Some(storage), Some(defaults)) =
                    (field("definition"), field("dataStorage"), &ty.default_value)
                else {
                    continue;
                };
                let offset = definition.data_offset as usize;
                let Some(guid_bytes) = defaults.get(offset..offset + 16) else {
                    continue;
                };
                let guid = Guid::new(guid_bytes.try_into().unwrap()).to_string();
                // The engine initializer hashes the GUID bytes, not a display
                // name. This also resolves roots only present in the container.
                if definition_root(guid_bytes.try_into().unwrap()) != root {
                    continue;
                }
                if !resource_ids.is_empty() && !resource_ids.contains(&guid) {
                    return Err(format!(
                        "attribute definition GUID/hash collision for {root:#010x}"
                    ));
                }
                let Some(array) = types.get(storage.r#type) else {
                    continue;
                };
                let Some(element) = array.inner_type.and_then(|i| types.get(i)) else {
                    continue;
                };
                if array.primitive_type != PrimitiveType::StaticArray
                    || element.size != 4
                    || !matches!(
                        element.primitive_type,
                        PrimitiveType::SInt32 | PrimitiveType::UInt32 | PrimitiveType::Float32
                    )
                    || array.size as usize != ids.len() * 4
                    || storage.data_offset + array.size as u64 > ty.size as u64
                {
                    continue;
                }
                let header_names = ["rootId", "storageOffset", "storageSize", "flags"];
                let mut header = json!({});
                let mut valid = true;
                for name in header_names {
                    let Some(f) = field(name) else {
                        valid = false;
                        break;
                    };
                    let width = if name == "flags" { 2 } else { 4 };
                    if f.data_offset + width > ty.size as u64 {
                        valid = false;
                        break;
                    }
                    header[name] = json!(f.data_offset);
                }
                if valid {
                    components.push(json!({"qualified_name":ty.qualified_name,"qualified_hash":ty.qualified_hash,
                    "size":ty.size,"storage_offset":storage.data_offset,"storage_size":array.size,
                    "definition_offset":definition.data_offset,"definition_guid":guid,"header":header,
                    "storage_scalar_type":element.qualified_name,
                    "engine_server_only":ty.attributes.contains_key("server_only"),
                    "scalar_type_matches":scalar.is_some_and(|s| s.primitive_type == element.primitive_type),
                    "integer_representation_compatible":scalar.is_some_and(|s| matches!(s.primitive_type, PrimitiveType::SInt32 | PrimitiveType::UInt32)) &&
                        matches!(element.primitive_type, PrimitiveType::SInt32 | PrimitiveType::UInt32),
                    "association":"fnv1a-definition-guid-bytes-and-static-array"}));
                }
            }
            let scalar_matches =
                components.len() == 1 && components[0]["scalar_type_matches"] == true;
            let common_integer =
                components.len() == 1 && components[0]["integer_representation_compatible"] == true;
            // These instructions operate on the same 32-bit representation
            // for signed/unsigned interpreters, including wrapping arithmetic.
            // Ordering, division and rescaling are intentionally excluded.
            let sign_independent = structure.iter().all(|node| {
                let Some(words) = node["calculation"].as_array() else {
                    return false;
                };
                let mut pc = 0;
                while pc < words.len() {
                    let Ok(word) = uint(&words[pc]) else {
                        return false;
                    };
                    pc += 1;
                    match word >> 16 {
                        1 | 2 | 6 | 7 | 8 => {}
                        5 if pc < words.len() => pc += 1,
                        _ => return false,
                    }
                }
                true
            });
            for (index, id) in ids.iter().enumerate() {
                let hash = uint(&id["value"])?;
                let name = names[index]
                    .as_str()
                    .filter(|n| !n.is_empty())
                    .ok_or("empty original attribute name")?;
                for link in ["parentIndex", "childIndex", "siblingIndex"] {
                    let target = uint(&structure[index][link])?;
                    if target != 65535 && target as usize >= ids.len() {
                        return Err(format!("invalid attribute {link}"));
                    }
                }
                let words = structure[index]["calculation"]
                    .as_array()
                    .ok_or("missing calculation words")?;
                for word in words {
                    uint(word)?;
                }
                let descriptor = json!({"name":name,"hash":hash,"hash_domain":"attribute-id",
                    "name_source":"kfc-debugNames","root_hash":root,"index":index,"resource_ids":resource_ids,
                    "container_ids":container_ids,"scalar_type_hash":scalar_hash,
                    "scalar_type":scalar.map(|t| t.qualified_name.as_str()),"components":components,
                    "calculation_scalar_type":if components.len() == 1 && (scalar_matches || (common_integer && sign_independent)) {
                        components[0]["storage_scalar_type"].clone()
                    } else {json!(scalar.map(|t| t.qualified_name.as_str()))},
                    "structure":structure[index],"calculation_words":words,
                    "calculation":decode_program(words, &operations, ids, names),
                    "calculation_encoding_verified":encoding_verified,
                    "literal_encoding":if scalar.is_some_and(|s| s.primitive_type == PrimitiveType::Float32) {"uint32 converted to float32"} else {"32-bit integer word"},
                    "calculation_execution":"attribute-command-v1, descending root index, guarded updates require exact build validation",
                    "effect_scope":"local process, network authority and publication remain engine-managed",
                    "storage_writable":scalar_matches || common_integer,
                    "write_value_domain":if !scalar_matches && common_integer {"integer-common-range-0-to-2147483647"} else {"declared-scalar"},
                    "calculation_writable":scalar_matches || (common_integer && sign_independent),
                    "calculation_reason":if !scalar_matches && !sign_independent {Some("signedness-dependent program with conflicting resource/storage scalar types")} else {None},
                    "storage_reason":if components.is_empty() {Some("no definition GUID/root hash association")}
                        else if !scalar_matches && !common_integer {Some("KFC scalar type differs from reflected storage")}
                        else if components.len() != 1 {Some("ambiguous component ownership")} else {None}});
                if catalog.entries.insert(hash, descriptor).is_some() {
                    return Err(format!("duplicate attribute ID {hash:#010x}"));
                }
            }
        }
        Ok(catalog)
    }

    pub(crate) fn root_entries(&self, root: u32) -> Result<Vec<&Value>, String> {
        let mut entries: Vec<_> = self
            .entries
            .values()
            .filter(|e| e["root_hash"] == root)
            .collect();
        entries.sort_by_key(|e| e["index"].as_u64());
        if entries.is_empty() || entries.iter().enumerate().any(|(i, e)| e["index"] != i) {
            return Err("incomplete attribute root".into());
        }
        Ok(entries)
    }

    /// Engine recompute walks structure from last entry to first. ScaleToNewMax
    /// can update an earlier referenced value before that entry is evaluated.
    pub(crate) fn recalculate(
        &self,
        root: u32,
        values: &[u32],
    ) -> Result<(Vec<u32>, Vec<Value>), String> {
        let entries = self.root_entries(root)?;
        if entries[0]["calculation_encoding_verified"] != true {
            return Err("engine AttributeOps enum differs from the verified model".into());
        }
        if entries.len() != values.len() {
            return Err("attribute root value count mismatch".into());
        }
        let kind = Scalar::parse(
            entries[0]["calculation_scalar_type"]
                .as_str()
                .ok_or("unknown scalar")?,
        )?;
        let mut result = values.to_vec();
        let mut trace = Vec::new();
        for (i, entry) in entries.iter().enumerate().rev() {
            let words: Vec<u32> = serde_json::from_value(entry["calculation_words"].clone())
                .map_err(|e| e.to_string())?;
            if words.is_empty() {
                continue;
            }
            let before = result.clone();
            if let Some(value) = calculation::evaluate(kind, &mut result, &words)? {
                result[i] = value;
            }
            trace.push(json!({"name":entry["name"],"hash":entry["hash"],"index":i,
                "before_bits":before,"after_bits":result}));
        }
        Ok((result, trace))
    }
}

/// Engine Guid -> HashKey32 (client 0x1e4600, server 0x2f080).
/// Zero GUID is special; nonzero GUIDs use FNV-1a in on-disk byte order.
pub(crate) fn definition_root(bytes: &[u8; 16]) -> u32 {
    if *bytes == [0; 16] {
        return 0;
    }
    bytes.iter().fold(0x811c9dc5u32, |h, b| {
        (h ^ *b as u32).wrapping_mul(0x1000193)
    })
}

pub(crate) fn uint(value: &Value) -> Result<u32, String> {
    value
        .as_u64()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| "expected u32 attribute data".into())
}

// Observed in both signed attribute interpreters (client 0x1e1b00, server
// 0x2c310): SHR opcode,16; low 16-bit operand; Push increments the word cursor
// once more before loading its 32-bit literal. This decodes, never executes.
pub(crate) fn decode_program(
    words: &[Value],
    operations: &BTreeMap<u32, String>,
    ids: &[Value],
    names: &[Value],
) -> Value {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut complete = true;
    while offset < words.len() {
        let word = uint(&words[offset]).unwrap();
        let opcode = word >> 16;
        let operand = word & 0xffff;
        let name = operations.get(&opcode).map(String::as_str);
        let mut instruction = json!({"word_offset":offset,"word":word,"opcode":opcode,"operation":name,"operand":operand});
        offset += 1;
        if name == Some("Push") {
            if let Some(immediate) = words.get(offset) {
                instruction["immediate_bits"] = immediate.clone();
                offset += 1;
            } else {
                instruction["reason"] = json!("truncated Push literal");
                complete = false;
            }
        } else if matches!(name, Some("Load" | "LoadRef" | "Store")) {
            if let (Some(id), Some(name)) = (ids.get(operand as usize), names.get(operand as usize))
            {
                instruction["attribute_hash"] = id["value"].clone();
                instruction["attribute_name"] = name.clone();
            } else {
                instruction["reason"] = json!("attribute index outside root definition");
                complete = false;
            }
        }
        if name.is_none() {
            complete = false;
        }
        result.push(instruction);
    }
    json!({"encoding":"attribute-command-v1","decoded":complete,"native_callable":false,
        "evaluation_api":"runtime.ecs.evaluate_attributes","instructions":result})
}
