//! Whole startup passes with a pregame exporter, on a small, valid game archive. A start reuses
//! the last pass while runtime mods write into the export folder, and runs it again when what the
//! exporter wrote or read there changed (game-files/record_prepared_game_files.rs). Shaped on a
//! player's export folder: flat files at its top from pregame exporters (XHL Area Harvest, Auto
//! Loot Plus), a pregame exporter's own folder (Unified EML Tweaks: `unified-tweaks/`), and runtime
//! mods' folders and files beside them (Unified Tweaks Live: `unified-tweaks-live/`, a native
//! DLL's `weapon_holster/`).

use super::test_game::Game;
use super::*;
use serde_json::json;
use std::fs;

/// The exporter's entrypoint: reads its saved settings when they are there, writes two flat files
/// and one in its folder, and counts its passes in a file of its own.
const EXPORTER: &str = r#"
local settings = "none"
if io.export_exists("unified-tweaks/user-settings.json") then
  settings = io.read_export_to_string("unified-tweaks/user-settings.json")
end
io.export("xhl-area-harvest-family-manifest.tsv", "family\trows")
io.export("auto-loot-plus-report.txt", "boss:field\tlabel")
io.export("unified-tweaks/ut-reference.txt", "UTREF " .. settings)
local passes = 0
if io.export_exists("unified-tweaks/passes.txt") then
  passes = tonumber(io.read_export_to_string("unified-tweaks/passes.txt"))
end
io.export("unified-tweaks/passes.txt", tostring(passes + 1))
return {}
"#;

fn exporter(game: &Game) {
    game.package("unifiedemltweaks", EXPORTER, &[]);
    let manifest =
        shroudforge_loader_config::paths::mods_dir(&game.0).join("unifiedemltweaks/mod.json");
    fs::write(
        manifest,
        serde_json::to_vec(&json!({
            "id": "unifiedemltweaks", "name": "Unified EML Tweaks", "version": "2.1.2",
            "capabilities": ["patch", "export"]
        }))
        .unwrap(),
    )
    .unwrap();
}

/// A runtime mod with the export capability: never part of the pregame pass.
fn runtime_exporter(game: &Game) {
    game.package("unified-tweaks-live", "return {}", &[]);
    let manifest =
        shroudforge_loader_config::paths::mods_dir(&game.0).join("unified-tweaks-live/mod.json");
    fs::write(
        manifest,
        serde_json::to_vec(&json!({
            "id": "unified-tweaks-live", "name": "Unified Tweaks Live", "version": "2.1.2",
            "capabilities": ["runtime", "export"]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn exports(game: &Game) -> std::path::PathBuf {
    shroudforge_loader_config::paths::export_dir(&game.0)
}

fn passes(game: &Game) -> String {
    fs::read_to_string(exports(game).join("unified-tweaks/passes.txt")).unwrap()
}

/// What runtime mods write while the game runs.
fn play_a_session(game: &Game, session: u32) {
    let e = exports(game);
    fs::create_dir_all(e.join("unified-tweaks-live")).unwrap();
    fs::write(
        e.join("unified-tweaks-live/level-limit.txt"),
        format!("{}", 25 + session),
    )
    .unwrap();
    fs::create_dir_all(e.join("weapon_holster")).unwrap();
    fs::write(
        e.join("weapon_holster/start.txt"),
        format!("start {session}"),
    )
    .unwrap();
    fs::write(e.join(format!("runtime-{session}.txt")), b"x").unwrap();
}

fn reused(game: &Game) -> bool {
    assert!(
        export_pass_needed(&game.0, "enshrouded").unwrap(),
        "a pregame exporter is enabled"
    );
    already_applied(&game.0, "enshrouded", true).unwrap()
}

#[test]
fn runtime_writes_into_the_export_folder_keep_the_last_pass() {
    let game = Game::new("exports-runtime");
    exporter(&game);
    runtime_exporter(&game);
    run_startup(&game.0).unwrap();
    assert_eq!(game.record()["status"], "applied", "{}", game.record());
    assert_eq!(passes(&game), "1");
    for session in 1..=3 {
        play_a_session(&game, session);
        assert!(
            reused(&game),
            "start {} after session {session}",
            session + 1
        );
        run_startup(&game.0).unwrap();
        assert_eq!(passes(&game), "1", "start {} reused the pass", session + 1);
    }
}

#[test]
fn an_exporters_output_or_input_changed_runs_the_pass_again() {
    let game = Game::new("exports-own");
    exporter(&game);
    run_startup(&game.0).unwrap();
    assert_eq!(passes(&game), "1");
    play_a_session(&game, 1);
    assert!(reused(&game));

    // its flat output deleted
    fs::remove_file(exports(&game).join("auto-loot-plus-report.txt")).unwrap();
    assert!(!reused(&game));
    run_startup(&game.0).unwrap();
    assert_eq!(passes(&game), "2");
    assert!(
        exports(&game).join("auto-loot-plus-report.txt").is_file(),
        "written again"
    );
    assert!(reused(&game));

    // an output in its folder changed
    fs::write(
        exports(&game).join("unified-tweaks/ut-reference.txt"),
        b"edited",
    )
    .unwrap();
    assert!(!reused(&game));
    run_startup(&game.0).unwrap();
    assert_eq!(passes(&game), "3");

    // the file it looked for and read appeared
    fs::write(
        exports(&game).join("unified-tweaks/user-settings.json"),
        b"{\"a\":1}",
    )
    .unwrap();
    assert!(!reused(&game));
    run_startup(&game.0).unwrap();
    assert_eq!(passes(&game), "4");
    assert_eq!(
        fs::read_to_string(exports(&game).join("unified-tweaks/ut-reference.txt")).unwrap(),
        "UTREF {\"a\":1}"
    );
    play_a_session(&game, 2);
    assert!(reused(&game));
}

/// Every export call a pregame mod can make is noted: what it copied, renamed and deleted, and the
/// folder it listed. A file added to that folder counts; one elsewhere does not.
#[test]
fn every_export_call_of_the_pass_is_in_the_record() {
    let game = Game::new("exports-calls");
    game.package(
        "scratcher",
        r#"
io.export("scratch/a.txt", "a")
io.export_copy("scratch/a.txt", "scratch/b.txt")
io.export_rename("scratch/b.txt", "scratch/c.txt")
io.export_delete("scratch/a.txt")
local names = io.export_list("scratch")
io.export("scratch/names.txt", table.concat(names, ","))
return {}
"#,
        &[],
    );
    let manifest = shroudforge_loader_config::paths::mods_dir(&game.0).join("scratcher/mod.json");
    fs::write(
        manifest,
        serde_json::to_vec(&json!({
            "id": "scratcher", "name": "Scratcher", "version": "1.0.0",
            "capabilities": ["patch", "export"]
        }))
        .unwrap(),
    )
    .unwrap();
    run_startup(&game.0).unwrap();
    assert_eq!(game.record()["status"], "applied", "{}", game.record());
    let path = shroudforge_game_assets::prepared_stamp::record_path(&game.0, "enshrouded");
    let record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let touched: Vec<(String, String, bool)> = record["exports"]["touched"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry[0].as_str().unwrap().to_owned(),
                entry[1].as_str().unwrap().to_owned(),
                !entry[2].is_null(),
            )
        })
        .collect();
    let expected = [
        ("path", "scratch/a.txt", false),
        ("path", "scratch/b.txt", false),
        ("path", "scratch/c.txt", true),
        ("path", "scratch/names.txt", true),
        ("listing", "scratch", true),
    ];
    assert_eq!(
        touched,
        expected
            .iter()
            .map(|(kind, path, there)| (kind.to_string(), path.to_string(), *there))
            .collect::<Vec<_>>(),
        "{record}"
    );
    play_a_session(&game, 1);
    assert!(reused(&game));
    fs::write(exports(&game).join("scratch/new.txt"), b"x").unwrap();
    assert!(!reused(&game), "a file added to the folder the pass listed");
}

/// The export API checks the export switch, not the package's capability, so a pregame package
/// with `patch` alone can read and write exports while exports are on. What it used there is in
/// the record too: a file it read that changed runs the pass again.
#[test]
fn a_patch_only_package_that_uses_the_export_api_is_recorded_too() {
    let game = Game::new("exports-patch-only");
    game.package(
        "reader",
        r#"
local value = "none"
if io.export_exists("reader/settings.txt") then
  value = io.read_export_to_string("reader/settings.txt")
end
io.export("reader/seen.txt", value)
return {}
"#,
        &[],
    );
    assert!(
        !export_pass_needed(&game.0, "enshrouded").unwrap(),
        "no package has the export capability"
    );
    run_startup(&game.0).unwrap();
    assert_eq!(game.record()["status"], "applied", "{}", game.record());
    let seen = || fs::read_to_string(exports(&game).join("reader/seen.txt")).unwrap();
    assert_eq!(seen(), "none");
    play_a_session(&game, 1);
    assert!(already_applied(&game.0, "enshrouded", false).unwrap());
    fs::write(exports(&game).join("reader/settings.txt"), b"on").unwrap();
    assert!(
        !already_applied(&game.0, "enshrouded", false).unwrap(),
        "the export the pass looked for appeared"
    );
    run_startup(&game.0).unwrap();
    assert_eq!(seen(), "on");
    play_a_session(&game, 2);
    assert!(already_applied(&game.0, "enshrouded", false).unwrap());
}
