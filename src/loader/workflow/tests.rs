use shroudforge_package::{Capability, config::read_manifest_path};
use std::{fs, path::Path};

#[test]
fn every_mod_has_valid_lua_and_declares_its_used_capabilities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let lua = mlua::Lua::new();

    for directory in fs::read_dir(root.join("mods")).unwrap().flatten() {
        let package_dir = directory.path();
        let source_path = package_dir.join("src/mod.lua");
        if !source_path.is_file() {
            continue;
        }

        let source = fs::read_to_string(&source_path).unwrap();
        lua.load(&source)
            .set_name(source_path.display().to_string())
            .into_function()
            .unwrap_or_else(|error| panic!("{}: {error}", source_path.display()));

        let manifest = read_manifest_path(root, &package_dir)
            .unwrap_or_else(|error| panic!("{}: {error}", package_dir.display()));
        let uses_runtime = [
            "runtime.ecs.",
            "runtime.world.",
            "runtime.patch.",
            "runtime.report_effect",
        ]
        .iter()
        .any(|pattern| source.contains(pattern));
        let uses_patch = [
            "runtime.require(\"game.assets.write\")",
            "runtime.require('game.assets.write')",
            "game.assets.update_asset(",
            "game.assets.save_assets(",
            "game.assets.reset_assets(",
            "game.assets.create_resource(",
            "game.assets.create_content(",
        ]
        .iter()
        .any(|pattern| source.contains(pattern));
        let uses_export = ["io.export", "io.export_exists"]
            .iter()
            .any(|pattern| source.contains(pattern));

        for (used, capability, feature) in [
            (uses_runtime, Capability::Runtime, "runtime API"),
            (uses_export, Capability::Export, "export API"),
        ] {
            assert_eq!(
                used,
                manifest.capabilities.contains(&capability),
                "{} capability does not match its {feature} usage",
                manifest.id
            );
        }

        // ShroudForge's explicit asset-write surface must be capability-gated.
        // EML mods can also mutate the objects returned by get_resources_by_type
        // directly, so a textual scan cannot require every declared `patch`
        // capability to match one of the newer explicit write calls above.
        assert!(
            !uses_patch || manifest.capabilities.contains(&Capability::Patch),
            "{} uses the asset write API without declaring the patch capability",
            manifest.id
        );

        assert_no_machine_patch_primitives(&source_path.display().to_string(), &source);
    }
}

#[test]
fn runtime_api_definition_has_no_machine_patch_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let definition =
        fs::read_to_string(root.join("src/loader/api/src/shroudforge/v1/definitions/runtime.lua"))
            .unwrap();
    assert_no_machine_patch_primitives("runtime definition", &definition);
}

fn assert_no_machine_patch_primitives(label: &str, source: &str) {
    for forbidden in [
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
