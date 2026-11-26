//! A game folder for tests that run whole startup passes: a small, valid archive pair, the loader
//! configuration and made-up packages.

use kfc::{
    container::{KFCFile, KFCWriter},
    reflection::TypeRegistry,
};
use serde_json::json;
use std::{fs, path::PathBuf, rc::Rc};

/// A game folder with a small archive pair and the type cache ShroudForge reads before it would
/// extract the types from the executable (`mod-runtime/reflection/load_type_registry.rs`).
pub(super) struct Game(pub(super) PathBuf);

impl Game {
    pub(super) fn new(name: &str) -> Game {
        let root = std::env::temp_dir().join(format!(
            "sf-startup-pass-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let exe = b"test executable identity";
        fs::write(root.join("enshrouded.exe"), exe).unwrap();
        for extension in ["kfc", "kfc_resources"] {
            fs::write(root.join(format!("enshrouded.{extension}")), []).unwrap();
        }
        KFCWriter::<Rc<KFCFile>, _>::new(
            &root,
            "enshrouded",
            Rc::new(TypeRegistry::default()),
            "startup pass fixture",
        )
        .unwrap()
        .finalize()
        .unwrap();
        shroudforge_loader_config::config::initialize_loader_config(&root).unwrap();
        let cache = shroudforge_loader_config::paths::cache_dir(&root);
        fs::create_dir_all(&cache).unwrap();
        let mut registry = TypeRegistry::default();
        registry.version = KFCFile::get_version_tag(root.join("enshrouded.kfc")).unwrap();
        fs::write(
            cache.join("types-enshrouded.json"),
            serde_json::to_vec(&registry).unwrap(),
        )
        .unwrap();
        // load_type_registry.rs's identity of the executable the cached types belong to
        fs::write(
            cache.join("types-enshrouded.identity.json"),
            serde_json::to_vec(&json!({
                "schemaVersion": 2,
                "executableSha256": shroudforge_loader_config::config::revision(exe),
                "reflectionExtractor": "kfc-a404b15cde40fc7bd43d812bb460ce40d2a9d3b1"
            }))
            .unwrap(),
        )
        .unwrap();
        Game(root)
    }

    /// A switched-on package with a `patch` part whose entrypoint is `lua`, needing `needs`.
    pub(super) fn package(&self, id: &str, lua: &str, needs: &[&str]) {
        let package = shroudforge_loader_config::paths::mods_dir(&self.0).join(id);
        fs::create_dir_all(package.join("src")).unwrap();
        let dependencies: Vec<_> = needs
            .iter()
            .map(|id| json!({"id": id, "version": "^1.0.0"}))
            .collect();
        fs::write(
            package.join("mod.json"),
            serde_json::to_vec(&json!({
                "id": id, "name": format!("The {id} mod"), "version": "1.2.3", "capabilities": ["patch"],
                "dependencies": dependencies
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            package.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
        )
        .unwrap();
        fs::write(package.join("src/mod.lua"), lua).unwrap();
    }

    /// The asset preparation record (`state.json` "assets").
    pub(super) fn record(&self) -> serde_json::Value {
        shroudforge_loader_config::config::read_document(
            &self.0,
            shroudforge_game_assets::documents::APPLIED,
        )
        .unwrap()
    }

    /// ShroudForge's log of this start.
    pub(super) fn log(&self) -> String {
        fs::read_to_string(shroudforge_loader_config::paths::current_log(&self.0))
            .unwrap_or_default()
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
