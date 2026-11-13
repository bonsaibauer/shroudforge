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
    if stage.join(PREPARED_MARKER).exists() {
        return Err(non_publishable_prepared());
    }
    if stage.join("generated-pending.json").exists() {
        return recover_generated(root, stem);
    }
    if stage.join("generated-inputs.json").exists() {
        generated_root(root, stem)?;
        generated_inventory(&stage, stem)?;
        return clean(&stage);
    }
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

// Generated-output publication deliberately has forward recovery, not baseline
// rollback. The caller must hold the startup lease and validate the generated
// index semantically before committing. No original asset is retained or copied
// by this mode. Existing baseline transactions retain their original behavior.
type Stamp = (u64, String);
type GeneratedRecord = (String, Option<Stamp>, Stamp);
type GeneratedJournal = (u32, String, Vec<GeneratedRecord>);
type GeneratedInputs = (u32, String, Stamp, Stamp);
type PreparedMarker = (u32, String, String);

const PREPARED_MARKER: &str = "prepared-only.json";

fn non_publishable_prepared() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "prepared generated assets cannot be published or recovered",
    )
}

fn regular_file(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(invalid());
                }
            }
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid());
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn stamp(path: &Path) -> io::Result<Stamp> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    if !regular_file(path)? {
        return Err(invalid());
    }
    let mut input = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

fn generated_root(root: &Path, stem: &str) -> io::Result<String> {
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(invalid());
    }
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(invalid());
    }
    root.to_str().map(str::to_owned).ok_or_else(invalid)
}

fn generated_inventory(stage: &Path, stem: &str) -> io::Result<Vec<String>> {
    let metadata = fs::symlink_metadata(stage)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid());
        }
    }
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(stage)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| invalid())?;
        if !regular_file(&entry.path())? {
            return Err(invalid());
        }
        if !valid_name(stem, &name)
            && !matches!(
                name.as_str(),
                "generated-inputs.json"
                    | "generated-pending.json"
                    | "generated-journal.tmp"
                    | PREPARED_MARKER
            )
        {
            return Err(invalid());
        }
        names.push(name);
    }
    let limit = if names.iter().any(|name| name == PREPARED_MARKER) {
        516
    } else {
        515
    };
    if names.len() > limit {
        return Err(invalid());
    }
    Ok(names)
}

fn read_generated_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    if !regular_file(path)? || fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(invalid());
    }
    serde_json::from_slice(&fs::read(path)?).map_err(|_| invalid())
}

/// Stage a replacement derived from the current composed assets. The returned
/// directory may contain edited outputs, never an original/baseline snapshot.
pub fn begin_generated(root: &Path, stem: &str) -> io::Result<PathBuf> {
    let identity = generated_root(root, stem)?;
    recover(root, stem)?;
    let inputs = (
        1u32,
        identity,
        stamp(&root.join(format!("{stem}.kfc")))?,
        stamp(&root.join(format!("{stem}.kfc_resources")))?,
    );
    let stage = directory(root, stem);
    fs::create_dir(&stage)?;
    let path = stage.join("generated-inputs.json");
    fs::write(&path, serde_json::to_vec(&inputs).map_err(|_| invalid())?)?;
    sync(&path)?;
    Ok(stage)
}

/// Validate a separate prepared-output directory without creating or recovering
/// a transaction. Prepared runs never use the source-root recovery path.
pub fn validate_prepared_output(
    source_root: &Path,
    output_root: &Path,
    stem: &str,
) -> io::Result<PathBuf> {
    let source = PathBuf::from(generated_root(source_root, stem)?);
    let output = PathBuf::from(generated_root(output_root, stem)?);
    if source.starts_with(&output) || output.starts_with(&source) {
        return Err(invalid());
    }

    let stage = directory(&output, stem);
    match fs::symlink_metadata(&stage) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "prepared generated-output stage already exists",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(output)
}

/// Validate the staged mod/config environment, asset input, and generated
/// output roots. Canonical paths must be pairwise disjoint.
pub fn validate_prepared_roots(
    environment_root: &Path,
    source_root: &Path,
    output_root: &Path,
    stem: &str,
) -> io::Result<(PathBuf, PathBuf)> {
    let environment = PathBuf::from(generated_root(environment_root, stem)?);
    let source = PathBuf::from(generated_root(source_root, stem)?);
    let output = PathBuf::from(generated_root(output_root, stem)?);
    let roots = [&environment, &source, &output];
    for left in 0..roots.len() {
        for right in left + 1..roots.len() {
            if roots[left].starts_with(roots[right]) || roots[right].starts_with(roots[left]) {
                return Err(invalid());
            }
        }
    }
    validate_prepared_output(&source, &output, stem)?;
    Ok((source, output))
}

/// Begin a private prepared stage under a separate output root. This records
/// source preimages and never recovers, publishes, or cleans the source root.
pub fn begin_generated_prepared(
    source_root: &Path,
    output_root: &Path,
    stem: &str,
) -> io::Result<PathBuf> {
    let output = validate_prepared_output(source_root, output_root, stem)?;
    let source_identity = generated_root(source_root, stem)?;
    let output_identity = generated_root(&output, stem)?;
    let source = PathBuf::from(&source_identity);
    let inputs: GeneratedInputs = (
        1,
        source_identity.clone(),
        stamp(&source.join(format!("{stem}.kfc")))?,
        stamp(&source.join(format!("{stem}.kfc_resources")))?,
    );

    let stage = directory(&output, stem);
    fs::create_dir(&stage)?;
    let marker = stage.join(PREPARED_MARKER);
    let marker_contents: PreparedMarker = (1, source_identity, output_identity);
    fs::write(
        &marker,
        serde_json::to_vec(&marker_contents).map_err(|_| invalid())?,
    )?;
    sync(&marker)?;
    let inputs_path = stage.join("generated-inputs.json");
    fs::write(
        &inputs_path,
        serde_json::to_vec(&inputs).map_err(|_| invalid())?,
    )?;
    sync(&inputs_path)?;
    Ok(stage)
}

fn preserves_prefix(original: &Path, generated: &Path) -> io::Result<bool> {
    use std::io::Read;
    let mut original = fs::File::open(original)?;
    let mut generated = fs::File::open(generated)?;
    let mut left = [0u8; 65536];
    let mut right = [0u8; 65536];
    loop {
        let count = original.read(&mut left)?;
        if count == 0 {
            return Ok(true);
        }
        if generated.read_exact(&mut right[..count]).is_err() || left[..count] != right[..count] {
            return Ok(false);
        }
    }
}

fn validate_generated_stage(
    source_root: &Path,
    stage: &Path,
    stem: &str,
    prepared_output_root: Option<&Path>,
) -> io::Result<(String, Vec<String>, Stamp, Stamp)> {
    let identity = generated_root(source_root, stem)?;
    if let Some(output_root) = prepared_output_root {
        let output_identity = generated_root(output_root, stem)?;
        let source = PathBuf::from(&identity);
        let output = PathBuf::from(&output_identity);
        if source.starts_with(&output) || output.starts_with(&source) {
            return Err(invalid());
        }
        let marker: PreparedMarker = read_generated_json(&stage.join(PREPARED_MARKER))?;
        if marker != (1, identity.clone(), output_identity) {
            return Err(invalid());
        }
    } else if stage.join(PREPARED_MARKER).exists() {
        return Err(non_publishable_prepared());
    }

    let mut names = generated_inventory(stage, stem)?;
    if names
        .iter()
        .any(|name| name == "generated-pending.json" || name == "generated-journal.tmp")
    {
        return Err(invalid());
    }
    let (version, expected_root, index, resources): GeneratedInputs =
        read_generated_json(&stage.join("generated-inputs.json"))?;
    if version != 1 || identity != expected_root {
        return Err(invalid());
    }
    if stamp(&source_root.join(format!("{stem}.kfc")))? != index
        || stamp(&source_root.join(format!("{stem}.kfc_resources")))? != resources
    {
        return Err(invalid());
    }
    names.retain(|name| valid_name(stem, name));
    let index_name = format!("{stem}.kfc");
    let resources_name = format!("{stem}.kfc_resources");
    if !names.contains(&index_name) || !names.contains(&resources_name) {
        return Err(invalid());
    }
    if !preserves_prefix(
        &source_root.join(&resources_name),
        &stage.join(&resources_name),
    )? {
        return Err(invalid());
    }
    for name in &names {
        stamp(&stage.join(name))?;
    }
    Ok((identity, names, index, resources))
}

/// Validate prepared output against the source preimages and retained resource
/// prefix. This reads both roots and does not publish or recover either root.
pub fn validate_generated_prepared(
    source_root: &Path,
    output_root: &Path,
    stem: &str,
) -> io::Result<()> {
    let source_identity = generated_root(source_root, stem)?;
    let output_identity = generated_root(output_root, stem)?;
    let source = PathBuf::from(source_identity);
    let output = PathBuf::from(output_identity);
    if source.starts_with(&output) || output.starts_with(&source) {
        return Err(invalid());
    }
    validate_generated_stage(&source, &directory(&output, stem), stem, Some(&output))?;
    Ok(())
}

fn prepare_generated(root: &Path, stem: &str) -> io::Result<()> {
    let stage = directory(root, stem);
    let (identity, mut names, index, resources) =
        validate_generated_stage(root, &stage, stem, None)?;
    let index_name = format!("{stem}.kfc");
    let resources_name = format!("{stem}.kfc_resources");
    names.sort_by_key(|name| (name == &index_name, name.clone()));
    let mut records = Vec::new();
    for name in names {
        let target = root.join(&name);
        let before = if name == index_name {
            Some(index.clone())
        } else if name == resources_name {
            Some(resources.clone())
        } else {
            None
        };
        match &before {
            Some(expected) if stamp(&target)? != *expected => return Err(invalid()),
            None if regular_file(&target)? => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("generated DAT target already exists: {name}"),
                ));
            }
            _ => {}
        }
        let output = stage.join(&name);
        if name == resources_name && !preserves_prefix(&target, &output)? {
            return Err(invalid());
        }
        sync(&output)?;
        records.push((name, before, stamp(&output)?));
    }
    let journal: GeneratedJournal = (1, identity, records);
    let temporary = stage.join("generated-journal.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(&journal).map_err(|_| invalid())?,
    )?;
    sync(&temporary)?;
    fs::rename(temporary, stage.join("generated-pending.json"))?;
    Ok(())
}

/// Complete an interrupted generated-output commit. Unknown bytes cause a
/// refusal with the journal and remaining outputs retained. Never restores .bak.
fn recover_generated(root: &Path, stem: &str) -> io::Result<()> {
    let identity = generated_root(root, stem)?;
    let stage = directory(root, stem);
    if stage.join(PREPARED_MARKER).exists() {
        return Err(non_publishable_prepared());
    }
    let names = generated_inventory(&stage, stem)?;
    let (version, expected_root, records): GeneratedJournal =
        read_generated_json(&stage.join("generated-pending.json"))?;
    if version != 1 || identity != expected_root || records.len() < 2 || records.len() > 512 {
        return Err(invalid());
    }
    let index_name = format!("{stem}.kfc");
    let resources_name = format!("{stem}.kfc_resources");
    let mut seen = std::collections::HashSet::new();
    for (name, before, after) in &records {
        if !valid_name(stem, name)
            || !seen.insert(name.clone())
            || after.1.len() != 64
            || !after.1.bytes().all(|b| b.is_ascii_hexdigit())
            || before.is_some() != (name == &index_name || name == &resources_name)
        {
            return Err(invalid());
        }
    }
    if records.last().map(|r| &r.0) != Some(&index_name)
        || !seen.contains(&resources_name)
        || names
            .iter()
            .any(|name| valid_name(stem, name) && !seen.contains(name))
    {
        return Err(invalid());
    }
    // Verify the entire pending set before any additional target mutation.
    for (name, before, after) in &records {
        let target = root.join(name);
        let actual = if regular_file(&target)? {
            Some(stamp(&target)?)
        } else {
            None
        };
        if actual.as_ref() == Some(after) {
            continue;
        }
        if actual != *before || stamp(&stage.join(name))? != *after {
            return Err(invalid());
        }
    }
    for (name, before, after) in &records {
        let target = root.join(name);
        let actual = if regular_file(&target)? {
            Some(stamp(&target)?)
        } else {
            None
        };
        if actual.as_ref() == Some(after) {
            continue;
        }
        if actual != *before || stamp(&stage.join(name))? != *after {
            return Err(invalid());
        }
        // Same-volume rename replaces atomically on supported Windows/Unix file
        // systems. Do not remove the target first: the previous index stays valid
        // until every output it will reference has been published.
        fs::rename(stage.join(name), &target)?;
        sync(&target)?;
    }
    for (name, _, after) in &records {
        if stamp(&root.join(name))? != *after {
            return Err(invalid());
        }
    }
    clean(&stage)
}

pub fn commit_generated(root: &Path, stem: &str) -> io::Result<()> {
    prepare_generated(root, stem)?;
    recover_generated(root, stem)
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

    #[test]
    fn generated_publication_refuses_occupied_dat() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(root.join("game_123.dat"), b"existing-mod-content").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
        fs::write(stage.join("game_123.dat"), b"new-content").unwrap();
        let result = commit_generated(&root, "game");
        assert!(
            result.is_err(),
            "generated publication must refuse an occupied DAT"
        );
        assert_eq!(
            fs::read(root.join("game_123.dat")).unwrap(),
            b"existing-mod-content"
        );
        abort(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn generated_preparation_does_not_copy_originals() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
        prepare_generated(&root, "game").unwrap();
        assert!(
            !stage.join("game.kfc.rollback").exists(),
            "no original index backup is allowed"
        );
        assert!(!stage.join("game.kfc_resources.rollback").exists());
        abort(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn generated_recovery_completes_each_interrupted_publication_boundary() {
        for published in 0..=3 {
            let root = fixture();
            let stage = begin_generated(&root, "game").unwrap();
            fs::write(stage.join("game.kfc"), b"new-index").unwrap();
            fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
            fs::write(stage.join("game_123.dat"), b"new-content").unwrap();
            prepare_generated(&root, "game").unwrap();
            let (_, _, records): GeneratedJournal =
                read_generated_json(&stage.join("generated-pending.json")).unwrap();
            for (name, _, _) in records.iter().take(published) {
                fs::rename(stage.join(name), root.join(name)).unwrap();
            }
            recover(&root, "game").unwrap();
            assert_eq!(fs::read(root.join("game.kfc")).unwrap(), b"new-index");
            assert_eq!(
                fs::read(root.join("game.kfc_resources")).unwrap(),
                b"old-data-new-data"
            );
            assert_eq!(fs::read(root.join("game_123.dat")).unwrap(), b"new-content");
            assert!(!stage.exists());
            recover(&root, "game").unwrap();
            remove_fixture(&root);
        }
    }

    #[test]
    fn generated_refuses_changed_inputs_and_changed_resource_prefix() {
        for drift in [false, true] {
            let root = fixture();
            let stage = begin_generated(&root, "game").unwrap();
            fs::write(stage.join("game.kfc"), b"new-index").unwrap();
            fs::write(
                stage.join("game.kfc_resources"),
                if drift {
                    b"old-data-more"
                } else {
                    b"bad-data-more"
                },
            )
            .unwrap();
            if drift {
                fs::write(root.join("game.kfc"), b"foreign-index").unwrap();
            }
            assert!(commit_generated(&root, "game").is_err());
            assert_eq!(
                fs::read(root.join("game.kfc_resources")).unwrap(),
                b"old-data"
            );
            assert!(!stage.join("generated-pending.json").exists());
            abort(&root, "game").unwrap();
            remove_fixture(&root);
        }
    }

    #[test]
    fn generated_recovery_refuses_foreign_output_without_more_writes() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
        prepare_generated(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"foreign-index").unwrap();
        assert!(recover(&root, "game").is_err());
        assert_eq!(fs::read(root.join("game.kfc")).unwrap(), b"old-index");
        assert_eq!(
            fs::read(root.join("game.kfc_resources")).unwrap(),
            b"old-data"
        );
        assert!(stage.join("generated-pending.json").exists());
        // The test repairs its own tampering; production recovery never does this.
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        recover(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn generated_abort_refuses_foreign_staging_entries() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(stage.join("foreign.settings"), b"preserve").unwrap();
        assert!(abort(&root, "game").is_err());
        assert_eq!(
            fs::read(stage.join("foreign.settings")).unwrap(),
            b"preserve"
        );
        fs::remove_file(stage.join("foreign.settings")).unwrap();
        abort(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn generated_empty_dat_is_an_occupied_target() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(root.join("game_123.dat"), b"").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
        fs::write(stage.join("game_123.dat"), b"new-content").unwrap();
        assert!(commit_generated(&root, "game").is_err());
        assert_eq!(fs::metadata(root.join("game_123.dat")).unwrap().len(), 0);
        abort(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn generated_corrupt_journal_preserves_all_files() {
        let root = fixture();
        let stage = begin_generated(&root, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-new-data").unwrap();
        prepare_generated(&root, "game").unwrap();
        let journal = fs::read(stage.join("generated-pending.json")).unwrap();
        fs::write(stage.join("generated-pending.json"), b"partial-json").unwrap();
        assert!(recover(&root, "game").is_err());
        assert_eq!(fs::read(root.join("game.kfc")).unwrap(), b"old-index");
        assert_eq!(
            fs::read(root.join("game.kfc_resources")).unwrap(),
            b"old-data"
        );
        assert_eq!(fs::read(stage.join("game.kfc")).unwrap(), b"new-index");
        fs::write(stage.join("generated-pending.json"), journal).unwrap();
        recover(&root, "game").unwrap();
        remove_fixture(&root);
    }

    #[test]
    fn prepared_generation_uses_separate_unpublishable_output_stage() {
        let source = fixture();
        let output = source.with_file_name(format!("game-output-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&output).unwrap();
        let before_index = fs::read(source.join("game.kfc")).unwrap();
        let before_resources = fs::read(source.join("game.kfc_resources")).unwrap();

        let stage = begin_generated_prepared(&source, &output, "game").unwrap();
        assert!(stage.join(PREPARED_MARKER).is_file());
        assert!(recover(&output, "game").is_err());
        assert!(commit(&output, "game").is_err());
        assert!(commit_generated(&output, "game").is_err());
        assert!(abort(&output, "game").is_err());
        assert_eq!(fs::read(source.join("game.kfc")).unwrap(), before_index);
        assert_eq!(
            fs::read(source.join("game.kfc_resources")).unwrap(),
            before_resources
        );
        assert!(stage.join(PREPARED_MARKER).is_file());

        fs::remove_dir_all(&stage).unwrap();
        fs::remove_dir(&output).unwrap();
        remove_fixture(&source);
    }

    #[test]
    fn prepared_generation_rejects_nested_roots_and_existing_stage() {
        let source = fixture();
        let nested_output = source.join("prepared-output");
        fs::create_dir(&nested_output).unwrap();
        assert!(validate_prepared_output(&source, &nested_output, "game").is_err());
        fs::remove_dir(&nested_output).unwrap();

        let output = source.with_file_name(format!("game-output-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&output).unwrap();
        let stage = begin_generated_prepared(&source, &output, "game").unwrap();
        assert!(begin_generated_prepared(&source, &output, "game").is_err());
        fs::remove_dir_all(stage).unwrap();
        fs::remove_dir(output).unwrap();
        remove_fixture(&source);
    }

    #[test]
    fn prepared_validation_checks_source_preimages_and_resource_prefix() {
        let source = fixture();
        let output = source.with_file_name(format!("game-output-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&output).unwrap();
        let stage = begin_generated_prepared(&source, &output, "game").unwrap();
        fs::write(stage.join("game.kfc"), b"new-index").unwrap();
        fs::write(stage.join("game.kfc_resources"), b"old-data-appended").unwrap();
        validate_generated_prepared(&source, &output, "game").unwrap();

        fs::write(source.join("game.kfc"), b"changed-index").unwrap();
        assert!(validate_generated_prepared(&source, &output, "game").is_err());
        assert_eq!(
            fs::read(stage.join("game.kfc_resources")).unwrap(),
            b"old-data-appended"
        );

        fs::remove_dir_all(&stage).unwrap();
        fs::remove_dir(&output).unwrap();
        remove_fixture(&source);
    }
}
