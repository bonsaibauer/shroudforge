//! The record of what the last startup asset pass left in the game folder.
//!
//! A start skipped the asset pass when state.json "assets" said `applied` with the current
//! preparation fingerprint (the enabled pregame packages, their files, the executable's size and
//! time). Two gaps made that guard both too weak and too strict:
//! - Nothing checked the game's own files. Another loader, Steam's file check or a restore could
//!   replace `<stem>.kfc` after the pass, and the record still said `applied`.
//! - While exports were on (the default), a pregame package with the `export` capability forced
//!   the whole restore-and-commit on every start, because nothing said whether what it exported
//!   was still there. That cost over two minutes per start on a Steam Deck, and 11 minutes with
//!   the EMF Beastmaster packs.
//!
//! This record closes both: it is written next to the working backup (`<stem>.kfc.prepared.json`)
//! after a pass succeeded, removed before a pass starts, and the pass is skipped only when
//! everything in it is still what it was:
//! - the preparation fingerprint and the export switch the pass ran with,
//! - Steam's build id of the installation, when Steam's manifest is there,
//! - `<stem>.kfc` (size, time, SHA-256 of all of it: about 9 MB) and `<stem>.kfc_resources` (size,
//!   time, SHA-256 of its first and last MiB: about 200 MB is too much to read every start),
//! - both working backups (size and time),
//! - while exports are on: what the pass's packages used in the export folder, and nothing else
//!   there. The pass notes every path it wrote, read, looked for, renamed, copied or deleted
//!   through the export API, and every folder it listed (`ExportTouches`); the record keeps each
//!   one's state: a file's size and SHA-256 (of its first and last MiB plus its time when it is
//!   over 16 MiB), a folder, nothing there, or a listed folder's names; a link by where it leads
//!   and what is there, as the export API follows links. A pass that used a path the record cannot
//!   name leaves no record. Pregame exporters are not scoped to a folder of their own: they write
//!   flat files at the top of the export folder (XHL Area Harvest, Auto Loot Plus) as well as
//!   folders (Unified EML Tweaks), and runtime mods write next to them while the game runs
//!   (Unified Tweaks Live, World Editor, native DLLs, diagnostics). A record of the whole export
//!   folder would run the whole pass again after any such runtime write.
//!
//! Anything that cannot be read, a record of another scheme, or any difference means "apply".

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::UNIX_EPOCH,
};

const SCHEME: &str = "shroudforge-prepared 1";
/// The part of `<stem>.kfc_resources` hashed at each end.
const EDGE: u64 = 1 << 20;
/// A pass that used more paths of the export folder than this is not recorded: the next start
/// runs the pass as before.
const MAX_EXPORT_ENTRIES: usize = 50_000;
/// An export file larger than this is known by its size, time and the SHA-256 of its first and
/// last `EDGE` bytes instead of all of it.
const WHOLE_EXPORT_HASH: u64 = 16 << 20;

/// What the pass ran with, besides the game files.
pub struct PassInputs<'a> {
    /// `shroudforge_package::prepared::fingerprint` of the plan the pass applied.
    pub fingerprint: &'a str,
    /// loader config `exports.enabled` at the pass.
    pub exports_enabled: bool,
    /// The export folder, while exports are on (`exports.enabled`). Every pregame package may use
    /// the export API then, with the `export` capability or without it (the API checks the switch).
    /// Only what the pass used there (`ExportTouches`) is recorded and compared.
    pub exporters: Option<&'a Path>,
}

/// How the pass used a path below the export folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Touch {
    /// A file the pass wrote, read, looked for, renamed, copied or deleted.
    Path,
    /// A folder the pass listed.
    Listing,
}

/// What the startup pass did in the export folder through ShroudForge's export API (`io.export`,
/// `io.export_exists`, `io.read_export_to_string`, `io.export_list`, `io.export_rename`,
/// `io.export_copy`, `io.export_delete`): the paths relative to the export folder, `/` between
/// their parts. The loader hands one to the pass (`RunOptions::export_touches`) and records it.
#[derive(Clone, Debug, Default)]
pub struct ExportTouches(Arc<Mutex<Noted>>);

#[derive(Debug, Default)]
struct Noted {
    touched: BTreeSet<(Touch, String)>,
    /// The pass used a path the record cannot name (a part with `:`, such as an NTFS
    /// stream): `record` then writes no record, so the next start runs the pass.
    unnamed: bool,
}

impl ExportTouches {
    /// A path the pass wrote, read, looked for, renamed, copied or deleted.
    pub fn path(&self, relative: &str) {
        self.add(Touch::Path, relative);
    }

    /// A folder the pass listed (`""` is the export folder itself).
    pub fn listing(&self, relative: &str) {
        self.add(Touch::Listing, relative);
    }

    fn add(&self, touch: Touch, relative: &str) {
        let mut noted = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match normalized(relative) {
            Some(relative) => {
                noted.touched.insert((touch, relative));
            }
            None => noted.unnamed = true,
        }
    }

    /// Everything noted so far.
    pub fn touched(&self) -> BTreeSet<(Touch, String)> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .touched
            .clone()
    }

    /// Whether the pass used a path that is not in `touched` because the record cannot name it.
    fn unnamed(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unnamed
    }
}

/// A relative path's parts joined with `/`; None for a path that leaves the export folder.
fn normalized(relative: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            part if part.contains(':') => return None,
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// `<game>/<stem>.kfc.prepared.json`, beside `<stem>.kfc.bak`.
pub fn record_path(root: &Path, file_name: &str) -> PathBuf {
    root.join(format!("{file_name}.kfc.prepared.json"))
}

/// Removes the record. Called before a pass changes the game files, so an interrupted pass never
/// leaves a record that describes files that are no longer there.
pub fn forget(root: &Path, file_name: &str) -> Result<(), String> {
    match fs::remove_file(record_path(root, file_name)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "{}: {error}",
            record_path(root, file_name).display()
        )),
    }
}

/// Writes the record of the files as they are now (after a successful pass), with what the pass
/// used in the export folder (`touches`, read only when `inputs.exporters` is set). Flushed and
/// renamed like the state file.
pub fn record(
    root: &Path,
    file_name: &str,
    inputs: &PassInputs,
    touches: &ExportTouches,
) -> Result<(), String> {
    let touched = touches.touched();
    if touches.unnamed() {
        return Err("the pass used a path of the export folder the record cannot name".into());
    }
    if touched.len() > MAX_EXPORT_ENTRIES {
        return Err(format!(
            "the pass used more than {MAX_EXPORT_ENTRIES} paths of the export folder"
        ));
    }
    let stamp = capture(root, file_name, inputs, &touched)?;
    crate::config::write_json(&record_path(root, file_name), &stamp)
}

/// `Ok` when the record is there and every part of it still holds; otherwise `Err` says what did
/// not. Whatever cannot be read or checked is an `Err` too: the caller then applies the asset mods
/// as before.
pub fn current(root: &Path, file_name: &str, inputs: &PassInputs) -> Result<(), String> {
    let path = record_path(root, file_name);
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let recorded: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    if recorded["scheme"] != SCHEME {
        return Err("the record is of another scheme".into());
    }
    // the paths the last pass used in the export folder, to be looked at again
    let touched = match (&inputs.exporters, recorded_touches(&recorded["exports"])) {
        (None, _) => BTreeSet::new(),
        (Some(_), Some(touched)) => touched,
        (Some(_), None) => return Err("exports differs from the last pass".into()),
    };
    let now = capture(root, file_name, inputs, &touched)?;
    for key in [
        "scheme",
        "target",
        "fingerprint",
        "exportsEnabled",
        "build",
        "files",
        "exports",
    ] {
        if recorded.get(key) != now.get(key) {
            return Err(format!("{key} differs from the last pass"));
        }
    }
    Ok(())
}

fn capture(
    root: &Path,
    file_name: &str,
    inputs: &PassInputs,
    touched: &BTreeSet<(Touch, String)>,
) -> Result<Value, String> {
    let file = |suffix: &str| root.join(format!("{file_name}.{suffix}"));
    Ok(json!({
        "scheme": SCHEME,
        "target": file_name,
        "fingerprint": inputs.fingerprint,
        "exportsEnabled": inputs.exports_enabled,
        "build": steam_build(root, file_name),
        "files": {
            "kfc": with_hash(&file("kfc"), None)?,
            "kfcResources": with_hash(&file("kfc_resources"), Some(EDGE))?,
            "kfcBackup": identity(&file("kfc.bak"))?,
            "kfcResourcesBackup": identity(&file("kfc_resources.bak"))?,
        },
        "exports": match inputs.exporters {
            Some(dir) => json!({"touched": export_state(dir, touched)?}),
            None => Value::Null,
        },
    }))
}

fn modified_nanos(metadata: &fs::Metadata) -> Result<String, String> {
    let time = metadata.modified().map_err(|error| error.to_string())?;
    // Before 1970 or unreadable: no stamp to trust.
    let since = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    Ok(since.as_nanos().to_string())
}

/// Size and last-write time of a regular file; a missing file is recorded as missing.
fn identity(path: &Path) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(json!({
            "size": metadata.len(),
            "modified": modified_nanos(&metadata)?,
        })),
        Ok(_) => Err(format!("{} is not a regular file", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// `identity` plus a SHA-256 of the whole file, or of its first and last `edge` bytes.
fn with_hash(path: &Path, edge: Option<u64>) -> Result<Value, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let mut file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut hash = Sha256::new();
    let mut take = |file: &mut fs::File, mut left: u64| -> Result<(), String> {
        let mut buffer = vec![0u8; 1 << 16];
        while left > 0 {
            let want = buffer
                .len()
                .min(usize::try_from(left).unwrap_or(usize::MAX));
            let count = file
                .read(&mut buffer[..want])
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            left -= count as u64;
        }
        Ok(())
    };
    let length = metadata.len();
    match edge {
        Some(edge) if length > edge * 2 => {
            take(&mut file, edge)?;
            file.seek(SeekFrom::End(-(edge as i64)))
                .map_err(|error| format!("{}: {error}", path.display()))?;
            take(&mut file, edge)?;
        }
        _ => take(&mut file, length)?,
    }
    // The file must not have changed while it was read.
    let after = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if after.len() != length || modified_nanos(&after)? != modified_nanos(&metadata)? {
        return Err(format!("{} changed while it was read", path.display()));
    }
    Ok(json!({
        "size": length,
        "modified": modified_nanos(&metadata)?,
        "sha256": format!("{:x}", hash.finalize()),
    }))
}

/// The (kind, path) pairs of a record's `exports.touched`; None when it has none (a record of a
/// pass without pregame exporters) or it is not that shape.
fn recorded_touches(exports: &Value) -> Option<BTreeSet<(Touch, String)>> {
    let mut touched = BTreeSet::new();
    for entry in exports.get("touched")?.as_array()? {
        let touch = match entry.get(0)?.as_str()? {
            "path" => Touch::Path,
            "listing" => Touch::Listing,
            _ => return None,
        };
        touched.insert((touch, normalized(entry.get(1)?.as_str()?)?));
    }
    Some(touched)
}

/// For each path the pass used, sorted: [kind, path, state now].
fn export_state(dir: &Path, touched: &BTreeSet<(Touch, String)>) -> Result<Vec<Value>, String> {
    touched
        .iter()
        .map(|(touch, relative)| {
            let path = relative
                .split('/')
                .filter(|part| !part.is_empty())
                .fold(dir.to_path_buf(), |path, part| path.join(part));
            Ok(match touch {
                Touch::Path => json!(["path", relative, path_state(&path)?]),
                Touch::Listing => json!(["listing", relative, listing_state(&path)?]),
            })
        })
        .collect()
}

/// A file's size and SHA-256 (no time: a runtime mod or the pass writing the same bytes again is no
/// change), `"folder"`, a link by what it leads to (`linked`), `"other"`, or null when nothing is
/// there.
fn path_state(path: &Path) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => linked(path, path_state),
        Ok(metadata) if metadata.is_file() => {
            let large = metadata.len() > WHOLE_EXPORT_HASH;
            let hashed = with_hash(path, large.then_some(EDGE))?;
            Ok(if large {
                hashed
            } else {
                json!({"size": hashed["size"], "sha256": hashed["sha256"]})
            })
        }
        Ok(metadata) if metadata.is_dir() => Ok(Value::from("folder")),
        Ok(_) => Ok(Value::from("other")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// The export API follows links (`io.read_export_to_string` and `io.export_list` resolve them), so
/// a link is known by where it leads and the state of what is there; null where it leads nowhere.
/// The resolved path holds no link, so `state` does not come back here.
fn linked(path: &Path, state: fn(&Path) -> Result<Value, String>) -> Result<Value, String> {
    match fs::canonicalize(path) {
        Ok(target) => Ok(json!({"link": target.to_string_lossy(), "state": state(&target)?})),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({"link": null})),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// The sorted names of a listed folder's entries, with their kind (a link with where it leads, as
/// `io.export_list` names it by that); null when it is not there, `"not a folder"` when something
/// else is, and a link to a folder by what it leads to (`linked`).
fn listing_state(path: &Path) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return linked(path, listing_state),
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok(Value::from("not a folder")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", path.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        names.push(match entry.file_type() {
            Ok(kind) if kind.is_symlink() => {
                let target = fs::canonicalize(entry.path())
                    .ok()
                    .map(|target| target.to_string_lossy().into_owned());
                json!([name, "link", target])
            }
            Ok(kind) if kind.is_dir() => json!([name, "folder"]),
            Ok(kind) if kind.is_file() => json!([name, "file"]),
            Ok(_) => json!([name, "other"]),
            Err(error) => return Err(format!("{}: {error}", entry.path().display())),
        });
        if names.len() > MAX_EXPORT_ENTRIES {
            return Err(format!(
                "{} has more than {MAX_EXPORT_ENTRIES} entries",
                path.display()
            ));
        }
    }
    names.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    Ok(Value::from(names))
}

/// Steam's build id of the installation (`buildid` in `steamapps/appmanifest_<app>.acf`), or null
/// where Steam's manifest is not there (a server installed without Steam).
fn steam_build(root: &Path, file_name: &str) -> Value {
    let app = if file_name == "enshrouded_server" {
        "2278520"
    } else {
        "1203620"
    };
    let Some(steamapps) = root.parent().and_then(Path::parent) else {
        return Value::Null;
    };
    let Ok(text) = fs::read_to_string(steamapps.join(format!("appmanifest_{app}.acf"))) else {
        return Value::Null;
    };
    text.lines()
        .find_map(|line| {
            let mut parts = line.split('"').filter(|part| !part.trim().is_empty());
            (parts.next()?.eq_ignore_ascii_case("buildid"))
                .then(|| parts.next().map(str::to_owned))
                .flatten()
        })
        .map(Value::from)
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Game {
        _temp: TempDir,
        root: PathBuf,
        exports: PathBuf,
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A game folder after a pass, with the export folder of a player's report: pregame exporters
    /// wrote flat files at the export folder's top (XHL Area Harvest, Auto Loot Plus) and a folder
    /// of their own (Unified EML Tweaks: `unified-tweaks/`); runtime mods write beside them while
    /// the game runs (Unified Tweaks Live: `unified-tweaks-live/`, World Editor, a native DLL).
    fn game() -> Game {
        let base = std::env::temp_dir().join(format!("sf-prepared-{}", uuid::Uuid::new_v4()));
        let root = base.join("steamapps/common/Enshrouded");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            base.join("steamapps/appmanifest_1203620.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1203620\"\n\t\"buildid\"\t\t\"26430625\"\n}\n",
        )
        .unwrap();
        fs::write(root.join("enshrouded.kfc"), b"patched header").unwrap();
        fs::write(
            root.join("enshrouded.kfc_resources"),
            vec![7u8; 3 * EDGE as usize],
        )
        .unwrap();
        fs::write(root.join("enshrouded.kfc.bak"), b"original header").unwrap();
        fs::write(
            root.join("enshrouded.kfc_resources.bak"),
            b"original resources",
        )
        .unwrap();
        let exports = root.join("shroudforge/exports");
        fs::create_dir_all(exports.join("unified-tweaks")).unwrap();
        fs::create_dir_all(exports.join("unified-tweaks-live")).unwrap();
        fs::write(
            exports.join("xhl-area-harvest-family-manifest.tsv"),
            b"family\trows",
        )
        .unwrap();
        fs::write(
            exports.join("auto-loot-plus-report.txt"),
            b"boss:field\tlabel",
        )
        .unwrap();
        fs::write(exports.join("unified-tweaks/ut-reference.txt"), b"UTREF 1").unwrap();
        fs::write(
            exports.join("unified-tweaks/user-settings.json"),
            b"{\"a\":1}",
        )
        .unwrap();
        fs::write(exports.join("unified-tweaks-live/level-limit.txt"), b"25").unwrap();
        Game {
            _temp: TempDir(base),
            root,
            exports,
        }
    }

    /// What the pregame exporters did in that pass: wrote three files, read one, looked for one
    /// that was not there, listed their own folder.
    fn pass_touches() -> ExportTouches {
        let touches = ExportTouches::default();
        touches.path("xhl-area-harvest-family-manifest.tsv");
        touches.path("auto-loot-plus-report.txt");
        touches.path("unified-tweaks/ut-reference.txt");
        touches.path("unified-tweaks/user-settings.json");
        touches.path("unified-tweaks/settings-last-launch.json");
        touches.listing("unified-tweaks");
        touches
    }

    fn inputs<'a>(fingerprint: &'a str, exporters: Option<&'a Path>) -> PassInputs<'a> {
        PassInputs {
            fingerprint,
            exports_enabled: true,
            exporters,
        }
    }

    fn none() -> ExportTouches {
        ExportTouches::default()
    }

    /// Rewrites a file with the same length and puts its last-write time back: only the hash can
    /// tell.
    fn rewrite_keeping_time(path: &Path, bytes: &[u8]) {
        let time = fs::metadata(path).unwrap().modified().unwrap();
        fs::write(path, bytes).unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    fn later(path: &Path) {
        let time =
            fs::metadata(path).unwrap().modified().unwrap() + std::time::Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    /// What runtime mods write into the export folder while the game runs, none of it through the
    /// pregame pass: new flat files at the top, files in their own folders (new and existing), a
    /// native DLL's folder, World Editor's state, diagnostics.
    fn play_a_session(g: &Game, session: u32) {
        let e = &g.exports;
        fs::write(
            e.join("unified-tweaks-live/level-limit.txt"),
            format!("{}", 25 + session),
        )
        .unwrap();
        fs::write(
            e.join("unified-tweaks-live/enemy-scale.txt"),
            format!("scale {session}"),
        )
        .unwrap();
        fs::create_dir_all(e.join("weapon_holster")).unwrap();
        fs::write(
            e.join("weapon_holster/start.txt"),
            format!("start {session}"),
        )
        .unwrap();
        fs::create_dir_all(e.join("world-editor/blueprints")).unwrap();
        fs::write(
            e.join("world-editor/editor-state.txt"),
            format!("state {session}"),
        )
        .unwrap();
        fs::write(e.join(format!("runtime-counter-{session}.txt")), b"x").unwrap();
        fs::create_dir_all(e.join("diagnostics")).unwrap();
        fs::write(e.join("diagnostics/capture.runtime"), format!("{session}")).unwrap();
    }

    #[test]
    fn an_unchanged_game_folder_matches_the_record() {
        let g = game();
        let exports = g.exports.clone();
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &pass_touches(),
        )
        .unwrap();
        assert_eq!(
            current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
            Ok(())
        );
        let recorded: Value =
            serde_json::from_slice(&fs::read(record_path(&g.root, "enshrouded")).unwrap()).unwrap();
        assert_eq!(recorded["build"], "26430625");
        assert_eq!(recorded["scheme"], SCHEME);
    }

    #[test]
    fn no_record_or_a_forgotten_record_means_apply() {
        let g = game();
        assert!(current(&g.root, "enshrouded", &inputs("f1", None)).is_err());
        record(&g.root, "enshrouded", &inputs("f1", None), &none()).unwrap();
        forget(&g.root, "enshrouded").unwrap();
        assert!(current(&g.root, "enshrouded", &inputs("f1", None)).is_err());
        forget(&g.root, "enshrouded").unwrap();
    }

    /// The player's case: runtime mods' writes into the export folder are no reason for a pass.
    #[test]
    fn runtime_writes_under_exports_by_other_mods_do_not_count() {
        let g = game();
        let exports = g.exports.clone();
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &pass_touches(),
        )
        .unwrap();
        play_a_session(&g, 1);
        // a runtime mod's folder that was there at the pass, its file touched again
        later(&g.exports.join("unified-tweaks-live/level-limit.txt"));
        assert_eq!(
            current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
            Ok(()),
            "runtime writes outside what the pregame pass used"
        );
    }

    /// Each start checks the record and, when it holds, leaves it as it is; sessions in between
    /// write into the export folder. Three starts, one pass.
    #[test]
    fn the_record_stays_current_across_starts_with_sessions_between() {
        let g = game();
        let exports = g.exports.clone();
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &pass_touches(),
        )
        .unwrap();
        let written = fs::read(record_path(&g.root, "enshrouded")).unwrap();
        for session in 1..=3 {
            play_a_session(&g, session);
            assert_eq!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
                Ok(()),
                "start {} after session {session}",
                session + 1
            );
            assert!(
                fs::read(record_path(&g.root, "enshrouded")).unwrap() == written,
                "a check never rewrites the record"
            );
        }
    }

    /// What the pregame exporters wrote, read or looked for, changed or deleted, means a pass.
    #[test]
    fn an_exporters_own_output_changed_or_deleted_counts() {
        type Change = fn(&Game);
        let changes: [(&str, Change); 9] = [
            ("a flat output rewritten", |g| {
                fs::write(
                    g.exports.join("xhl-area-harvest-family-manifest.tsv"),
                    b"other rows",
                )
                .unwrap()
            }),
            ("a flat output deleted", |g| {
                fs::remove_file(g.exports.join("auto-loot-plus-report.txt")).unwrap()
            }),
            ("an output rewritten with the same length and time", |g| {
                rewrite_keeping_time(
                    &g.exports.join("unified-tweaks/ut-reference.txt"),
                    b"UTREF 2",
                )
            }),
            ("an output in the exporter's folder deleted", |g| {
                fs::remove_file(g.exports.join("unified-tweaks/ut-reference.txt")).unwrap()
            }),
            ("a file the pass read changed", |g| {
                fs::write(
                    g.exports.join("unified-tweaks/user-settings.json"),
                    b"{\"a\":2}",
                )
                .unwrap()
            }),
            (
                "a file the pass looked for and did not find appeared",
                |g| {
                    fs::write(
                        g.exports.join("unified-tweaks/settings-last-launch.json"),
                        b"{}",
                    )
                    .unwrap()
                },
            ),
            ("a file added to a folder the pass listed", |g| {
                fs::write(g.exports.join("unified-tweaks/new.txt"), b"x").unwrap()
            }),
            ("an output replaced by a folder", |g| {
                fs::remove_file(g.exports.join("auto-loot-plus-report.txt")).unwrap();
                fs::create_dir(g.exports.join("auto-loot-plus-report.txt")).unwrap()
            }),
            ("the whole export folder deleted", |g| {
                fs::remove_dir_all(&g.exports).unwrap()
            }),
        ];
        for (what, change) in changes {
            let g = game();
            let exports = g.exports.clone();
            record(
                &g.root,
                "enshrouded",
                &inputs("f1", Some(&exports)),
                &pass_touches(),
            )
            .unwrap();
            play_a_session(&g, 1);
            assert_eq!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
                Ok(())
            );
            change(&g);
            let result = current(&g.root, "enshrouded", &inputs("f1", Some(&exports)));
            assert_eq!(
                result,
                Err("exports differs from the last pass".to_owned()),
                "{what}: must apply again"
            );
        }
    }

    #[test]
    fn any_change_means_apply() {
        type Change = fn(&Game);
        let changes: [(&str, Change); 9] = [
            ("kfc rewritten, same length and time", |g| {
                rewrite_keeping_time(&g.root.join("enshrouded.kfc"), b"vanilla header")
            }),
            ("kfc touched", |g| later(&g.root.join("enshrouded.kfc"))),
            ("kfc removed", |g| {
                fs::remove_file(g.root.join("enshrouded.kfc")).unwrap()
            }),
            ("resources start rewritten", |g| {
                let path = g.root.join("enshrouded.kfc_resources");
                let mut bytes = fs::read(&path).unwrap();
                bytes[0] = 8;
                rewrite_keeping_time(&path, &bytes)
            }),
            ("resources replaced", |g| {
                fs::write(g.root.join("enshrouded.kfc_resources"), b"vanilla").unwrap()
            }),
            ("backup replaced", |g| {
                fs::write(g.root.join("enshrouded.kfc.bak"), b"a newer original").unwrap()
            }),
            ("resource backup moved aside", |g| {
                fs::remove_file(g.root.join("enshrouded.kfc_resources.bak")).unwrap()
            }),
            ("game build", |g| {
                let manifest = g.root.join("../../appmanifest_1203620.acf");
                let text = fs::read_to_string(&manifest)
                    .unwrap()
                    .replace("26430625", "26500000");
                fs::write(manifest, text).unwrap()
            }),
            ("record of another scheme", |g| {
                let path = record_path(&g.root, "enshrouded");
                let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["scheme"] = json!("shroudforge-prepared 0");
                fs::write(path, value.to_string()).unwrap()
            }),
        ];
        for (what, change) in changes {
            let g = game();
            let exports = g.exports.clone();
            record(
                &g.root,
                "enshrouded",
                &inputs("f1", Some(&exports)),
                &pass_touches(),
            )
            .unwrap();
            change(&g);
            assert!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))).is_err(),
                "{what}: must apply again"
            );
        }
    }

    #[test]
    fn other_pass_inputs_mean_apply() {
        let g = game();
        let exports = g.exports.clone();
        record(&g.root, "enshrouded", &inputs("f1", None), &none()).unwrap();
        assert_eq!(current(&g.root, "enshrouded", &inputs("f1", None)), Ok(()));
        // another package plan or package file
        assert!(current(&g.root, "enshrouded", &inputs("f2", None)).is_err());
        // a pregame exporter now (the record has no export state)
        assert!(current(&g.root, "enshrouded", &inputs("f1", Some(&exports))).is_err());
        // exports switched off
        let off = PassInputs {
            fingerprint: "f1",
            exports_enabled: false,
            exporters: None,
        };
        assert!(current(&g.root, "enshrouded", &off).is_err());
        // the server's record is its own
        assert!(current(&g.root, "enshrouded_server", &inputs("f1", None)).is_err());
        // no pregame exporter any more (the record has an export state)
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &pass_touches(),
        )
        .unwrap();
        assert!(current(&g.root, "enshrouded", &inputs("f1", None)).is_err());
    }

    #[test]
    fn exports_outside_a_pregame_exporter_pass_do_not_count() {
        let g = game();
        record(&g.root, "enshrouded", &inputs("f1", None), &none()).unwrap();
        fs::write(g.exports.join("world-editor-capture.sfbp"), b"x").unwrap();
        assert_eq!(current(&g.root, "enshrouded", &inputs("f1", None)), Ok(()));
    }

    /// An exporter that used nothing in the export folder records an empty state, which holds.
    #[test]
    fn an_exporter_pass_that_used_no_export_is_current() {
        let g = game();
        let exports = g.exports.clone();
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &none(),
        )
        .unwrap();
        play_a_session(&g, 1);
        fs::remove_file(g.exports.join("auto-loot-plus-report.txt")).unwrap();
        assert_eq!(
            current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
            Ok(())
        );
    }

    #[test]
    fn touches_keep_relative_paths_inside_the_export_folder() {
        let touches = ExportTouches::default();
        touches.path("./a\\b//c.txt");
        touches.path("../outside.txt");
        touches.path("C:/outside.txt");
        touches.listing("");
        assert_eq!(
            touches.touched().into_iter().collect::<Vec<_>>(),
            vec![
                (Touch::Path, "a/b/c.txt".to_owned()),
                (Touch::Listing, String::new())
            ]
        );
    }

    /// A path the record cannot name (a part with `:`, such as an NTFS stream `file:stream`, which
    /// ShroudForge's own path check lets through) leaves no record, so the next start runs the
    /// pass: what the pass used is never left out of the record.
    #[test]
    fn a_path_the_record_cannot_name_leaves_no_record() {
        let g = game();
        let exports = g.exports.clone();
        let touches = pass_touches();
        touches.path("unified-tweaks/ut-reference.txt:stream");
        assert!(
            record(
                &g.root,
                "enshrouded",
                &inputs("f1", Some(&exports)),
                &touches
            )
            .is_err(),
            "a pass that used a path the record cannot name is not recorded"
        );
        assert!(current(&g.root, "enshrouded", &inputs("f1", Some(&exports))).is_err());
        // a copy of the note shares it
        let copy = touches.clone();
        let fresh = ExportTouches::default();
        fresh.path("a.txt");
        assert!(record(&g.root, "enshrouded", &inputs("f1", Some(&exports)), &copy).is_err());
        assert!(record(&g.root, "enshrouded", &inputs("f1", Some(&exports)), &fresh).is_ok());
    }

    /// The export API follows links (`io.read_export_to_string` and
    /// `io.export_list` resolve them), so a link the pass used is known by what it leads to: a file
    /// read through a link, a folder listed through a link, a listed folder's entry that is a link.
    #[cfg(unix)]
    #[test]
    fn a_link_the_pass_used_is_known_by_what_it_leads_to() {
        use std::os::unix::fs::symlink;
        type Change = fn(&Game);
        let changes: [(&str, Change); 4] = [
            ("the file a link the pass read leads to changed", |g| {
                fs::write(
                    g.exports.join("unified-tweaks/user-settings.json"),
                    b"{\"a\":2}",
                )
                .unwrap()
            }),
            ("a file added to the folder a listed link leads to", |g| {
                fs::write(g.exports.join("unified-tweaks/new.txt"), b"x").unwrap()
            }),
            ("an entry of a listed folder links elsewhere", |g| {
                fs::remove_file(g.exports.join("linked/alias")).unwrap();
                symlink(
                    "../xhl-area-harvest-family-manifest.tsv",
                    g.exports.join("linked/alias"),
                )
                .unwrap()
            }),
            ("the link the pass read leads nowhere", |g| {
                fs::remove_file(g.exports.join("unified-tweaks/user-settings.json")).unwrap()
            }),
        ];
        for (what, change) in changes {
            let g = game();
            let exports = g.exports.clone();
            symlink(
                "unified-tweaks/user-settings.json",
                exports.join("settings-link.json"),
            )
            .unwrap();
            symlink("unified-tweaks", exports.join("tweaks-link")).unwrap();
            fs::create_dir_all(exports.join("linked")).unwrap();
            symlink("../auto-loot-plus-report.txt", exports.join("linked/alias")).unwrap();
            let touches = ExportTouches::default();
            touches.path("settings-link.json");
            touches.listing("tweaks-link");
            touches.listing("linked");
            record(
                &g.root,
                "enshrouded",
                &inputs("f1", Some(&exports)),
                &touches,
            )
            .unwrap();
            play_a_session(&g, 1);
            assert_eq!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
                Ok(()),
                "{what}: unchanged links hold"
            );
            change(&g);
            assert_eq!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
                Err("exports differs from the last pass".to_owned()),
                "{what}: must apply again"
            );
        }
    }

    /// An export folder that is a link (moved to another drive) and that
    /// the pass listed: a file added at its top runs the pass.
    #[cfg(unix)]
    #[test]
    fn a_linked_export_folder_that_the_pass_listed() {
        let g = game();
        let moved = g.root.join("../exports-elsewhere");
        fs::rename(&g.exports, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &g.exports).unwrap();
        let exports = g.exports.clone();
        let touches = ExportTouches::default();
        touches.listing("");
        touches.path("");
        record(
            &g.root,
            "enshrouded",
            &inputs("f1", Some(&exports)),
            &touches,
        )
        .unwrap();
        assert_eq!(
            current(&g.root, "enshrouded", &inputs("f1", Some(&exports))),
            Ok(())
        );
        fs::write(moved.join("new-at-the-top.txt"), b"x").unwrap();
        assert!(
            current(&g.root, "enshrouded", &inputs("f1", Some(&exports))).is_err(),
            "a file added to the listed export folder"
        );
    }

    /// An export file over 16 MiB is known by its size, time and the SHA-256 of its first and last
    /// MiB: a change at either end or a new time runs the pass.
    #[test]
    fn a_large_export_is_known_by_its_size_time_and_ends() {
        type Change = fn(&Path);
        let changes: [(&str, Change); 3] = [
            ("its first byte, same time", |path| {
                let mut bytes = fs::read(path).unwrap();
                bytes[0] = 2;
                rewrite_keeping_time(path, &bytes)
            }),
            ("its last byte, same time", |path| {
                let mut bytes = fs::read(path).unwrap();
                *bytes.last_mut().unwrap() = 2;
                rewrite_keeping_time(path, &bytes)
            }),
            ("a new time", |path| later(path)),
        ];
        for (what, change) in changes {
            let g = game();
            let exports = g.exports.clone();
            let big = exports.join("unified-tweaks/big.bin");
            fs::write(&big, vec![1u8; (WHOLE_EXPORT_HASH + 1) as usize]).unwrap();
            let touches = pass_touches();
            touches.path("unified-tweaks/big.bin");
            record(
                &g.root,
                "enshrouded",
                &inputs("f1", Some(&exports)),
                &touches,
            )
            .unwrap();
            let recorded: Value =
                serde_json::from_slice(&fs::read(record_path(&g.root, "enshrouded")).unwrap())
                    .unwrap();
            let state = recorded["exports"]["touched"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry[1] == "unified-tweaks/big.bin")
                .unwrap()[2]
                .clone();
            assert!(state["modified"].is_string(), "{state}");
            assert_eq!(state["size"], WHOLE_EXPORT_HASH + 1);
            change(&big);
            assert!(
                current(&g.root, "enshrouded", &inputs("f1", Some(&exports))).is_err(),
                "{what}: must apply again"
            );
        }
    }

    #[test]
    fn a_missing_steam_manifest_is_no_build_on_both_sides() {
        let g = game();
        fs::remove_file(g.root.join("../../appmanifest_1203620.acf")).unwrap();
        record(&g.root, "enshrouded", &inputs("f1", None), &none()).unwrap();
        assert_eq!(current(&g.root, "enshrouded", &inputs("f1", None)), Ok(()));
    }
}
