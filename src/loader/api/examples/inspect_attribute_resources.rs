//! Read-only inspection through the same KFC parser used by the Lua API.
//! Usage: cargo run -p shroudforge-api --example inspect_attribute_resources -- EXE OUTPUT.json
use std::{fs::File, path::PathBuf};
use kfc::{container::{KFCFile,KFCReader}, reflection::{LookupKey,TypeRegistry}, resource::value::Value};

fn main() -> Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_> = std::env::args_os().skip(1).collect();
    if args.len()!=2 { return Err("expected EXE OUTPUT.json".into()); }
    let exe=PathBuf::from(&args[0]);
    let directory=exe.parent().ok_or("missing game directory")?;
    let stem=exe.file_stem().and_then(|v|v.to_str()).ok_or("invalid executable name")?;
    let registry=TypeRegistry::load_from_executable(&exe)?;
    let file=KFCFile::from_path(exe.with_extension("kfc"),false)?;
    let mut reader=KFCReader::new(directory,stem)?.into_cursor()?;
    let mut resources=Vec::new();
    for name in ["keen::AttributeContainerResource","keen::BaseAttributeResource","keen::BalancingTable"] {
        let Some(ty)=registry.get_by_name(LookupKey::Qualified(name)) else { continue; };
        for id in shroudforge_parser::kfc_format::resources_by_type(&file,&registry,name) {
            let mut bytes=Vec::new();
            if !reader.read_resource_into(&id,&mut bytes)? { return Err(format!("missing resource {id}").into()); }
            let value=Value::from_bytes(&registry,ty,&bytes)?;
            resources.push(serde_json::json!({"id":id.to_string(),"qualifiedType":name,"typeHash":ty.qualified_hash,"value":value}));
        }
    }
    let output=PathBuf::from(&args[1]);
    if let Some(parent)=output.parent() { std::fs::create_dir_all(parent)?; }
    serde_json::to_writer_pretty(File::create(output)?,&serde_json::json!({"version":registry.version,"executable":exe,"resources":resources}))?;
    println!("Read {} attribute/balancing resources; no game files or memory changed",resources.len());
    Ok(())
}
