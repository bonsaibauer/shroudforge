pub mod manifest;
mod pregame;

use manifest::{Capability, PackageManifest, validate};
use shroudforge_api::{IngameRuntime, ShroudForgeApi};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::Read,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub use pregame::run as prepare;

const PROVIDED_COMPONENTS: &[(&str, &str)] = &[("shroudforge-api", shroudforge_api::API_VERSION)];

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("I/O error at {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid manifest {0}: {1}")]
    Manifest(PathBuf, String),
    #[error("duplicate mod id: {0}")]
    Duplicate(String),
    #[error("dependency error for {0}: {1}")]
    Dependency(String, String),
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),
    #[error("Lua contract error in {0}: {1}")]
    LuaContract(String, String),
    #[error("loader environment error: {0}")]
    Environment(String),
    #[error("pregame phase failed: {0}")]
    Pregame(String),
}

#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub lua: Vec<String>,
}

struct Package {
    manifest: PackageManifest,
    package: shroudforge_package::Mod,
}

pub struct ModLoader {
    api: ShroudForgeApi,
    runtime: Option<IngameRuntime>,
}

impl ModLoader {
    pub fn new(api: ShroudForgeApi, is_client: bool) -> Self {
        let _ = is_client;
        Self { api, runtime: None }
    }

    pub fn load_directory(&mut self, mods: impl AsRef<Path>) -> Result<LoadReport, LoaderError> {
        let (packages, enabled) = discover(mods.as_ref())?;
        let order = resolve(&packages, &enabled)?;
        let mut report = LoadReport::default();
        for id in order {
            let item = &packages[&id];
            let is_runtime = item
                .manifest
                .package
                .capabilities
                .contains(&Capability::Runtime);
            if !is_runtime {
                continue;
            }
            let lua_path = item.manifest.lua_entrypoint();
            if !item.package.fs().exists(lua_path) {
                continue;
            }
            report.lua.push(id);
        }
        let game = mods.as_ref().parent().ok_or_else(|| {
            LoaderError::Environment(format!(
                "mods path has no parent: {}",
                mods.as_ref().display()
            ))
        })?;
        let game_utf8 = game.to_str().ok_or_else(|| {
            LoaderError::Environment(format!("path is not UTF-8: {}", game.display()))
        })?;
        let environment = shroudforge_package::ModEnvironment::load(game_utf8)
            .map_err(|value| LoaderError::Environment(format_environment(&value)))?;
        let file_name = if game.join("enshrouded.exe").is_file() {
            "enshrouded"
        } else {
            "enshrouded_server"
        };
        self.runtime = Some(
            IngameRuntime::start(&environment, self.api.clone(), file_name)
                .map_err(|error| LoaderError::LuaContract("runtime".into(), error.to_string()))?,
        );
        Ok(report)
    }

    pub fn update(&mut self, delta_seconds: f64) {
        if let Some(runtime) = &mut self.runtime {
            runtime.update(delta_seconds);
        }
    }
}

fn discover(mods: &Path) -> Result<(BTreeMap<String, Package>, BTreeSet<String>), LoaderError> {
    let game = mods.parent().ok_or_else(|| {
        LoaderError::Environment(format!("mods path has no parent: {}", mods.display()))
    })?;
    let game_utf8 = game.to_str().ok_or_else(|| {
        LoaderError::Environment(format!("path is not UTF-8: {}", game.display()))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8);
    let environment = match environment {
        Ok(environment) => environment,
        Err(report) if report.error.is_none() => {
            return Err(LoaderError::Environment(format_environment(&report)));
        }
        Err(report) => return Err(LoaderError::Environment(format_environment(&report))),
    };
    let registry: Vec<shroudforge_package::Mod> =
        environment.mod_registry().values().cloned().collect();
    let mut result = BTreeMap::new();
    let mut enabled = BTreeSet::new();
    for package in registry {
        let root = package_path(&package);
        let text = read_text(&package, "mod.json")?;
        let manifest: PackageManifest = serde_json::from_str(&text)
            .map_err(|error| LoaderError::Manifest(root.join("mod.json"), error.to_string()))?;
        validate(&manifest).map_err(|error| LoaderError::Manifest(root.join("mod.json"), error))?;
        let id = manifest.package.id.clone();
        if environment.is_mod_enabled(&id) {
            enabled.insert(id.clone());
        }
        if result
            .insert(id.clone(), Package { manifest, package })
            .is_some()
        {
            return Err(LoaderError::Duplicate(id));
        }
    }
    Ok((result, enabled))
}

fn read_text(package: &shroudforge_package::Mod, relative: &str) -> Result<String, LoaderError> {
    let path = package_path(package).join(relative);
    let mut fs = package.fs();
    let mut reader = fs
        .read_file(relative)
        .map_err(|error| LoaderError::Io(path.clone(), error))?;
    let mut source = String::new();
    reader
        .read_to_string(&mut source)
        .map_err(|error| LoaderError::Io(path, error))?;
    Ok(source)
}

fn package_path(package: &shroudforge_package::Mod) -> PathBuf {
    package.fs().root().as_std_path().to_path_buf()
}

fn format_environment(report: &shroudforge_package::ModEnvironmentErrorReport) -> String {
    if let Some(error) = &report.error {
        return error.to_string();
    }
    report
        .mods
        .iter()
        .map(|value| format!("{}: {}", value.path, value.error))
        .collect::<Vec<_>>()
        .join("; ")
}

fn resolve(
    packages: &BTreeMap<String, Package>,
    enabled: &BTreeSet<String>,
) -> Result<Vec<String>, LoaderError> {
    fn visit(
        id: &str,
        packages: &BTreeMap<String, Package>,
        enabled: &BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), LoaderError> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.into()) {
            return Err(LoaderError::Dependency(
                id.into(),
                "dependency cycle".into(),
            ));
        }
        for dependency in &packages[id].manifest.package.dependencies {
            if let Some((_, version)) = PROVIDED_COMPONENTS
                .iter()
                .find(|(name, _)| *name == dependency.id)
            {
                let version =
                    semver::Version::parse(version).expect("workspace API version is semver");
                if !dependency.version.matches(&version) {
                    return Err(LoaderError::Dependency(
                        id.into(),
                        format!(
                            "{} {} does not match {}",
                            dependency.id, version, dependency.version
                        ),
                    ));
                }
                continue;
            }
            match packages.get(&dependency.id) {
                None if dependency.optional.unwrap_or(false) => continue,
                None => {
                    return Err(LoaderError::Dependency(
                        id.into(),
                        format!("missing {}", dependency.id),
                    ));
                }
                Some(_)
                    if !enabled.contains(&dependency.id)
                        && dependency.optional.unwrap_or(false) =>
                {
                    continue;
                }
                Some(_) if !enabled.contains(&dependency.id) => {
                    return Err(LoaderError::Dependency(
                        id.into(),
                        format!("dependency {} is disabled", dependency.id),
                    ));
                }
                Some(candidate)
                    if !dependency
                        .version
                        .matches(&candidate.manifest.package.version) =>
                {
                    return Err(LoaderError::Dependency(
                        id.into(),
                        format!(
                            "{} {} does not match {}",
                            dependency.id, candidate.manifest.package.version, dependency.version
                        ),
                    ));
                }
                Some(_) => visit(&dependency.id, packages, enabled, visiting, done, order)?,
            }
        }
        visiting.remove(id);
        done.insert(id.into());
        order.push(id.into());
        Ok(())
    }
    let mut order = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for id in enabled {
        visit(id, packages, enabled, &mut visiting, &mut done, &mut order)?;
    }
    Ok(order)
}

#[repr(C)]
pub struct RuntimeHandle(ModLoader);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_create(
    game: *const u16,
    mods: *const u16,
) -> *mut RuntimeHandle {
    fn path(value: *const u16) -> Option<PathBuf> {
        if value.is_null() {
            return None;
        }
        let mut length = 0;
        unsafe {
            while *value.add(length) != 0 {
                length += 1;
            }
        }
        Some(PathBuf::from(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(value, length)
        })))
    }
    fn append_runtime_log(game: &Path, message: &str) {
        let log = game.join("shroudforge.log");
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log) {
            let line = shroudforge_package::logging::format_line('E', "runtime", message);
            let _ = writeln!(file, "{line}");
        }
    }
    fn create_runtime(game: PathBuf, mods: PathBuf) -> Result<RuntimeHandle, String> {
        let _ = shroudforge_package::logging::initialize(&game, false);
        let is_client = game.join("enshrouded.exe").is_file();
        let files = if is_client {
            shroudforge_parser::GameFiles::client(&game)
        } else {
            shroudforge_parser::GameFiles::server(&game)
        };
        let schema = shroudforge_parser::GameParser::parse(&shroudforge_parser::KfcParser, &files)
            .map_err(|error| format!("parser failed: {error}"))?;
        let contract = shroudforge_compatibility::Compatibility::new([
            "runtime.lifecycle",
            "runtime.ecs.query",
            "runtime.ecs.resolve",
            "runtime.ecs.read",
            "runtime.ecs.write",
        ])
        .resolve(schema);
        let mut loader = ModLoader::new(ShroudForgeApi::new(contract), is_client);
        loader
            .load_directory(&mods)
            .map_err(|error| format!("mod loading failed at {}: {error}", mods.display()))?;
        Ok(RuntimeHandle(loader))
    }
    std::panic::catch_unwind(|| {
        let Some(game) = path(game) else {
            return std::ptr::null_mut();
        };
        let Some(mods) = path(mods) else {
            append_runtime_log(&game, "Runtime initialization failed: null mods path");
            return std::ptr::null_mut();
        };
        match create_runtime(game.clone(), mods) {
            Ok(handle) => Box::into_raw(Box::new(handle)),
            Err(error) => {
                append_runtime_log(&game, &format!("Runtime initialization failed: {error}"));
                std::ptr::null_mut()
            }
        }
    })
    .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_update(
    handle: *mut RuntimeHandle,
    delta_seconds: f64,
) -> bool {
    if handle.is_null() {
        return false;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        (*handle).0.update(delta_seconds)
    }))
    .is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_destroy(handle: *mut RuntimeHandle) {
    if !handle.is_null() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(handle) })
        }));
    }
}
