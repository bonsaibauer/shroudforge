use crate::LoaderError;
use shroudforge_api::ShroudForgeApi;
use shroudforge_compatibility::Compatibility;
use shroudforge_parser::{GameFiles, GameParser, KfcParser};
use std::path::Path;

pub fn run(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let game_utf8 = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    let file_name = if game_directory.join("enshrouded.exe").is_file() {
        "enshrouded"
    } else if game_directory.join("enshrouded_server.exe").is_file() {
        "enshrouded_server"
    } else {
        return Err(LoaderError::Environment(format!(
            "no Enshrouded executable in {}",
            game_directory.display()
        )));
    };
    let backup = game_directory.join(format!("{file_name}.kfc.bak"));
    if backup.is_file() && !shroudforge_api::restore(game_utf8, file_name) {
        return Err(LoaderError::Pregame(format!(
            "failed to restore the clean {file_name}.kfc baseline"
        )));
    }
    let files = if file_name == "enshrouded" {
        GameFiles::client(game_directory)
    } else {
        GameFiles::server(game_directory)
    };
    let schema = KfcParser
        .parse(&files)
        .map_err(|error| LoaderError::Pregame(error.to_string()))?;
    let api = ShroudForgeApi::new(Compatibility::default().resolve(schema));
    shroudforge_api::run(
        &environment,
        api,
        shroudforge_api::RunArgs {
            file_name: file_name.into(),
            options: shroudforge_api::RunOptions {
                assets_write: true,
                export: true,
                ..Default::default()
            },
        },
    )
    .map_err(|error| LoaderError::Pregame(error.to_string()))?;
    Ok(())
}

fn format_report(report: &shroudforge_package::ModEnvironmentErrorReport) -> String {
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
