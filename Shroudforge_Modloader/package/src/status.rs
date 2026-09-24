use serde_json::{Value,json};
use std::{fs,path::Path};

pub fn configuration(root: &Path, server: bool, api: &str) -> Value {
    let mut errors=Vec::new();
    let mut checks=Vec::new();
    let mut mod_states=serde_json::Map::new();
    let runtime=crate::config::read_document(root,"mod-status").ok();
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let runtime_fresh=runtime.as_ref().is_some_and(|value|value["running"]==true && value["updatedAt"].as_u64().is_some_and(|timestamp|timestamp<=now && now-timestamp<=5));
    if let Ok(events)=crate::config::read_document(root,"events-state") {
        if let Some(events)=events.as_object(){for (id,value) in events {if let Err(error)=crate::news::validate_event(root,value){errors.push(format!("event {id}: {error}"));}}}
    }
    for result in [crate::config::read_loader(root).map(|_|()),crate::news::read(root).map(|_|()),crate::news::read_ids(root).map(|_|())] {
        if let Err(error)=result { errors.push(error); }
    }
    let (api_state,api_detail)=match runtime.as_ref().filter(|_|runtime_fresh).and_then(|value|value["apiVersion"].as_str()) {
        Some(version) if version==api => ("ok",format!("The running ShroudForge runtime process reports Lua API {version}.")),
        Some(version) => ("warning",format!("Runtime reports API {version}; expected {api}.")),
        None => ("neutral","No current API initialization report from the game process.".into()),
    };
    checks.push(json!({"id":"api","group":"api","state":api_state,"detail":api_detail}));
    let mut assets=json!({"state":"unknown","detail":"Preparation status has not been checked."});
    if let Ok(entries)=fs::read_dir(root.join("mods")) {
        for entry in entries.flatten() {
            let path=entry.path();
            if !path.is_dir() && path.extension().is_none_or(|extension|extension!="zip") { continue; }
            if let Err(error)=crate::config::read_manifest_path(root,&path) { errors.push(format!("{}: {error}",path.display())); }
        }
    }
    match root.to_str().ok_or("installation path is not UTF-8").and_then(|path|crate::ModEnvironment::load(path).map_err(|_|"mod discovery failed")) {
        Ok(env)=> {
            let (plan,failures)=env.plan_report(server,api);
            for item in env.mod_registry().values() {
                let manifest=item.info();
                let fingerprint=crate::config::revision(&serde_json::to_vec(manifest).unwrap_or_default());
                let state=if runtime_fresh {
                    let runtime=runtime.as_ref().unwrap();
                    if let Some(reason)=runtime["errors"].get(&manifest.id) {json!({"state":"failed","detail":reason})}
                    else if runtime["loaded"].get(&manifest.id).is_some_and(|loaded| loaded!=&fingerprint) {
                        json!({"state":"restart-required","detail":"Configuration saved; the running process still uses the previous settings."})
                    }else if runtime["active"].as_array().is_some_and(|active|active.iter().any(|id|id==&manifest.id)) {
                        json!({"state":"active","detail":"Lua lifecycle is active in the target process and the applied settings match."})
                    }else if manifest.enabled {json!({"state":"not-running","detail":"Activation is saved; the mod is not active in the running process."})}
                    else {json!({"state":"disabled","detail":"The mod is disabled."})}
                }else if manifest.enabled {json!({"state":"unconfirmed","detail":"Activation is saved; no current runtime report is available."})}
                else{json!({"state":"disabled","detail":"The mod is disabled."})};
                mod_states.insert(manifest.id.clone(),state);
            }
            checks.push(json!({"id":"mods","group":"mods","state":if failures.is_empty(){"ok"}else{"warning"},"detail":if failures.is_empty(){format!("{} mods satisfy activation, target, API, and dependency requirements. Execution status is reported separately.",plan.len())}else{failures.join("; ")}}));
            let needs_prepare=plan.iter().any(|item|item.info().capabilities.contains(&crate::Capability::AssetsWrite));
            let existing=crate::config::read_document(root,"applied").is_ok();
            if needs_prepare || existing {
                assets=match crate::prepared::fingerprint(&env,server,api) {
                    Ok(fingerprint)=> if crate::prepared::matches(root,&fingerprint) {
                        json!({"state":"applied","detail":"Preparation record matches the game and current mod configuration."})
                    }else{json!({"state":"prepare-required","detail":"The startup asset pass will apply this configuration on the next game start. Use `shroudforge launch` if the game has already passed its early asset-loading window."})},
                    Err(error)=>json!({"state":"unknown","detail":error}),
                };
            } else { assets=json!({"state":"not-required","detail":"No enabled asset mods and no previous preparation record."}); }
        }
        Err(error)=>errors.push(error.into()),
    }
    let executable=root.join(if server{"enshrouded_server.exe"}else{"enshrouded.exe"});
    checks.push(json!({"id":"game","group":"game","state":if executable.is_file(){"ok"}else{"warning"},"detail":if executable.is_file(){"Game executable found; build and hook checks run in the target process."}else{"Game executable is missing from this installation."}}));
    let parser=crate::config::read_document(root,"parser-status").ok();
    let parser_current=parser.as_ref().zip(fs::metadata(&executable).ok()).is_some_and(|(status,metadata)|{
        let modified=metadata.modified().ok().and_then(|value|value.duration_since(std::time::UNIX_EPOCH).ok()).map(|value|value.as_secs());
        status["target"]==if server{"enshrouded_server"}else{"enshrouded"} && status["gameFileSize"].as_u64()==Some(metadata.len()) && status["gameFileModified"].as_u64()==modified
    });
    let (parser_state,parser_detail)=match parser {
        Some(_) if !parser_current => ("neutral","Parser result belongs to an older game binary; prepare/launch will refresh it.".into()),
        Some(value) if value["status"]=="parsed" => ("ok",format!("KFC Parser successfully processed build {}.",value["target"])),
        Some(value) => ("warning",format!("Parser reported an error: {}",value["error"].as_str().unwrap_or("unknown error"))),
        None => ("neutral","No parser result has been saved by prepare/launch.".into()),
    };
    checks.push(json!({"id":"parser","group":"parser","state":parser_state,"detail":parser_detail}));
    let runtime_check = runtime.as_ref().filter(|value|runtime_fresh).map(|value|&value["runtimeProvider"]);
    let (runtime_state,runtime_detail)=match runtime_check {
        Some(value) if value["available"]==true => (if value["ready"]==true {"ok"} else {"warning"},format!("KFC Runtime ABI {}; initialized={}; ready={}; writable={}; {}",value["abi"],value["initialized"],value["ready"],value["writable"],value["detail"].as_str().unwrap_or("provider status unavailable"))),
        Some(value) => ("warning",format!("KFC Runtime provider unavailable (reported ABI {}): {}",value["abi"],value["reason"].as_str().unwrap_or("provider or ABI unavailable"))),
        None => ("neutral","Waiting for a fresh status from the game process.".into()),
    };
    checks.push(json!({"id":"runtime","group":"game","state":runtime_state,"detail":runtime_detail}));
    let updates=match crate::config::read_document(root,"state") {Ok(value)=>value,Err(error) if error.contains("has no updates state")=>Value::Null,Err(error)=>{errors.push(error);Value::Null}};
    if let Err(error)=crate::config::read_document(root,"applied") {if !error.contains("has no assets state"){errors.push(error);}}
    json!({"errors":errors,"checks":checks,"assets":assets,"lastUpdate":updates,"modStates":mod_states})
}
