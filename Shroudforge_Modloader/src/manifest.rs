use serde::Deserialize;
use shroudforge_package::ModManifest;

#[derive(Debug, Clone, Deserialize)]
pub struct PackageManifest {
    #[serde(flatten)]
    pub package: ModManifest,
    /// Required version of the single ShroudForge API contract.
    pub api: Option<semver::VersionReq>,
    #[serde(default = "default_target")]
    pub target: String,
}

impl PackageManifest {
    pub fn lua_entrypoint(&self) -> &'static str {
        "src/mod.lua"
    }
}

fn default_target() -> String {
    "both".into()
}

pub fn validate(manifest: &PackageManifest) -> Result<(), String> {
    shroudforge_package::validate_manifest(&manifest.package)?;
    if manifest.package.name.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    if !matches!(manifest.target.as_str(), "client" | "server" | "both") {
        return Err("target must be 'client', 'server' or 'both'".into());
    }
    if manifest.api.as_ref().is_some_and(|required| {
        !required.matches(&semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap())
    }) {
        return Err(format!(
            "required ShroudForge API {} is unavailable",
            manifest.api.as_ref().unwrap()
        ));
    }
    Ok(())
}

pub use shroudforge_package::{Capability, Dependency};

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    fn parse(value: &str) -> PackageManifest {
        serde_json::from_str(value).unwrap()
    }

    #[test]
    fn every_package_uses_the_fixed_lua_entrypoint() {
        let manifest = parse(
            r#"{
            "id":"sf-unlimited-flight", "name":"SF Unlimited Flight", "version":"1.0.0",
            "capabilities":["runtime"]
        }"#,
        );
        assert_eq!(manifest.lua_entrypoint(), "src/mod.lua");
        assert!(validate(&manifest).is_ok());
    }

    #[test]
    fn lua_runtime_contract_has_no_machine_patch_surface() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let definition =
            fs::read_to_string(root.join("Shroudforge_API/definitions/runtime.lua")).unwrap();
        assert_no_machine_patch_primitives("runtime definition", &definition);
    }

    #[test]
    fn lua_mods_have_no_machine_patch_primitives() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for directory in fs::read_dir(root.join("mods")).unwrap().flatten() {
            let lua = directory.path().join("src/mod.lua");
            if lua.is_file() {
                let source = fs::read_to_string(&lua).unwrap();
                assert_no_machine_patch_primitives(&lua.display().to_string(), &source);
            }
        }
    }

    #[test]
    fn every_mod_has_valid_lua_and_matching_capabilities() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lua = mlua::Lua::new();
        for directory in fs::read_dir(root.join("mods")).unwrap().flatten() {
            let source_path = directory.path().join("src/mod.lua");
            let manifest_path = directory.path().join("mod.json");
            if !source_path.is_file() || !manifest_path.is_file() {
                continue;
            }
            let source = fs::read_to_string(&source_path).unwrap();
            lua.load(&source)
                .set_name(source_path.display().to_string())
                .into_function()
                .unwrap_or_else(|error| panic!("{}: {error}", source_path.display()));

            let manifest = PackageManifest {
                package: shroudforge_package::config::read_manifest_path(root, &directory.path())
                    .unwrap(),
                api: None,
                target: "both".into(),
            };
            assert!(validate(&manifest).is_ok(), "{}", manifest_path.display());
            if source.contains("runtime.ecs.") {
                assert!(
                    manifest.package.capabilities.contains(&Capability::Runtime),
                    "{} uses runtime.ecs without the runtime capability",
                    manifest.package.id
                );
            }
            if source.contains("game.assets.") {
                assert!(
                    manifest
                        .package
                        .capabilities
                        .contains(&Capability::AssetsWrite),
                    "{} uses game.assets without the assets-write capability",
                    manifest.package.id
                );
            }
        }
    }

    fn assert_no_machine_patch_primitives(label: &str, source: &str) {
        for forbidden in [
            "signature",
            "payload",
            "memory.write",
            "create_patch",
            "set_patch_enabled",
            "release_patch",
        ] {
            assert!(
                !source.contains(forbidden),
                "{label} contains forbidden primitive: {forbidden}"
            );
        }
    }
}
