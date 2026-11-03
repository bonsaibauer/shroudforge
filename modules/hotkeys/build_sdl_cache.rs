use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

const CACHE_DIRECTORY: &str = "SHROUDFORGE_SDL_CACHE_DIR";
const CACHE_CONTEXT: &str = "SHROUDFORGE_SDL_CACHE_KEY";
const MANIFEST: &str = "manifest.json";

pub fn build_or_restore(
    directory: &Path,
    source: &Path,
    msvc: bool,
    static_crt: bool,
    compile_jobs: usize,
    build: impl FnOnce() -> PathBuf,
) -> PathBuf {
    println!("cargo:rerun-if-env-changed={CACHE_DIRECTORY}");
    println!("cargo:rerun-if-env-changed={CACHE_CONTEXT}");
    let cache = if msvc {
        match SdlCache::from_environment(directory, source, static_crt, compile_jobs) {
            Ok(cache) => cache,
            Err(error) => {
                println!("cargo:warning=SDL component cache disabled: {error}");
                None
            }
        }
    } else {
        None
    };
    if let Some(cache) = &cache {
        let imported = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("cached-sdl");
        match cache.restore(&imported) {
            Ok(()) => {
                println!(
                    "cargo:warning=SDL component cache hit: {}",
                    cache.fingerprint
                );
                return imported;
            }
            Err(error) => {
                println!(
                    "cargo:warning=SDL component cache miss: {error}. Building pinned SDL sources."
                );
            }
        }
    }

    let installed = build();
    if let Some(cache) = cache {
        match cache.export(&installed) {
            Ok(()) => println!(
                "cargo:warning=SDL component cache exported: {}",
                cache.fingerprint
            ),
            Err(error) => println!("cargo:warning=SDL component cache export skipped: {error}"),
        }
    }
    installed
}

struct SdlCache {
    directory: PathBuf,
    fingerprint: String,
    contract: Value,
    files: [PathBuf; 2],
}

impl SdlCache {
    fn from_environment(
        directory: &Path,
        source: &Path,
        static_crt: bool,
        compile_jobs: usize,
    ) -> io::Result<Option<Self>> {
        let cache_directory = env::var_os(CACHE_DIRECTORY);
        let context = env::var(CACHE_CONTEXT).ok();
        if cache_directory.is_none() && context.is_none() {
            return Ok(None);
        }
        let root =
            PathBuf::from(cache_directory.ok_or_else(|| invalid("cache directory is missing"))?);
        let context = context.ok_or_else(|| invalid("cache context is missing"))?;
        if !root.is_absolute()
            || context.len() != 64
            || !context.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid(
                "an absolute directory and a SHA-256 context key are required",
            ));
        }
        let profile = env::var("PROFILE").map_err(|_| invalid("Cargo profile is missing"))?;
        if !matches!(profile.as_str(), "debug" | "release") {
            return Err(invalid("unsupported Cargo profile"));
        }
        for name in [
            "CC",
            "CXX",
            "CFLAGS",
            "CXXFLAGS",
            "CMAKE",
            "CMAKE_GENERATOR",
            "CMAKE_GENERATOR_PLATFORM",
            "CMAKE_GENERATOR_TOOLSET",
            "CMAKE_TOOLCHAIN_FILE",
            "CMAKE_PREFIX_PATH",
            "CMAKE_C_FLAGS",
            "CMAKE_CXX_FLAGS",
            "CL",
            "_CL_",
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "VCToolsVersion",
            "VCToolsInstallDir",
            "WindowsSdkDir",
            "WindowsSDKVersion",
            "UCRTVersion",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
        let mut environment = BTreeMap::new();
        for (name, value) in env::vars_os() {
            let Some(name) = name.to_str() else { continue };
            if is_build_environment(name) {
                println!("cargo:rerun-if-env-changed={name}");
                environment.insert(name.to_owned(), value.to_string_lossy().into_owned());
            }
        }
        let mut sources = Sha256::new();
        hash_directory(&mut sources, source, source)?;
        let contract = json!({
            "format": 1,
            "context": context.to_ascii_lowercase(),
            "recipeSha256": recipe_digest(directory)?,
            "sourceSha256": format!("{:x}", sources.finalize()),
            "target": env::var("TARGET").unwrap_or_default(),
            "host": env::var("HOST").unwrap_or_default(),
            "profile": profile,
            "optLevel": env::var("OPT_LEVEL").unwrap_or_default(),
            "debug": env::var("DEBUG").unwrap_or_default(),
            "targetFeatures": env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default(),
            "staticCrt": static_crt,
            "compileJobs": compile_jobs,
            "environment": environment,
        });
        Ok(Some(Self::new(root, contract, &profile)))
    }

    fn new(root: PathBuf, contract: Value, profile: &str) -> Self {
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&contract).unwrap())
        );
        Self {
            directory: root.join(&fingerprint),
            fingerprint,
            contract,
            files: [
                PathBuf::from("lib/SDL3-static.lib"),
                PathBuf::from(format!(
                    "build/include-config-{profile}/build_config/SDL_build_config.h"
                )),
            ],
        }
    }

    fn restore(&self, destination: &Path) -> io::Result<()> {
        let manifest_path = self.directory.join(MANIFEST);
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        if fs::metadata(&manifest_path)?.len() > 64 * 1024 {
            return Err(invalid("cache manifest is too large"));
        }
        let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)
            .map_err(|_| invalid("cache manifest is invalid JSON"))?;
        if manifest["contract"] != self.contract || manifest["format"] != 1 {
            return Err(invalid("cache contract does not match this build"));
        }
        let files = manifest["files"]
            .as_object()
            .ok_or_else(|| invalid("cache file hashes are missing"))?;
        if files.len() != self.files.len() {
            return Err(invalid("cache file inventory does not match"));
        }
        for relative in &self.files {
            let name = portable_path(relative);
            let expected = files
                .get(&name)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("cache file hash is missing"))?;
            let source = self.directory.join(relative);
            println!("cargo:rerun-if-changed={}", source.display());
            if file_digest(&source)? != expected {
                return Err(invalid("cached SDL file hash does not match"));
            }
            let target = destination.join(relative);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(&source, &target)?;
            if file_digest(&target)? != expected {
                return Err(invalid("copied SDL file hash does not match"));
            }
        }
        Ok(())
    }

    fn export(&self, installed: &Path) -> io::Result<()> {
        fs::create_dir_all(&self.directory)?;
        let mut files = BTreeMap::new();
        for relative in &self.files {
            let source = installed.join(relative);
            let hash = file_digest(&source)?;
            let destination = self.directory.join(relative);
            fs::create_dir_all(destination.parent().unwrap())?;
            let temporary = destination.with_extension(format!("cache-tmp-{}", std::process::id()));
            fs::copy(&source, &temporary)?;
            if file_digest(&temporary)? != hash {
                return Err(invalid("exported SDL file hash does not match"));
            }
            fs::rename(&temporary, &destination)?;
            files.insert(portable_path(relative), hash);
        }
        let manifest = json!({ "format": 1, "contract": self.contract, "files": files });
        let temporary = self
            .directory
            .join(format!("manifest-tmp-{}.json", std::process::id()));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?,
        )?;
        fs::rename(temporary, self.directory.join(MANIFEST))
    }
}

fn is_build_environment(name: &str) -> bool {
    [
        "CC",
        "CXX",
        "CFLAGS",
        "CXXFLAGS",
        "CMAKE",
        "HOST_CC",
        "HOST_CXX",
        "HOST_CFLAGS",
        "HOST_CXXFLAGS",
        "HOST_CMAKE",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE",
        "VCTools",
        "VCINSTALLDIR",
        "VSINSTALLDIR",
        "WindowsSdk",
        "WindowsSDK",
        "UCRT",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        || matches!(name, "CL" | "_CL_" | "INCLUDE" | "LIB" | "LIBPATH")
}

fn recipe_digest(directory: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    for path in ["build.rs", "build_sdl_cache.rs", "Cargo.toml"] {
        hash_file(&mut hash, &directory.join(path), Path::new(path))?;
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn hash_directory(hash: &mut Sha256, root: &Path, directory: &Path) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if entry.file_name() == ".git" {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            hash_directory(hash, root, &entry.path())?;
        } else if kind.is_file() {
            hash_file(
                hash,
                &entry.path(),
                entry.path().strip_prefix(root).unwrap(),
            )?;
        } else {
            return Err(invalid("SDL source cache requires regular files"));
        }
    }
    Ok(())
}

fn hash_file(hash: &mut Sha256, path: &Path, relative: &Path) -> io::Result<()> {
    let name = portable_path(relative);
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    let mut file = fs::File::open(path)?;
    hash.update(file.metadata()?.len().to_le_bytes());
    hash_contents(hash, &mut file)
}

fn hash_contents(hash: &mut Sha256, file: &mut fs::File) -> io::Result<()> {
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            return Ok(());
        }
        hash.update(&buffer[..length]);
    }
}

fn file_digest(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(invalid("SDL cache file is empty or not a regular file"));
    }
    let mut hash = Sha256::new();
    hash_contents(&mut hash, &mut fs::File::open(path)?)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> (PathBuf, SdlCache, PathBuf) {
        let root = env::temp_dir().join(format!(
            "shroudforge-sdl-cache-{}-{name}",
            std::process::id()
        ));
        let cache = SdlCache::new(
            root.join("cache"),
            json!({ "context": name, "target": "msvc", "crt": "dynamic" }),
            "release",
        );
        let installed = root.join("installed");
        for relative in &cache.files {
            let path = installed.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, format!("fixture:{name}:{}", relative.display())).unwrap();
        }
        (root, cache, installed)
    }

    #[test]
    fn exports_and_restores_verified_library_and_private_header() {
        let (root, cache, installed) = fixture("roundtrip");
        cache.export(&installed).unwrap();
        let restored = root.join("restored");
        cache.restore(&restored).unwrap();
        for relative in &cache.files {
            assert_eq!(
                fs::read(installed.join(relative)).unwrap(),
                fs::read(restored.join(relative)).unwrap()
            );
        }
    }

    #[test]
    fn corrupt_library_is_rejected_and_can_be_replaced() {
        let (root, cache, installed) = fixture("corruption");
        cache.export(&installed).unwrap();
        fs::write(cache.directory.join(&cache.files[0]), "corrupt").unwrap();
        assert!(cache.restore(&root.join("rejected")).is_err());
        cache.export(&installed).unwrap();
        cache.restore(&root.join("restored")).unwrap();
    }

    #[test]
    fn incompatible_contract_and_missing_header_are_rejected() {
        let (root, cache, installed) = fixture("contract");
        cache.export(&installed).unwrap();
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(cache.directory.join(MANIFEST)).unwrap()).unwrap();
        manifest["contract"]["crt"] = json!("static");
        fs::write(
            cache.directory.join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(cache.restore(&root.join("wrong-crt")).is_err());
        cache.export(&installed).unwrap();
        fs::remove_file(cache.directory.join(&cache.files[1])).unwrap();
        assert!(cache.restore(&root.join("missing-header")).is_err());
    }

    #[test]
    fn source_fingerprint_tracks_header_changes_but_ignores_git_metadata() {
        let (root, _, _) = fixture("source");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("header.h"), "initial header").unwrap();
        fs::write(source.join(".git"), "first checkout path").unwrap();
        let digest = || {
            let mut hash = Sha256::new();
            hash_directory(&mut hash, &source, &source).unwrap();
            format!("{:x}", hash.finalize())
        };
        let initial = digest();
        fs::write(source.join(".git"), "another checkout path").unwrap();
        assert_eq!(initial, digest());
        fs::write(source.join("header.h"), "modified header").unwrap();
        assert_ne!(initial, digest());
    }

    #[test]
    fn file_hashes_use_standard_sha256() {
        let (root, _, _) = fixture("digest");
        let path = root.join("abc");
        fs::write(&path, "abc").unwrap();
        assert_eq!(
            file_digest(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn recipe_changes_invalidate_the_component_cache() {
        let (root, _, _) = fixture("recipe");
        let recipe = root.join("recipe");
        fs::create_dir_all(&recipe).unwrap();
        for name in ["build.rs", "build_sdl_cache.rs", "Cargo.toml"] {
            fs::write(recipe.join(name), format!("initial:{name}")).unwrap();
        }
        let mut previous = recipe_digest(&recipe).unwrap();
        for name in ["build.rs", "build_sdl_cache.rs", "Cargo.toml"] {
            fs::write(recipe.join(name), format!("changed:{name}")).unwrap();
            let changed = recipe_digest(&recipe).unwrap();
            assert_ne!(previous, changed);
            previous = changed;
        }
    }

    #[test]
    fn target_crt_profile_and_flags_cannot_restore_another_builds_artifacts() {
        let (root, cache, installed) = fixture("variants");
        cache.export(&installed).unwrap();
        for (name, value) in [
            ("target", json!("another-target")),
            ("crt", json!("static")),
            ("context", json!("another-compiler-sdk-context")),
            ("sourceSha256", json!("changed-source")),
            ("recipeSha256", json!("changed-recipe")),
            ("profile", json!("debug")),
            ("compileJobs", json!(1)),
            ("environment", json!({ "CFLAGS": "/changed" })),
        ] {
            let mut contract = cache.contract.clone();
            contract[name] = value;
            let incompatible = SdlCache::new(root.join("cache"), contract, "release");
            assert_ne!(cache.fingerprint, incompatible.fingerprint);
            assert!(
                incompatible
                    .restore(&root.join(format!("rejected-{name}")))
                    .is_err()
            );
        }
    }
}
