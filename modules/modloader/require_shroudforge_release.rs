//! The dependency through which a mod requires a ShroudForge release, such as
//! `{ "id": "shroudforge", "version": ">=1.8.1" }`. It is checked against the
//! installed release (`shroudforge/version.json`). `shroudforge-api` keeps
//! checking the Lua API version.

use std::path::Path;

/// The dependency id that names the ShroudForge release.
pub const SHROUDFORGE_RELEASE_DEPENDENCY: &str = "shroudforge";

/// The installed ShroudForge release, when it reads as a semantic version.
pub fn installed_release(root: &Path) -> Option<semver::Version> {
    semver::Version::parse(&crate::installed_version(root)).ok()
}

/// The status a dependency on the ShroudForge release shows, with the
/// installed release: `installed`, `version-mismatch` or `invalid-version`.
pub fn release_dependency_status(root: &Path, requirement: &str) -> (&'static str, String) {
    let installed = crate::installed_version(root);
    let status = match semver::VersionReq::parse(requirement)
        .ok()
        .zip(semver::Version::parse(&installed).ok())
    {
        Some((requirement, release)) if requirement.matches(&release) => "installed",
        Some(_) => "version-mismatch",
        None => "invalid-version",
    };
    (status, installed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_with_release(release: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("shroudforge-release-{}", std::process::id()))
            .join(release);
        let file = shroudforge_loader_config::paths::version_file(&root);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, format!(r#"{{"version":"{release}"}}"#)).unwrap();
        root
    }

    #[test]
    fn a_release_requirement_is_checked_against_the_installed_release() {
        let root = root_with_release("1.8.1");
        assert_eq!(installed_release(&root), Some(semver::Version::new(1, 8, 1)));
        assert_eq!(release_dependency_status(&root, ">=1.8.1").0, "installed");
        assert_eq!(release_dependency_status(&root, ">=1.8.2").0, "version-mismatch");
        assert_eq!(release_dependency_status(&root, "not a version").0, "invalid-version");
        std::fs::remove_dir_all(root).unwrap();
    }
}
