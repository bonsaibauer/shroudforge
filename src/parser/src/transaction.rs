//! Recoverable publication for one parsed KFC file set.
//!
//! Mods only ever write into a private staging directory. `commit` publishes
//! the validated set; `recover` restores the previous complete set after an
//! interrupted publication.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn invalid() -> io::Error {
    io::Error::other("invalid ShroudForge asset transaction")
}

fn sync(path: &Path) -> io::Result<()> {
    fs::OpenOptions::new().write(true).open(path)?.sync_all()
}

pub fn directory(root: &Path, stem: &str) -> PathBuf {
    root.join(format!(".{stem}.shroudforge-stage"))
}

fn valid_name(stem: &str, name: &str) -> bool {
    name == format!("{stem}.kfc")
        || name == format!("{stem}.kfc_resources")
        || name
            .strip_prefix(&format!("{stem}_"))
            .and_then(|value| value.strip_suffix(".dat"))
            .is_some_and(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            })
}

fn records(stage: &Path, stem: &str) -> io::Result<Vec<(String, bool)>> {
    let records: Vec<(String, bool)> =
        serde_json::from_slice(&fs::read(stage.join("pending.json"))?).map_err(|_| invalid())?;
    if records.iter().any(|(name, _)| !valid_name(stem, name)) {
        return Err(invalid());
    }
    Ok(records)
}

fn clean(stage: &Path) -> io::Result<()> {
    if !stage.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(stage)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(invalid());
        }
        fs::remove_file(entry.path())?;
    }
    fs::remove_dir(stage)
}

fn replace(source: &Path, target: &Path) -> io::Result<()> {
    if target.exists() {
        fs::remove_file(target)?;
    }
    fs::rename(source, target)
}

pub fn recover(root: &Path, stem: &str) -> io::Result<()> {
    let stage = directory(root, stem);
    if stage.join("pending.json").is_file() {
        for (name, existed) in records(&stage, stem)? {
            let target = root.join(&name);
            if existed {
                let restore = stage.join(format!("{name}.restore"));
                fs::copy(stage.join(format!("{name}.rollback")), &restore)?;
                sync(&restore)?;
                replace(&restore, &target)?;
            } else if target.exists() {
                fs::remove_file(target)?;
            }
        }
        fs::rename(stage.join("pending.json"), stage.join("completed.json"))?;
    }
    clean(&stage)
}

pub fn begin(root: &Path, stem: &str) -> io::Result<PathBuf> {
    recover(root, stem)?;
    let stage = directory(root, stem);
    fs::create_dir(&stage)?;
    Ok(stage)
}

pub fn abort(root: &Path, stem: &str) -> io::Result<()> {
    recover(root, stem)
}

fn prepare(root: &Path, stem: &str) -> io::Result<Vec<(String, bool)>> {
    let stage = directory(root, stem);
    let mut files = Vec::new();
    for entry in fs::read_dir(&stage)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| invalid())?;
        if !entry.file_type()?.is_file() || !valid_name(stem, &name) {
            return Err(invalid());
        }
        files.push(name);
    }

    // Publish the index last, after every referenced data file is in place.
    files.sort_by_key(|name| (name.ends_with(".kfc"), name.clone()));
    let mut records = Vec::new();
    for name in files {
        sync(&stage.join(&name))?;
        let target = root.join(&name);
        let existed = target.is_file();
        if existed {
            let rollback = stage.join(format!("{name}.rollback"));
            fs::copy(&target, &rollback)?;
            sync(&rollback)?;
        }
        records.push((name, existed));
    }

    let journal = stage.join("journal.tmp");
    fs::write(
        &journal,
        serde_json::to_vec(&records).map_err(|_| invalid())?,
    )?;
    sync(&journal)?;
    fs::rename(journal, stage.join("pending.json"))?;
    Ok(records)
}

pub fn commit(root: &Path, stem: &str) -> io::Result<()> {
    let stage = directory(root, stem);
    let records = prepare(root, stem)?;
    let publish = (|| {
        for (name, _) in records {
            replace(&stage.join(&name), &root.join(&name))?;
        }
        fs::rename(stage.join("pending.json"), stage.join("completed.json"))?;
        Ok::<_, io::Error>(())
    })();

    if let Err(error) = publish {
        recover(root, stem)?;
        return Err(error);
    }
    clean(&stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("shroudforge-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("game.kfc"), b"old-index").unwrap();
        fs::write(root.join("game.kfc_resources"), b"old-data").unwrap();
        root
    }

    fn remove_fixture(root: &Path) {
        for name in ["game.kfc", "game.kfc_resources", "game_123.dat"] {
            let path = root.join(name);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn interrupted_publish_restores_previous_set() {
        let root = fixture();
        let stage = begin(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"new-data").unwrap();
        fs::write(stage.join("game_123.dat"), b"new-content").unwrap();
        prepare(&root, "game").unwrap();
        replace(
            &stage.join("game.kfc_resources"),
            &root.join("game.kfc_resources"),
        )
        .unwrap();
        replace(&stage.join("game_123.dat"), &root.join("game_123.dat")).unwrap();
        recover(&root, "game").unwrap();
        assert_eq!(fs::read(root.join("game.kfc")).unwrap(), b"old-index");
        assert_eq!(
            fs::read(root.join("game.kfc_resources")).unwrap(),
            b"old-data"
        );
        assert!(!root.join("game_123.dat").exists());
        remove_fixture(&root);
    }

    #[test]
    fn commit_publishes_complete_set() {
        let root = fixture();
        let stage = begin(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"new-data").unwrap();
        commit(&root, "game").unwrap();
        assert_eq!(fs::read(root.join("game.kfc")).unwrap(), b"new-index");
        assert_eq!(
            fs::read(root.join("game.kfc_resources")).unwrap(),
            b"new-data"
        );
        assert!(!stage.exists());
        remove_fixture(&root);
    }
}
