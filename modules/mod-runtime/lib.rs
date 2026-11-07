pub mod documents;
use kfc::{ResourceReference, TypeDefinition};
use shroudforge_compatibility::GameContract;
use std::path::PathBuf;
use std::sync::Arc;

mod ingame;
mod logging;
mod lua;
mod native_bridge;
mod native_dlls;
pub mod reflection;
mod session;
mod startup;
mod type_aliases;
use ingame as runtime;
use lua::bindings as env;
use lua::definitions as definition;
use lua::eml;
use lua::mod_runner as runner;
use reflection as runtime_resolution;
use shroudforge_runtime_bridge as runtime_provider;
use startup::cache;
use startup::open_game_assets as game_files;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Deployed contract must describe this binary, not invent native capabilities.
/// ECS readiness and per-component compatibility are still checked at call time.
pub fn runtime_operations(_root: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let value =
        lua::shroudforge::v1::read_runtime_operations::catalog().map_err(anyhow::Error::msg)?;
    shroudforge_loader_config::config::validate_document(_root, crate::documents::API, &value)
        .map_err(anyhow::Error::msg)?;
    lua::shroudforge::v1::read_runtime_operations::operations().map_err(anyhow::Error::msg)
}

/// Parsed and compatibility-verified game model used by the Lua API.
#[derive(Clone)]
pub struct ShroudForgeApi {
    contract: Arc<GameContract>,
}

impl ShroudForgeApi {
    pub fn new(contract: GameContract) -> Self {
        Self {
            contract: Arc::new(contract),
        }
    }

    pub fn game_version(&self) -> &str {
        &self.contract.schema().game_version
    }

    pub fn parser_id(&self) -> &str {
        &self.contract.schema().parser
    }

    pub fn type_by_name(&self, name: &str) -> Option<&TypeDefinition> {
        self.contract.schema().types.get(name)
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeDefinition> {
        self.contract.schema().types.values()
    }

    pub fn resource_types(&self) -> impl Iterator<Item = (&str, usize)> {
        self.contract
            .schema()
            .resources
            .iter()
            .map(|(name, values)| (name.as_str(), values.len()))
    }

    pub fn resources(&self, type_name: &str) -> &[ResourceReference] {
        self.contract
            .schema()
            .resources
            .get(type_name)
            .map_or(&[], Vec::as_slice)
    }

    pub fn has_runtime(&self, operation: &str) -> bool {
        self.contract.has_runtime(operation)
    }

    pub fn runtime(&self, operation: &str) -> Availability {
        self.contract.runtime(operation)
    }
}

pub use kfc::{
    EnumValueDefinition, FieldDefinition, GameFiles, GameParser, KfcParser, ParserError,
};
pub use shroudforge_compatibility::Availability;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimePhase {
    #[default]
    Pregame,
    Ingame,
}

impl RuntimePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pregame => "pregame",
            Self::Ingame => "ingame",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// If true, it will ignore the cache and re-apply all mods.
    pub skip_cache: bool,
    /// If true, keep the asset writer enabled even when input fingerprints match.
    /// Used when a caller has explicitly restored the clean KFC baseline.
    pub force_patch: bool,
    /// If None, it will auto-detect based on the files within the game directory.
    pub is_server: Option<bool>,
    /// If true, mods may write typed resources back to the game asset container.
    pub patch: bool,
    /// If true, it will allow mods to use `io.export` to export files to the export directory.
    pub export: bool,
    /// If None, it will use the default export directory (`<game_dir>/shroudforge/exports`).
    pub export_dir: Option<PathBuf>,
    /// Execution phase. This changes availability, never the public API shape.
    pub phase: RuntimePhase,
}

#[derive(Debug, Clone)]
pub struct RunArgs {
    pub file_name: String,
    /// The features to enable.
    pub options: RunOptions,
}

pub use lua::definitions::export_lua_definitions::export_lua_definitions;
pub use native_dlls::prepare_startup as prepare_native_dlls;
pub use runtime::IngameRuntime;
pub use startup::manage_asset_baseline::{
    backup_companion_file, create_working_kfc_backup, restore, working_baseline_is_incompatible,
};
pub use startup::run_startup_mods::{run, run_with_local_schema};

#[cfg(test)]
use runtime::Lifecycle;
