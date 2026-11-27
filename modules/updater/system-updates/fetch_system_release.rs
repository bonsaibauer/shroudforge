use crate::provider_config::ProviderConfig;
use crate::release::Release;

pub fn fetch_latest(provider: &ProviderConfig, current: &str) -> Result<Option<Release>, String> {
    fetch_latest_with_cached_checksum(provider, current, None, None, None)
}

pub fn fetch_latest_with_cached_checksum(
    provider: &ProviderConfig,
    current: &str,
    cached_version: Option<&str>,
    cached_checksum: Option<&str>,
    cached_download_url: Option<&str>,
) -> Result<Option<Release>, String> {
    if provider.kind != "github-releases" {
        return Err(format!("unsupported update provider: {}", provider.kind));
    }
    fetch_latest_github_with_cached_checksum(
        provider,
        current,
        cached_version,
        cached_checksum,
        cached_download_url,
    )
}

pub fn fetch_latest_github(
    provider: &ProviderConfig,
    current: &str,
) -> Result<Option<Release>, String> {
    fetch_latest_github_with_cached_checksum(provider, current, None, None, None)
}

pub fn fetch_latest_github_with_cached_checksum(
    provider: &ProviderConfig,
    current: &str,
    cached_version: Option<&str>,
    cached_checksum: Option<&str>,
    cached_download_url: Option<&str>,
) -> Result<Option<Release>, String> {
    let Some(release) = shroudforge_release_feed::fetch_latest_with_cached_checksum(
        &provider.base_url,
        &provider.project_id,
        current,
        provider.include_prereleases,
        true,
        cached_version,
        cached_checksum,
        cached_download_url,
    )?
    else {
        return Ok(None);
    };
    let checksum = release
        .checksum
        .ok_or("GitHub release checksum was not fetched")?;
    let (mandatory, notes) = crate::parse_release_notes(&release.body);
    Ok(Some(Release {
        version: release.version,
        base_version: release.base_version,
        build: release.build,
        download_url: release.download_url,
        checksum,
        release_url: Some(release.release_url),
        prerelease: release.prerelease,
        mandatory,
        message: if notes.trim().is_empty() {
            "A new ShroudForge version is available on GitHub.".into()
        } else {
            notes
        },
    }))
}
