use semver::Version;
use serde::Deserialize;
use std::time::Duration;

pub const GITHUB_API_BASE_URL: &str = "https://api.github.com";
pub const SHROUDFORGE_PROJECT_ID: &str = "bonsaibauer/shroudforge";

#[derive(Clone, Debug)]
pub struct ReleaseMetadata {
    pub version: String,
    pub base_version: String,
    pub build: u64,
    pub download_url: String,
    pub checksum: Option<String>,
    pub release_url: String,
    pub body: String,
    pub prerelease: bool,
}

#[derive(Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub body: String,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Find the newest compatible ShroudForge GitHub release.
///
/// When `include_checksum` is false this performs only the release metadata
/// request. The updater requests the checksum before it accepts a release for
/// installation. The Modloader news fallback only needs release metadata.
pub fn fetch_latest(
    base_url: &str,
    project_id: &str,
    current_version: &str,
    include_prereleases: bool,
    include_checksum: bool,
) -> Result<Option<ReleaseMetadata>, String> {
    fetch_latest_with_cached_checksum(
        base_url,
        project_id,
        current_version,
        include_prereleases,
        include_checksum,
        None,
        None,
        None,
    )
}

/// Find the newest compatible release and reuse its previously verified
/// checksum when the selected release tag has not changed.
pub fn fetch_latest_with_cached_checksum(
    base_url: &str,
    project_id: &str,
    current_version: &str,
    include_prereleases: bool,
    include_checksum: bool,
    cached_version: Option<&str>,
    cached_checksum: Option<&str>,
    cached_download_url: Option<&str>,
) -> Result<Option<ReleaseMetadata>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("ShroudForge/{current_version} (modloader-ui)"))
        .build()
        .map_err(|error| error.to_string())?;
    let url = format!(
        "{}/repos/{}/releases?per_page=100",
        base_url.trim_end_matches('/'),
        project_id.trim_matches('/')
    );
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|error| format!("GitHub is unreachable: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", response.status()));
    }
    let releases: Vec<GithubRelease> = response
        .json()
        .map_err(|error| format!("Invalid GitHub response: {error}"))?;
    let mut eligible = releases
        .into_iter()
        .filter(|release| !release.draft && (include_prereleases || !release.prerelease))
        .filter_map(|release| {
            let version = release.tag_name.trim_start_matches('v').to_owned();
            let (base_version, build) = parse_release_tag(&version).ok()?;
            let parsed_version = Version::parse(&base_version).ok()?;
            let archive = release.assets.iter().find(|asset| {
                asset.name.starts_with("shroudforge-") && asset.name.ends_with(".zip")
            })?;
            let checksum_name = format!("{}.sha256", archive.name);
            let checksum_asset = release
                .assets
                .iter()
                .find(|asset| asset.name == checksum_name)?;
            let archive_url = archive.browser_download_url.clone();
            let checksum_url = checksum_asset.browser_download_url.clone();
            Some((
                parsed_version,
                build,
                release,
                archive_url,
                checksum_url,
                base_version,
                version,
            ))
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let Some((_, build, release, download_url, checksum_url, base_version, version)) =
        eligible.pop()
    else {
        return Ok(None);
    };
    let checksum = if include_checksum {
        let cached_checksum = cached_version
            .zip(cached_checksum)
            .filter(|(cached_version, checksum)| {
                *cached_version == version.as_str()
                    && cached_download_url == Some(download_url.as_str())
                    && is_sha256(checksum)
            })
            .map(|(_, checksum)| checksum.to_ascii_lowercase());
        Some(match cached_checksum {
            Some(checksum) => checksum,
            None => fetch_checksum(&client, &checksum_url)?,
        })
    } else {
        None
    };
    Ok(Some(ReleaseMetadata {
        version,
        base_version,
        build,
        download_url,
        checksum,
        release_url: release.html_url,
        body: release.body,
        prerelease: release.prerelease,
    }))
}

fn fetch_checksum(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("Could not download GitHub checksum: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub checksum endpoint returned HTTP {}",
            response.status()
        ));
    }
    let checksum = response
        .text()
        .map_err(|error| format!("Could not read GitHub checksum: {error}"))?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !is_sha256(&checksum) {
        return Err("GitHub release does not contain a valid SHA-256 checksum".into());
    }
    Ok(checksum)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn parse_release_tag(tag: &str) -> Result<(String, u64), String> {
    let tag = tag.trim_start_matches('v');
    let (version, build) = tag
        .rsplit_once("-build.")
        .or_else(|| tag.rsplit_once("-dev."))
        .ok_or_else(|| format!("Invalid ShroudForge release tag: {tag}"))?;
    Version::parse(version)
        .map_err(|error| format!("Invalid ShroudForge version '{version}': {error}"))?;
    let build = build
        .parse::<u64>()
        .map_err(|_| format!("Invalid ShroudForge build ID: {build}"))?;
    Ok((version.to_owned(), build))
}

pub fn is_update_available(
    current_version: &str,
    current_build: u64,
    latest_version: &str,
    latest_build: u64,
) -> bool {
    let Ok(current) = Version::parse(current_version.trim_start_matches('v')) else {
        return false;
    };
    let Ok(latest) = Version::parse(latest_version.trim_start_matches('v')) else {
        return false;
    };
    latest > current || (latest == current && latest_build > current_build)
}
