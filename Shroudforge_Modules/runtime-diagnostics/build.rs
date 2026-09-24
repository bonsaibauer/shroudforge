use std::{env, fs, path::PathBuf};

const TOOLS: &[(&str, &str)] = &[
    ("dump-live-components", "DUMP_LIVE_COMPONENTS"),
    ("inspect-component-metadata", "INSPECT_COMPONENT_METADATA"),
    ("live-entity-manager-sample", "LIVE_ENTITY_MANAGER_SAMPLE"),
    ("live-entity-managers", "LIVE_ENTITY_MANAGERS"),
    ("live-type-references", "LIVE_TYPE_REFERENCES"),
];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let root = manifest.join("../..");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("output directory"));
    let native = root.join("build/native-diagnostics/Release");
    let mut generated = String::new();

    for (tool, constant) in TOOLS {
        let source = native.join(format!("{tool}.exe"));
        let destination = out.join(format!("{tool}.exe"));
        println!("cargo:rerun-if-changed={}", source.display());
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            if !source.is_file() {
                panic!("native diagnostic tool is missing; build build/native-diagnostics first: {}", source.display());
            }
            fs::copy(&source, &destination).expect("copy native diagnostic tool into embedded resources");
        } else {
            fs::write(&destination, []).expect("create non-Windows placeholder resource");
        }
        generated.push_str(&format!(
            "pub static {constant}: &[u8] = include_bytes!({:?});\n",
            destination.to_string_lossy()
        ));
    }

    fs::write(out.join("embedded_tools.rs"), generated).expect("write embedded tool index");
    println!("cargo:rerun-if-changed=build.rs");
}
