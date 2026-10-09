//! Read-only inspection through the same KFC parser used by the Lua API.
//! Usage: cargo run -p shroudforge-api --example inspect_building_inputs -- EXE OUTPUT.json [--backup]
use kfc::{
    container::{KFCFile, KFCReader, KFCReaderOptions},
    reflection::{LookupKey, TypeRegistry},
    resource::value::Value,
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "--backup")) {
        return Err("expected EXE OUTPUT.json [--backup]".into());
    }
    let exe = PathBuf::from(&args[0]);
    let directory = exe.parent().ok_or("missing game directory")?;
    let stem = exe
        .file_stem()
        .and_then(|v| v.to_str())
        .ok_or("invalid executable name")?;
    let registry = TypeRegistry::load_from_executable(&exe)?;
    let backup = args.len() == 3;
    let options = if backup {
        KFCReaderOptions {
            kfc_extension: "kfc.bak",
            resource_extension: "kfc_resources.bak",
            ..Default::default()
        }
    } else {
        KFCReaderOptions::default()
    };
    let kfc_path = exe.with_extension(options.kfc_extension);
    let file = KFCFile::from_path(&kfc_path, false)?;
    let version = KFCFile::get_version_tag(&kfc_path)?;
    let mut reader = KFCReader::new_with_options(directory, stem, options)?.into_cursor()?;
    let mut resources = Vec::new();
    for name in [
        "keen::ItemInfo",
        "keen::VoxelBlueprintItemRegistryResource",
        "keen::VoxelMaterialResolvedList",
        "keen::VoxelBlueprintMaterialPoolRegistryResource",
    ] {
        let Some(ty) = registry.get_by_name(LookupKey::Qualified(name)) else {
            continue;
        };
        for id in shroudforge_parser::kfc_format::resources_by_type(&file, &registry, name) {
            let mut bytes = Vec::new();
            if !reader.read_resource_into(&id, &mut bytes)? {
                return Err(format!("missing resource {id}").into());
            }
            let value = Value::from_bytes(&registry, ty, &bytes)?;
            let mut value = serde_json::to_value(value)?;
            if name == "keen::ItemInfo" {
                value = serde_json::json!({"itemId":value["itemId"],"debugName":value["debugName"],"equipment":value["equipment"]});
            }
            resources.push(serde_json::json!({"id":id.to_string(),"qualifiedType":name,"typeHash":ty.qualified_hash,"value":value}));
        }
    }
    let output = PathBuf::from(&args[1]);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = BufWriter::new(File::create(output)?);
    serde_json::to_writer_pretty(
        &mut output,
        &serde_json::json!({"version":version,"executable":exe,"container":kfc_path,
            "type_source":"fresh executable extraction; no type cache", "resources":resources}),
    )?;
    output.flush()?;
    println!(
        "Read {} building input resources; no game files or memory changed",
        resources.len()
    );
    Ok(())
}
