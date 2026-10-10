//! Build-scoped provisional bindings for every evidenced native entry point.
use super::{
    buffer_transform::{self, Plan},
    loader::runtime_provider,
    registry::json_to_lua,
};
use mlua::{Lua, Table, Value as LuaValue};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Catalog {
    base: Option<&'static Value>,
    indices: BTreeMap<u32, usize>,
    owners: BTreeMap<u32, Vec<Value>>,
    plans: BTreeMap<u32, Option<Plan>>,
    registry: Value,
    last_refresh: Option<Instant>,
    registry_reason: Option<String>,
}

impl Catalog {
    fn refresh(&mut self) -> Result<(), String> {
        if self.base.is_none() {
            let base = runtime_provider::functions()?;
            let entries = base["entries"]
                .as_array()
                .ok_or("invalid function inventory")?;
            for (i, entry) in entries.iter().enumerate() {
                let rva = entry["rva"]
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or("invalid function RVA")?;
                if self.indices.insert(rva, i).is_some() {
                    return Err("duplicate function RVA".into());
                }
            }
            self.base = Some(base);
        }
        if self
            .last_refresh
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return Ok(());
        }
        self.last_refresh = Some(Instant::now());
        match runtime_provider::registry() {
            Ok(registry) if registry["available"] == true => {
                self.registry_reason = None;
                if registry != self.registry {
                    self.set_registry(registry);
                }
            }
            other => {
                self.registry_reason = Some(match other {
                    Err(reason) => reason,
                    _ => "live component registration is not ready".into(),
                });
                if !self.owners.is_empty() {
                    self.set_registry(Value::Null);
                }
            }
        }
        Ok(())
    }

    fn set_registry(&mut self, registry: Value) {
        self.owners.clear();
        self.plans.clear();
        if let Some(entries) = registry["entries"].as_array() {
            for entry in entries {
                if let Some(callbacks) = entry["callbacks"].as_array() {
                    for callback in callbacks {
                        let Some(rva) = callback["function_rva"]
                            .as_u64()
                            .and_then(|v| u32::try_from(v).ok())
                        else {
                            continue;
                        };
                        self.owners.entry(rva).or_default().push(json!({
                            "qualified_name":entry["qualified_name"], "qualified_hash":entry["qualified_hash"],
                            "registration_index":entry["index"], "runtime_type":entry["runtime_type"],
                            "template_type":entry["template_type"], "origin":callback["origin"], "slot_offset":callback["slot_offset"]
                        }));
                        let plan = callback["code_hex"]
                            .as_str()
                            .and_then(buffer_transform::unhex)
                            .and_then(|code| buffer_transform::prove(&code));
                        self.plans.insert(rva, plan);
                    }
                }
            }
        }
        self.registry = registry;
    }

    fn keys(&self) -> Vec<u32> {
        self.indices
            .keys()
            .chain(self.owners.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn selector(&self, selector: LuaValue) -> Result<u32, String> {
        let rva = match selector {
            LuaValue::Integer(value) => {
                u32::try_from(value).map_err(|_| "RVA outside u32".to_owned())?
            }
            LuaValue::String(value) => {
                let text = value.to_str().map_err(|e| e.to_string())?;
                let text = if let Some((sha, name)) = text.split_once('/') {
                    if Some(sha) != self.base.unwrap()["image"]["sha256"].as_str() {
                        return Err("binding belongs to a different executable".into());
                    }
                    name
                } else {
                    text.as_ref()
                };
                if let Some(rva) = text.strip_prefix("unclear_") {
                    if rva.len() != 8 {
                        return Err("expected unclear_<eight hexadecimal RVA digits>".into());
                    }
                    u32::from_str_radix(rva, 16)
                        .map_err(|_| "invalid provisional binding name".to_owned())?
                } else if let Some(rva) = text.strip_prefix("native:") {
                    rva.parse::<u32>()
                        .map_err(|_| "invalid native binding ID".to_owned())?
                } else {
                    let matches: Vec<u32> = self
                        .indices
                        .iter()
                        .filter_map(|(rva, index)| {
                            (self.base.unwrap()["entries"][*index]["engine_descriptors"]
                                .as_array()
                                .is_some_and(|descriptors| {
                                    descriptors.iter().any(|v| v["name"] == text)
                                })
                                || self.base.unwrap()["entries"][*index]["operations"]
                                    .as_array()
                                    .is_some_and(|operations| {
                                        operations.iter().any(|v| v["id"] == text)
                                    }))
                            .then_some(*rva)
                        })
                        .collect();
                    match matches.as_slice() {
                        [rva] => *rva,
                        [] => return Err(format!("engine function name not found: {text}")),
                        _ => {
                            return Err(format!(
                                "Ambiguous engine function name: {text}. Select a build-scoped RVA"
                            ));
                        }
                    }
                }
            }
            _ => return Err("expected binding name or integer RVA".into()),
        };
        if !self.indices.contains_key(&rva) && !self.owners.contains_key(&rva) {
            return Err("no evidenced entry point at this RVA".into());
        }
        Ok(rva)
    }

    fn describe(&mut self, rva: u32, analyze: bool) -> Value {
        if analyze && !self.plans.contains_key(&rva) {
            self.plans.insert(
                rva,
                runtime_provider::function_code(rva)
                    .and_then(|code| buffer_transform::prove(&code)),
            );
        }
        let mut entry = self.indices.get(&rva).map(|i| self.base.unwrap()["entries"][*i].clone())
            .unwrap_or_else(|| json!({"id":format!("native:{rva}"),"rva":rva,"ranges":[],"code_bytes":null,"chain_resolved":null}));
        let names: std::collections::BTreeSet<_> = entry["engine_descriptors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["name"].as_str().map(str::to_owned))
            .collect();
        let name = if names.len() == 1 {
            names.first().unwrap().clone()
        } else {
            format!("unclear_{rva:08x}")
        };
        entry["name_provisional"] = json!(names.len() != 1);
        entry["name"] = json!(name);
        entry["key"] = json!(format!(
            "{}/{}",
            self.base.unwrap()["image"]["sha256"]
                .as_str()
                .unwrap_or("unknown-image"),
            name
        ));
        entry["owners"] = json!(self.owners.get(&rva).cloned().unwrap_or_default());
        entry["provisional"] = json!(true);
        entry["native_callable"] = json!(false);
        entry["signature"] = json!({"native_arguments":null,"native_return":null,"calling_convention":"windows-x64","complete":false});
        entry["validation"] = json!({"address":if self.owners.contains_key(&rva) { "live-engine-registration" } else { entry["address_evidence"].as_str().unwrap_or("unwind-entry") },
            "type_ownership":self.owners.contains_key(&rva),"native_signature":false,"engine_context":false,
            "gameplay_effects":false,"buffer_adapter_checked":self.plans.contains_key(&rva)});
        if let Some(Some(plan)) = self.plans.get(&rva) {
            entry["callable"] = json!(true);
            entry["execution"] = json!("owned-buffer-copy-interpreter");
            entry["buffer_transform"] = serde_json::to_value(plan).unwrap();
            entry["signature"]["adapter_arguments"] = json!(["source_bytes", "destination_bytes"]);
            entry["signature"]["adapter_return"] =
                json!("destination_bytes, reason, native_constant_result");
            entry["validation"]["owned_buffer_effects"] = json!(true);
            entry["reason"] = Value::Null;
        } else {
            entry["callable"] = json!(false);
            entry["execution"] = Value::Null;
            entry["reason"] = json!(
                "Native arguments, engine context and effects are unresolved. No verified adapter"
            );
        }
        entry
    }
}

fn binding(lua: &Lua, descriptor: &Value, plan: Option<Plan>) -> mlua::Result<Table> {
    let LuaValue::Table(table) = json_to_lua(lua, descriptor)? else {
        unreachable!()
    };
    let reason = descriptor["reason"]
        .as_str()
        .unwrap_or("no verified adapter")
        .to_owned();
    table.raw_set(
        "call",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let Some(plan) = &plan else {
                return Ok((None, Some(reason.clone()), None));
            };
            if args.len() != 2 {
                return Ok((
                    None,
                    Some("call expects source_bytes and destination_bytes".into()),
                    None,
                ));
            }
            let (Some(LuaValue::String(source)), Some(LuaValue::String(destination))) =
                (args.front(), args.get(1))
            else {
                return Ok((
                    None,
                    Some("call expects two owned Lua byte strings".into()),
                    None,
                ));
            };
            match plan.apply(source.as_bytes().as_ref(), destination.as_bytes().as_ref()) {
                Ok(bytes) => Ok((
                    Some(lua.create_string(bytes)?),
                    None,
                    plan.native_constant_result,
                )),
                Err(reason) => Ok((None, Some(reason), None)),
            }
        })?,
    )?;
    Ok(table)
}

pub(crate) fn attach(lua: &Lua, table: &Table, patch: &Table) -> mlua::Result<()> {
    let catalog = Rc::new(RefCell::new(Catalog::default()));
    let list_catalog = catalog.clone();
    let list = lua.create_function(move |lua, (offset, limit): (Option<usize>, Option<usize>)| {
        let mut catalog = list_catalog.borrow_mut();
        if let Err(reason) = catalog.refresh() { return Ok((None, Some(reason))); }
        let keys = catalog.keys();
        let offset = offset.unwrap_or(0).min(keys.len());
        let end = offset.saturating_add(limit.unwrap_or(256).clamp(1, 4096)).min(keys.len());
        let mut page = json!({"schema_version":1,"image":catalog.base.unwrap()["image"],
            "count":keys.len(),"unwind_count":catalog.base.unwrap()["unwind_count"],"registered_callback_count":catalog.owners.len(),
            "pointer_target_count":catalog.base.unwrap()["pointer_target_count"],"unreadable_pointer_pages":catalog.base.unwrap()["unreadable_pointer_pages"],
            "range_count":catalog.base.unwrap()["range_count"],"source":"unwind-code-pointers-and-live-registration",
            "complete_engine_api":false,"registry_reason":catalog.registry_reason,
            "limitations":["unreferenced leaf functions and inlined code can be absent", "code pointers can target internal labels. A function boundary is not implied", "provisional names are scoped to the executable SHA256", "native ABI and gameplay effects remain unresolved unless explicitly validated"],
            "offset":offset,"next_offset":(end < keys.len()).then_some(end)});
        page["entries"] = keys[offset..end].iter().map(|rva| catalog.describe(*rva, false)).collect();
        let LuaValue::Table(page) = json_to_lua(lua, &page)? else { unreachable!() };
        Ok((Some(page), None))
    })?;
    table.raw_set("list", list.clone())?;
    table.raw_set("list_native", list)?;
    let modifiers_catalog = catalog.clone();
    let checked_available = patch.raw_get::<mlua::Function>("available")?;
    let checked_set = patch.raw_get::<mlua::Function>("set_enabled")?;
    table.raw_set(
        "bind_modifier",
        lua.create_function(move |lua, id: String| {
            let mut catalog = modifiers_catalog.borrow_mut();
            if let Err(reason) = catalog.refresh() {
                return Ok((None, Some(reason)));
            }
            let matches: Vec<_> = catalog.base.unwrap()["entries"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|entry| {
                    entry["modifiers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|m| m["id"] == id)
                        .map(|m| (entry["rva"].as_u64().unwrap() as u32, m.clone()))
                })
                .collect();
            let [(rva, modifier)] = matches.as_slice() else {
                return Ok((
                    None,
                    Some(format!(
                        "modifier has no unique verified function association: {id}"
                    )),
                ));
            };
            if let Some(requirements) = modifier["attribute_requirements"].as_array() {
                let state = lua.app_data_ref::<super::AppState>().unwrap();
                let attributes = match state.attribute_catalog() {
                    Ok(c) => c,
                    Err(reason) => return Ok((None, Some(reason))),
                };
                for required in requirements {
                    let selector = LuaValue::Integer(required["hash"].as_i64().unwrap());
                    let valid = attributes.get(&selector).is_ok_and(|actual| {
                        ["name", "index", "root_hash"]
                            .iter()
                            .all(|key| actual[*key] == required[*key])
                            && actual["storage_writable"] == true
                    });
                    if !valid {
                        return Ok((
                            None,
                            Some(format!(
                                "KFC attribute definition no longer proves modifier {id}: {}",
                                required["name"]
                            )),
                        ));
                    }
                }
            }
            let operation = modifier["backend_operation"].as_str().unwrap().to_owned();
            let descriptor = catalog.describe(*rva, false);
            let LuaValue::Table(bound) = json_to_lua(lua, modifier)? else {
                unreachable!()
            };
            bound.raw_set("owner", json_to_lua(lua, &descriptor)?)?;
            let available = checked_available.clone();
            let available_operation = operation.clone();
            bound.raw_set(
                "available",
                lua.create_function(move |_, ()| {
                    available.call::<bool>(available_operation.clone())
                })?,
            )?;
            let set = checked_set.clone();
            bound.raw_set(
                "set_enabled",
                lua.create_function(move |_, enabled: bool| {
                    set.call::<(bool, Option<String>)>((operation.clone(), enabled))
                })?,
            )?;
            Ok((Some(bound), None))
        })?,
    )?;
    let get = lua.create_function(move |lua, selector: LuaValue| {
        let mut catalog = catalog.borrow_mut();
        if let Err(reason) = catalog.refresh() {
            return Ok((None, Some(reason)));
        }
        let rva = match catalog.selector(selector) {
            Ok(rva) => rva,
            Err(reason) => return Ok((None, Some(reason))),
        };
        let descriptor = catalog.describe(rva, true);
        let plan = catalog.plans.get(&rva).cloned().flatten();
        Ok((Some(binding(lua, &descriptor, plan)?), None))
    })?;
    table.raw_set("get", get.clone())?;
    table.raw_set("get_native", get)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_engine_names_are_resolved_without_inventing_helper_names() {
        let base = Box::leak(Box::new(json!({"image":{"sha256":"fixture"},"entries":[
            {"rva":16,"engine_descriptors":[{"name":"network_player_attributes"}]},
            {"rva":32,"engine_descriptors":[{"name":"shared_name"}]},
            {"rva":48,"engine_descriptors":[{"name":"shared_name"}]}
        ]})));
        let mut catalog = Catalog {
            base: Some(base),
            ..Default::default()
        };
        for (i, rva) in [16, 32, 48].into_iter().enumerate() {
            catalog.indices.insert(rva, i);
        }
        let lua = Lua::new();
        assert_eq!(
            catalog
                .selector(LuaValue::String(
                    lua.create_string("network_player_attributes").unwrap()
                ))
                .unwrap(),
            16
        );
        assert!(
            catalog
                .selector(LuaValue::String(lua.create_string("shared_name").unwrap()))
                .is_err()
        );
        let descriptor = catalog.describe(16, false);
        assert_eq!(descriptor["name"], "network_player_attributes");
        assert_eq!(descriptor["name_provisional"], false);
        assert_eq!(descriptor["native_callable"], false);
    }

    #[test]
    fn bundled_mods_bind_profile_modifiers_and_restore_on_unload() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for target in ["client", "server"] {
            let directory = root.join(format!("src/loader/runtime/profiles/enshrouded/{target}"));
            let profile = std::fs::read_dir(directory)
                .unwrap()
                .find_map(|entry| {
                    let path = entry.ok()?.path();
                    (path.extension()?.to_str()? == "json").then(|| {
                        serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap()
                    })
                })
                .unwrap();
            for patch in profile["runtimePatches"].as_object().unwrap().values() {
                let lua = Lua::new();
                lua.globals()
                    .set(
                        "expected_modifier",
                        patch["modifier"]["id"].as_str().unwrap(),
                    )
                    .unwrap();
                lua.load(r#"
                    calls = {}; state = false
                    runtime = {require=function() end, report_effect=function() end,
                        functions={bind_modifier=function(id)
                            assert(id == expected_modifier)
                            return {available=function() return true end,
                                set_enabled=function(value) state=value; calls[#calls+1]=value; return true end}
                        end}}
                    shroudforge={settings={get=function() return true end},
                        log={error=function(message) error(message) end,info=function() end},
                        ui={on_action=function() end}}
                "#).exec().unwrap();
                let alias = profile["runtimePatches"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .find(|(_, v)| *v == patch)
                    .unwrap()
                    .0;
                let slug = format!(
                    "sf-{}",
                    alias
                        .strip_prefix("runtime.patch.")
                        .unwrap()
                        .replace('_', "-")
                );
                let source =
                    std::fs::read_to_string(root.join(format!("mods/{slug}/src/mod.lua"))).unwrap();
                let module: Table = lua.load(source).eval().unwrap();
                module
                    .get::<mlua::Function>("on_load")
                    .unwrap()
                    .call::<()>(())
                    .unwrap();
                assert!(
                    lua.globals().get::<bool>("state").unwrap(),
                    "{target}/{slug}"
                );
                if let Some(update) = module.get::<Option<mlua::Function>>("on_update").unwrap() {
                    update.call::<()>(()).unwrap();
                }
                module
                    .get::<mlua::Function>("on_unload")
                    .unwrap()
                    .call::<()>(())
                    .unwrap();
                assert!(
                    !lua.globals().get::<bool>("state").unwrap(),
                    "{target}/{slug}"
                );
                assert!(lua.globals().get::<Table>("calls").unwrap().len().unwrap() >= 2);
            }
        }
    }

    #[test]
    fn blueprint_mod_resolves_original_query_action_instead_of_fixed_id() {
        let lua = Lua::new();
        lua.load(r#"
            loader={features={patch=true}}
            shroudforge={log={info=function() end}}
            recipes={data={recipes={{knowledgeRequirement={knowledgeOrQueryId={value=999}}}}}}
            game={types={get=function(name) return name end},assets={get_resources_by_type=function(name)
                if name=='keen::GameKnowledgeQueryResourceDb' then
                    return {{data={queries={{name='Unlock_Flame_Altar_PK',actions={{name='NPC_Flame_Hint01',query={knowledgeOrQueryId={value=123456}}}}}}}}}
                end
                return {recipes}
            end}}
        "#).exec().unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../mods/sf-unlock-blueprints/src/mod.lua");
        lua.load(std::fs::read_to_string(path).unwrap())
            .exec()
            .unwrap();
        lua.load("assert(recipes.data.recipes[1].knowledgeRequirement.knowledgeOrQueryId.value == 123456)").exec().unwrap();
    }
    #[test]
    #[ignore = "set SHROUDFORGE_TEST_FUNCTIONS to a production-reader verification JSON"]
    fn production_callback_binding_corpus() {
        let path = std::env::var("SHROUDFORGE_TEST_FUNCTIONS").unwrap();
        let registry: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let base: Value =
            serde_json::from_slice(&std::fs::read(format!("{path}.functions.json")).unwrap())
                .unwrap();
        let base = Box::leak(Box::new(base));
        let mut catalog = Catalog {
            base: Some(base),
            ..Default::default()
        };
        for (i, entry) in base["entries"].as_array().unwrap().iter().enumerate() {
            catalog
                .indices
                .insert(entry["rva"].as_u64().unwrap() as u32, i);
        }
        catalog.set_registry(registry.clone());
        let mut adapters = Vec::new();
        let lua = Lua::new();
        for rva in catalog.owners.keys().copied().collect::<Vec<_>>() {
            let descriptor = catalog.describe(rva, false);
            let plan = catalog.plans.get(&rva).cloned().flatten();
            if let Some(plan) = plan {
                let bound = binding(&lua, &descriptor, Some(plan.clone())).unwrap();
                let source: Vec<u8> = (0..plan.source_minimum).map(|i| (i * 37) as u8).collect();
                let destination = vec![0xa5; plan.destination_minimum + 16];
                let expected = plan.apply(&source, &destination).unwrap();
                let (actual, reason, result): (mlua::String, Option<String>, Option<u32>) = bound
                    .get::<mlua::Function>("call")
                    .unwrap()
                    .call((
                        lua.create_string(&source).unwrap(),
                        lua.create_string(&destination).unwrap(),
                    ))
                    .unwrap();
                assert!(reason.is_none());
                assert_eq!(actual.as_bytes().as_ref(), expected);
                assert_eq!(result, plan.native_constant_result);
                adapters.push(descriptor);
            }
        }
        assert!(
            !adapters.is_empty(),
            "no independently observed callback adapter accepted"
        );
        let artifact = json!({"schema_version":1,"candidate_count":catalog.keys().len(),"callback_count":catalog.owners.len(),"adapters":adapters});
        std::fs::write(
            format!("{path}.adapters.json"),
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();
        println!(
            "{} candidates, {} distinct registered callbacks, {} proven owned-buffer adapters",
            catalog.keys().len(),
            catalog.owners.len(),
            adapters.len()
        );
        for adapter in &adapters {
            println!(
                "{}: {}",
                adapter["name"], adapter["owners"][0]["qualified_name"]
            );
        }
    }
    #[test]
    fn includes_leaf_callbacks_merges_owners_and_returns_partial_bindings() {
        let base = Box::leak(Box::new(
            json!({"image":{"sha256":"fixture"},"entries":[{"rva":16,"id":"native:16"}]}),
        ));
        let mut catalog = Catalog {
            base: Some(base),
            ..Default::default()
        };
        catalog.indices.insert(16, 0);
        catalog.set_registry(
            json!({"entries":[{"qualified_name":"keen::ecs::Example", "callbacks":[
            {"function_rva":16,"slot_offset":0}, {"function_rva":32,"slot_offset":8}]}]}),
        );
        assert_eq!(catalog.keys(), vec![16, 32]);
        let lua = Lua::new();
        assert_eq!(
            catalog
                .selector(LuaValue::String(
                    lua.create_string("fixture/unclear_00000020").unwrap()
                ))
                .unwrap(),
            32
        );
        assert!(
            catalog
                .selector(LuaValue::String(
                    lua.create_string("other/unclear_00000020").unwrap()
                ))
                .is_err()
        );
        assert!(catalog.selector(LuaValue::Integer(33)).is_err());
        let descriptor = catalog.describe(32, false);
        assert_eq!(
            descriptor["owners"][0]["qualified_name"],
            "keen::ecs::Example"
        );
        let bound = binding(&lua, &descriptor, None).unwrap();
        let result: (Option<String>, String, Option<u32>) = bound
            .get::<mlua::Function>("call")
            .unwrap()
            .call(())
            .unwrap();
        assert!(result.0.is_none());
        assert!(result.1.contains("unresolved"));
        bound.set("callable", true).unwrap(); // Descriptor edits cannot change the captured dispatcher.
        let result: (Option<String>, String, Option<u32>) = bound
            .get::<mlua::Function>("call")
            .unwrap()
            .call(())
            .unwrap();
        assert!(result.0.is_none());
    }
}
