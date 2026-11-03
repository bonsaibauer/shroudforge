import fs from "node:fs/promises";
import path from "node:path";

const integer = "(?:0|[1-9]\\d*)";
const versionPattern = `${integer}\\.${integer}\\.${integer}`;
const tagPattern = new RegExp(`^v?(${versionPattern})(?:-(build|dev)\\.(${integer}))?$`);

export function validateRepository(repository) {
  if (typeof repository !== "string" || !/^[A-Za-z0-9][A-Za-z0-9_.-]*\/[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(repository)) {
    throw new Error("Website repository must use the GitHub owner/repository format.");
  }
  return repository;
}

function isGithubUrl(value, expectedPath) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "github.com" && !url.port &&
      !url.username && !url.password && !url.search && !url.hash &&
      decodeURIComponent(url.pathname) === expectedPath;
  } catch {
    return false;
  }
}

function findAsset(assets, name, repository, tag) {
  const matches = assets.filter(asset => asset?.name === name);
  if (matches.length !== 1) return null;
  const asset = matches[0];
  return asset.state === "uploaded" && isGithubUrl(asset.browser_download_url, `/${repository}/releases/download/${tag}/${name}`) ? asset : null;
}

function normalizeRelease(release, repository) {
  if (!release || typeof release !== "object" || release.draft || typeof release.prerelease !== "boolean") return null;
  const match = typeof release.tag_name === "string" && release.tag_name.match(tagPattern);
  if (!match || !Array.isArray(release.assets)) return null;
  const [, version, , taggedBuild] = match;
  const tag = release.tag_name;
  let build = taggedBuild;
  if (build === undefined) {
    const archivePattern = new RegExp(`^shroudforge-${version.replaceAll(".", "\\.")}-(${integer})\\.zip$`);
    const candidates = release.assets.map(asset => typeof asset?.name === "string" ? asset.name.match(archivePattern) : null).filter(Boolean);
    if (candidates.length !== 1) return null;
    build = candidates[0][1];
  }
  const assetName = `shroudforge-${version}-${build}.zip`;
  const asset = findAsset(release.assets, assetName, repository, tag);
  const checksum = findAsset(release.assets, `${assetName}.sha256`, repository, tag);
  if (!asset || !checksum || !Number.isSafeInteger(asset.size) || asset.size <= 0 ||
      !Number.isSafeInteger(asset.download_count) || asset.download_count < 0) return null;
  if (!isGithubUrl(release.html_url, `/${repository}/releases/tag/${tag}`)) return null;
  const published = typeof release.published_at === "string" ? new Date(release.published_at) : null;
  if (!published || !Number.isFinite(published.getTime())) return null;
  const securityReport = findAsset(release.assets, `security-report-${version}-${build}.zip`, repository, tag);
  return {
    tag,
    version,
    build,
    prerelease: release.prerelease,
    publishedAt: published.toISOString(),
    releaseUrl: release.html_url,
    asset: {
      name: asset.name,
      url: asset.browser_download_url,
      size: asset.size,
      downloadCount: asset.download_count,
    },
    checksum: { name: checksum.name, url: checksum.browser_download_url },
    securityReport: securityReport ? { name: securityReport.name, url: securityReport.browser_download_url } : null,
  };
}

export function compareReleases(left, right) {
  const leftVersion = left.version.split(".").map(BigInt);
  const rightVersion = right.version.split(".").map(BigInt);
  for (let index = 0; index < 3; index++) {
    if (leftVersion[index] !== rightVersion[index]) return leftVersion[index] > rightVersion[index] ? -1 : 1;
  }
  const leftBuild = BigInt(left.build);
  const rightBuild = BigInt(right.build);
  if (leftBuild !== rightBuild) return leftBuild > rightBuild ? -1 : 1;
  if (left.prerelease !== right.prerelease) return left.prerelease ? 1 : -1;
  return right.publishedAt.localeCompare(left.publishedAt) || left.tag.localeCompare(right.tag);
}

export function normalizeReleases(rawReleases, repository) {
  validateRepository(repository);
  if (!Array.isArray(rawReleases)) throw new Error("GitHub release data must be a JSON array.");
  const releases = new Map();
  for (const raw of rawReleases) {
    const release = normalizeRelease(raw, repository);
    if (release && !releases.has(release.tag)) releases.set(release.tag, release);
  }
  return [...releases.values()].sort(compareReleases);
}

export async function readGithubReleases(repository) {
  validateRepository(repository);
  if (process.env.RELEASES_FILE) {
    const file = path.resolve(process.env.RELEASES_FILE);
    let raw;
    try {
      raw = JSON.parse(await fs.readFile(file, "utf8"));
    } catch {
      throw new Error("RELEASES_FILE could not be read as JSON.");
    }
    return normalizeReleases(raw, repository);
  }

  const rawReleases = [];
  const headers = {
    Accept: "application/vnd.github+json",
    "User-Agent": "ShroudForge-download-website",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (process.env.GH_TOKEN) headers.Authorization = `Bearer ${process.env.GH_TOKEN}`;
  for (let page = 1; ; page++) {
    let response;
    try {
      response = await fetch(`https://api.github.com/repos/${repository}/releases?per_page=100&page=${page}`, {
        headers,
        signal: AbortSignal.timeout(30_000),
      });
    } catch {
      throw new Error(`Could not reach the GitHub releases API on page ${page}.`);
    }
    if (!response.ok) {
      const rateLimit = response.status === 403 || response.status === 429 ? " Check the API rate limit or provide GH_TOKEN." : "";
      throw new Error(`GitHub releases API returned HTTP ${response.status} on page ${page}.${rateLimit}`);
    }
    let releases;
    try {
      releases = await response.json();
    } catch {
      throw new Error(`GitHub releases API returned invalid JSON on page ${page}.`);
    }
    if (!Array.isArray(releases)) throw new Error(`GitHub releases API returned a non-array response on page ${page}.`);
    rawReleases.push(...releases);
    if (releases.length < 100) break;
  }
  return normalizeReleases(rawReleases, repository);
}
