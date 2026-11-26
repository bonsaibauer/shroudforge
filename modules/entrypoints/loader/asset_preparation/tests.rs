use super::*;

#[test]
fn runtime_exports_do_not_restore_prepared_assets_during_startup() {
    let root = std::env::temp_dir().join(format!(
        "sf-startup-export-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    for (id, capabilities) in [
        ("production", serde_json::json!(["patch"])),
        ("editor", serde_json::json!(["runtime", "export"])),
    ] {
        let package = root.join("mods").join(id);
        std::fs::create_dir_all(package.join("src")).unwrap();
        std::fs::write(
            package.join("mod.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": id, "name": id, "version": "1.0.0", "capabilities": capabilities
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            package.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
        )
        .unwrap();
        std::fs::write(
            package.join("src/mod.lua"),
            "error('prepared assets must not be reapplied')",
        )
        .unwrap();
    }
    std::fs::write(root.join("enshrouded.exe"), b"test executable identity").unwrap();
    std::fs::write(root.join("enshrouded.kfc"), b"already prepared recipe data").unwrap();
    std::fs::write(root.join("enshrouded.kfc.bak"), b"original recipe data").unwrap();
    std::fs::write(root.join("enshrouded.kfc_resources"), b"prepared resources").unwrap();
    std::fs::write(
        root.join("enshrouded.kfc_resources.bak"),
        b"original resources",
    )
    .unwrap();
    shroudforge_loader_config::config::initialize_loader_config(&root).unwrap();
    let environment = shroudforge_package::ModEnvironment::load(root.to_str().unwrap()).unwrap();
    assert_eq!(
        environment.plan(false, shroudforge_api::API_VERSION).len(),
        2
    );
    let fingerprint = shroudforge_package::prepared::fingerprint(
        &environment,
        false,
        shroudforge_api::API_VERSION,
    )
    .unwrap();
    shroudforge_loader_config::config::write_document(&root, shroudforge_game_assets::documents::APPLIED, &serde_json::json!({
            "schemaVersion": 1, "status": "applied", "target": "enshrouded", "fingerprint": fingerprint,
            "phase": "prepare", "completedAt": 1, "mods": [{"id":"production","version":"1.0.0"}]
        })).unwrap();
    assert!(!export_pass_needed(&root, "enshrouded").unwrap());
    // Without the record of the pass the files are not known to be prepared.
    assert!(!already_applied(&root, "enshrouded", false).unwrap());
    // What the pass used in the export folder (recorded while exports are on, as here).
    let touches = shroudforge_game_assets::prepared_stamp::ExportTouches::default();
    touches.path("map.json");
    let exports = shroudforge_loader_config::paths::export_dir(&root);
    let record = || {
        shroudforge_game_assets::prepared_stamp::record(
            &root,
            "enshrouded",
            &shroudforge_game_assets::prepared_stamp::PassInputs {
                fingerprint: &fingerprint,
                exports_enabled: true,
                exporters: Some(&exports),
            },
            &touches,
        )
        .unwrap()
    };
    record();
    assert!(already_applied(&root, "enshrouded", false).unwrap());
    run_startup(&root).unwrap();
    assert_eq!(
        std::fs::read(root.join("enshrouded.kfc")).unwrap(),
        b"already prepared recipe data"
    );
    // The game's file replaced after the pass (another loader, Steam's file check).
    std::fs::write(root.join("enshrouded.kfc"), b"vanilla recipe data").unwrap();
    assert!(!already_applied(&root, "enshrouded", false).unwrap());
    std::fs::write(root.join("enshrouded.kfc"), b"already prepared recipe data").unwrap();
    record();
    assert!(already_applied(&root, "enshrouded", false).unwrap());
    // The pass is skipped only while what it used in the export folder is as it left it.
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::write(exports.join("map.json"), b"{}").unwrap();
    assert!(!already_applied(&root, "enshrouded", true).unwrap());
    record();
    assert!(already_applied(&root, "enshrouded", true).unwrap());
    std::fs::remove_file(exports.join("map.json")).unwrap();
    assert!(!already_applied(&root, "enshrouded", true).unwrap());
    // A real pregame exporter must still request its export pass.
    std::fs::write(
        root.join("mods/editor/mod.json"),
        r#"{"id":"editor","name":"editor","version":"1.0.0","capabilities":["export"]}"#,
    )
    .unwrap();
    assert!(export_pass_needed(&root, "enshrouded").unwrap());
}
