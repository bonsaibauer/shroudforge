//! Host-owned selected startup. Package declarations express demand, not readiness.
use mod_loader::{Capability, ModEnvironment};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub const METADATA: u32 = 1;
pub const GENERIC_HOOKS: u32 = 2;
pub const MAP: u32 = 4;
pub const GRAPHICS: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanV2 {
    pub struct_bytes: u32,
    pub version: u32,
    pub services: u32,
    pub process_role: u32,
    pub opt_ins: u32,
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Admitted,
    Taken,
    Committed,
    Creating,
    Running,
    Failed,
}

struct Input {
    path: PathBuf,
    digest: [u8; 32],
    // On Windows this reference denies replacement and writes through native loading.
    _lease: fs::File,
}

struct Admission {
    root: PathBuf,
    plan: PlanV2,
    environment: ModEnvironment,
    identity: serde_json::Value,
    inputs: Vec<Input>,
    phase: Phase,
}

static ADMISSION: OnceLock<Mutex<Option<Admission>>> = OnceLock::new();
fn state() -> &'static Mutex<Option<Admission>> {
    ADMISSION.get_or_init(|| Mutex::new(None))
}

fn canonical(root: &Path) -> Result<PathBuf, String> {
    // Reject junctions/symlinks before canonicalization can hide their identity.
    for path in root.ancestors() {
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Startup path contains a reparse point".into());
            }
        }
        if metadata.file_type().is_symlink() {
            return Err("Startup path contains a symbolic link".into());
        }
    }
    fs::canonicalize(root).map_err(|e| e.to_string())
}

fn environment(root: &Path) -> Result<ModEnvironment, String> {
    let root = root.to_str().ok_or("Startup root is not UTF-8")?;
    ModEnvironment::load(root).map_err(|e| format!("Startup package discovery failed: {e:?}"))
}

fn package_code(path: &Path, package_root: &Path) -> bool {
    // Only executable inputs and the authored manifest are immutable here.
    // extended.mod.json and [Plugin] declarations have parsed identity below;
    // other package data belongs to the mod and may be writable during init.
    path == package_root.join("mod.json")
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                ["lua", "luac", "dll", "exe"]
                    .iter()
                    .any(|code| extension.eq_ignore_ascii_case(code))
            })
}

fn inspect(
    env: &ModEnvironment,
    server: bool,
) -> Result<(u32, serde_json::Value, BTreeSet<PathBuf>), String> {
    let root = env.game_dir().as_std_path();
    let (packages, issues) = env.plan_report(server, crate::API_VERSION);
    let mut services = 0;
    let mut manifest_identity = Vec::new();
    let mut files = BTreeSet::new();
    for package in &packages {
        let info = package.info();
        for capability in &info.capabilities {
            services |= match capability {
                Capability::RuntimeEngineHooks => GENERIC_HOOKS,
                Capability::RuntimeMap => MAP,
                Capability::RuntimeOverlay => GRAPHICS,
                _ => 0,
            };
        }
        manifest_identity.push(serde_json::to_value(info).map_err(|e| e.to_string())?);
    }
    let (ordinary, plugins, native_issues) =
        crate::native_dlls::selected_startup_plugins(root, server);
    for plugin in &plugins {
        if plugin.declaration.state.is_enabled() {
            if plugin.declaration.declares_service("shroudforge.map") {
                services |= MAP;
            }
            if plugin.declaration.declares_service("shroudforge.overlay") {
                services |= GRAPHICS;
            }
        }
    }
    // Existing packages without selected service demand retain legacy startup.
    if services == 0 {
        return Ok((0, serde_json::Value::Null, files));
    }
    for package in &packages {
        let package_root = package.fs().root().as_std_path().to_path_buf();
        if package_root.is_file() {
            files.insert(package_root);
        } else {
            for item in walkdir::WalkDir::new(&package_root).follow_links(false) {
                let item = item.map_err(|e| e.to_string())?;
                if item.file_type().is_symlink() {
                    return Err("Selected package contains a symbolic link".into());
                }
                if item.file_type().is_file() && package_code(item.path(), &package_root) {
                    files.insert(item.into_path());
                }
            }
        }
    }
    for plugin in &plugins {
        let package = packages
            .iter()
            .find(|p| plugin.entry.mod_id.as_deref() == Some(p.info().id.as_str()))
            .ok_or("Approved native plugin is absent from the executable package plan")?;
        if !package
            .info()
            .capabilities
            .contains(&Capability::RuntimeRegisterDll)
        {
            return Err("Native service consumer lacks runtime-register-dll capability".into());
        }
        files.insert(plugin.entry.path.clone());
    }
    if !issues.is_empty() || native_issues {
        return Err("Selected startup has unresolved package or DLL selections".into());
    }
    if !ordinary.is_empty() {
        return Err(
            "Selected startup cannot compose ordinary DLL execution with framework services".into(),
        );
    }
    if server && services & (MAP | GRAPHICS) != 0 {
        return Err("Client map and graphics services cannot start in a server process".into());
    }
    let dll_plan = mod_loader::dll_manager::resolve_load_plan(root, server);
    let entries: Vec<_> = dll_plan
        .entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "statusId":entry.status_id,"modId":entry.mod_id,"package":entry.package_directory,
                "path":entry.path,"mode":entry.mode,"source":entry.source
            })
        })
        .collect();
    // Profiles can provide virtual Lua/package files outside the physical mod
    // directory. Bind their contents as well as the resolved manifests.
    let profiles =
        mod_loader::mod_profiles::read_profiles(root).map_err(|errors| errors.join("; "))?;
    let declarations: Vec<_> = plugins
        .iter()
        .map(|plugin| {
            let declaration = &plugin.declaration;
            serde_json::json!({"path":plugin.entry.path,"state":declaration.state.as_str(),
            "dll":declaration.dll,"dlls":declaration.declared_dlls,
            "lifecycle":declaration.lifecycle,"services":declaration.services})
        })
        .collect();
    let identity = serde_json::json!({"packages":manifest_identity,"dlls":entries,
        "profiles":profiles,"declarations":declarations});
    Ok((services, identity, files))
}

impl Admission {
    fn capture(root: &Path, server: bool) -> Result<Option<Self>, String> {
        let root = canonical(root)?;
        let environment = environment(&root)?;
        let (services, identity, mut paths) = inspect(&environment, server)?;
        if services == 0 {
            return Ok(None);
        }
        paths.insert(root.join("shroudforge/kfc-runtime.dll"));
        let mut inputs = Vec::new();
        for path in paths {
            let path = canonical(&path)?;
            if !path.starts_with(&root) {
                return Err("Selected startup input escapes the installation".into());
            }
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(1);
            }
            let mut lease = options
                .open(&path)
                .map_err(|e| format!("Could not retain startup input {}: {e}", path.display()))?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let count = lease.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            inputs.push(Input {
                path,
                digest: hash.finalize().into(),
                _lease: lease,
            });
        }
        let admitted = Self {
            root,
            plan: PlanV2 {
                struct_bytes: 24,
                version: 2,
                services: services | METADATA,
                process_role: if server { 2 } else { 1 },
                opt_ins: 0,
                reserved: 0,
            },
            environment,
            identity,
            inputs,
            phase: Phase::Admitted,
        };
        admitted.verify()?;
        Ok(Some(admitted))
    }
    fn verify(&self) -> Result<(), String> {
        let (services, identity, paths) =
            inspect(&environment(&self.root)?, self.plan.process_role == 2)?;
        if services | METADATA != self.plan.services || identity != self.identity {
            return Err("Selected startup composition changed; restart is required".into());
        }
        let mut expected: BTreeSet<_> = paths
            .iter()
            .map(|p| canonical(p))
            .collect::<Result<_, _>>()?;
        expected.insert(canonical(&self.root.join("shroudforge/kfc-runtime.dll"))?);
        if expected != self.inputs.iter().map(|i| i.path.clone()).collect() {
            return Err("Selected startup file membership changed".into());
        }
        // Windows leases prevent mutations; this also checks non-Windows fixtures.
        for input in &self.inputs {
            let mut file = fs::File::open(&input.path).map_err(|e| e.to_string())?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if <[u8; 32]>::from(hash.finalize()) != input.digest {
                return Err("Selected startup input bytes changed".into());
            }
        }
        Ok(())
    }
    fn transition(&mut self, root: &Path, from: Phase, to: Phase) -> Result<(), String> {
        let result = (|| {
            if self.phase == Phase::Failed || self.phase != from || canonical(root)? != self.root {
                return Err("Selected startup phase or root differs".into());
            }
            self.verify()?;
            Ok(())
        })();
        self.phase = if result.is_ok() { to } else { Phase::Failed };
        result
    }
}

/// Called after asset preparation, before any native loading. A selected plan
/// never falls back to legacy startup after admission or native preparation fails.
pub fn prepare(root: &Path, server: bool) -> Result<bool, String> {
    let mut state = state()
        .lock()
        .map_err(|_| "Startup admission lock poisoned")?;
    if state.is_some() {
        return Err("Selected startup was already admitted".into());
    }
    let Some(admission) = Admission::capture(root, server)? else {
        return Ok(false);
    };
    *state = Some(admission);
    if let Err(error) = crate::native_dlls::prepare_selected_startup(root, server) {
        state.as_mut().unwrap().phase = Phase::Failed;
        return Err(error);
    }
    Ok(true)
}
pub fn take(root: &Path) -> Result<Option<PlanV2>, String> {
    let mut state = state()
        .lock()
        .map_err(|_| "Startup admission lock poisoned")?;
    let Some(admission) = state.as_mut() else {
        return Ok(None);
    };
    admission.transition(root, Phase::Admitted, Phase::Taken)?;
    Ok(Some(admission.plan))
}
pub fn commit(root: &Path, completed: u32) -> Result<(), String> {
    let mut state = state()
        .lock()
        .map_err(|_| "Startup admission lock poisoned")?;
    let admission = state.as_mut().ok_or("No admitted startup")?;

    if completed != admission.plan.services {
        admission.phase = Phase::Failed;
        return Err("Native startup completed a different service mask".into());
    }
    admission.transition(root, Phase::Taken, Phase::Committed)
}
pub fn before_create(root: &Path) -> Result<Option<ModEnvironment>, String> {
    let mut state = state()
        .lock()
        .map_err(|_| "Startup admission lock poisoned")?;
    let Some(admission) = state.as_mut() else {
        return Ok(None);
    };
    admission.transition(root, Phase::Committed, Phase::Creating)?;
    Ok(Some(admission.environment.clone()))
}
pub fn after_create(success: bool) {
    if let Ok(mut state) = state().lock() {
        if let Some(admission) = state.as_mut() {
            admission.phase = if success && admission.phase == Phase::Creating {
                Phase::Running
            } else {
                Phase::Failed
            };
            // Startup-only package leases no longer block runtime settings.
            admission.inputs.clear();
        }
    }
}

pub(crate) fn require_ordinary_loading() -> Result<(), String> {
    let state = state()
        .lock()
        .map_err(|_| "Startup admission lock poisoned")?;
    if state.is_some() {
        return Err("Ordinary DLL execution is unsupported in selected startup".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(capabilities: &[&str]) -> Self {
            let root =
                std::env::temp_dir().join(format!("sf-selected-startup-{}", uuid::Uuid::new_v4()));
            let package = root.join("mods/probe");
            fs::create_dir_all(&package).unwrap();
            fs::create_dir_all(root.join("shroudforge")).unwrap();
            fs::write(
                root.join("shroudforge/kfc-runtime.dll"),
                b"inert fixture; never loaded",
            )
            .unwrap();
            fs::write(
                package.join("mod.json"),
                serde_json::to_vec(&serde_json::json!({
                    "id":"probe","name":"Probe","version":"1.0.0","capabilities":capabilities
                }))
                .unwrap(),
            )
            .unwrap();
            fs::write(
                package.join("extended.mod.json"),
                br#"{"schemaVersion":1,"enabled":true,"targets":["client"]}"#,
            )
            .unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn selected_startup_uses_actual_package_demand_and_refuses_changed_inputs() {
        let legacy = Fixture::new(&["runtime"]);
        assert!(Admission::capture(&legacy.0, false).unwrap().is_none());
        let fixture = Fixture::new(&["runtime-map", "runtime-overlay"]);
        let config = fixture.0.join("mods/probe/config.ini");
        let log = fixture.0.join("mods/probe/runtime.log");
        fs::write(&config, b"[Settings]\nValue=1\n").unwrap();
        fs::write(&log, b"old log").unwrap();
        let mut admission = Admission::capture(&fixture.0, false).unwrap().unwrap();
        // Existing mod-owned outputs remain writable before initialization.
        fs::write(&config, b"[Settings]\nValue=2\n").unwrap();
        fs::write(&log, b"initialized").unwrap();
        fs::write(
            fixture.0.join("mods/probe/extended.mod.json"),
            b"{ \"schemaVersion\":1, \"enabled\":true, \"targets\":[\"client\"] }\n",
        )
        .unwrap();
        assert_eq!(admission.plan.services, METADATA | MAP | GRAPHICS);
        assert!(
            admission
                .transition(&fixture.0, Phase::Admitted, Phase::Taken)
                .is_ok()
        );
        fs::write(fixture.0.join("mods/probe/new.lua"), b"return nil").unwrap();
        assert!(
            admission
                .transition(&fixture.0, Phase::Taken, Phase::Committed)
                .is_err()
        );
        assert_eq!(admission.phase, Phase::Failed);
        assert!(
            admission
                .transition(&fixture.0, Phase::Failed, Phase::Committed)
                .is_err()
        );
        let fixture = Fixture::new(&["runtime-map"]);
        let mut admission = Admission::capture(&fixture.0, false).unwrap().unwrap();
        let profiles = shroudforge_loader_config::paths::mod_profiles_dir(&fixture.0);
        fs::create_dir_all(&profiles).unwrap();
        fs::write(
            profiles.join("probe.json"),
            br#"{
            "schemaVersion":1,"id":"probe","packageDirectory":"probe","name":"Probe",
            "packageFiles":{"src/mod.lua":"return {}"}
        }"#,
        )
        .unwrap();
        assert!(
            admission
                .transition(&fixture.0, Phase::Admitted, Phase::Taken)
                .is_err()
        );
    }

    #[test]
    fn selected_startup_rejects_wrong_root_early_create_and_ordinary_dll_composition() {
        let fixture = Fixture::new(&["runtime-map"]);
        let other = Fixture::new(&["runtime-map"]);
        for (root, from) in [(&other.0, Phase::Admitted), (&fixture.0, Phase::Committed)] {
            let mut admission = Admission::capture(&fixture.0, false).unwrap().unwrap();
            assert!(admission.transition(root, from, Phase::Creating).is_err());
        }
        let ordinary = fixture.0.join("mods/ordinary");
        fs::create_dir_all(&ordinary).unwrap();
        fs::write(
            ordinary.join("native-plugin.ini"),
            b"[Plugin]\nEnabled=true\nDll=original.dll\n",
        )
        .unwrap();
        fs::write(ordinary.join("original.dll"), b"must never execute").unwrap();
        let (entries, plugins, issues) =
            crate::native_dlls::selected_startup_plugins(&fixture.0, false);
        assert_eq!(entries.len(), 1);
        assert!(plugins.is_empty() && !issues);
        let error = Admission::capture(&fixture.0, false).err().unwrap();
        assert!(error.contains("ordinary DLL execution"), "{error}");
        // Without selected service demand, the same ordinary DLL stays on the
        // existing data-driven loading path; it is never executed by this test.
        let manifest = fixture.0.join("mods/probe/mod.json");
        fs::write(
            manifest,
            br#"{"id":"probe","name":"Probe","version":"1.0.0","capabilities":["runtime"]}"#,
        )
        .unwrap();
        assert!(Admission::capture(&fixture.0, false).unwrap().is_none());
    }

    #[test]
    fn removed_capability_cannot_enter_the_executable_package_plan() {
        let fixture = Fixture::new(&["runtime-first-person"]);
        let env = environment(&fixture.0).unwrap();
        assert!(!env.mod_registry().contains_key("probe"));
        assert!(env.plan(false, crate::API_VERSION).is_empty());
    }

    #[test]
    fn selected_startup_public_sequence_commits_once_before_create_without_loading_code() {
        let fixture = Fixture::new(&[]);
        let profiles = shroudforge_loader_config::paths::mod_profiles_dir(&fixture.0);
        fs::create_dir_all(&profiles).unwrap();
        fs::write(
            profiles.join("probe.json"),
            br#"{
            "schemaVersion":1,"id":"probe","packageDirectory":"probe","name":"Probe",
            "modJson":{"capabilities":["runtime-map"]},
            "packageFiles":{"src/mod.lua":"return {}"}
        }"#,
        )
        .unwrap();
        assert!(require_ordinary_loading().is_ok());
        assert!(prepare(&fixture.0, false).unwrap());
        assert!(require_ordinary_loading().is_err());
        let plan = take(&fixture.0).unwrap().unwrap();
        assert_eq!(plan.services, METADATA | MAP);
        commit(&fixture.0, plan.services).unwrap();
        let env = before_create(&fixture.0).unwrap().unwrap();
        assert!(env.mod_registry().contains_key("probe"));
        let mut script = String::new();
        env.mod_registry()["probe"]
            .fs()
            .read_file("src/mod.lua")
            .unwrap()
            .read_to_string(&mut script)
            .unwrap();
        assert_eq!(script, "return {}");
        after_create(true);
        assert!(require_ordinary_loading().is_err());
        assert!(before_create(&fixture.0).is_err());
        assert!(take(&fixture.0).is_err());
        // Clear only this test-owned singleton; ordinary DLL executor was never started.
        *state().lock().unwrap() = None;
    }
}
