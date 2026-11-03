const states = new Set(["available", "missing", "unavailable", "invalid"]);
const statuses = new Set(["clean", "findings", "warning", "incomplete"]);
const scannerStatuses = new Set(["clean", "findings", "warning", "error", "skipped", "unknown"]);
const jobResults = new Set(["success", "failure", "cancelled", "skipped", "unknown"]);
const scannerIds = ["trivy-source", "defender", "trivy-release", "clamav", "virustotal"];
const statisticKeys = ["malicious", "suspicious", "undetected", "harmless", "timeout", "confirmed_timeout", "failure", "type_unsupported", "other"];
const isHash = value => typeof value === "string" && /^[a-f0-9]{64}$/.test(value);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

export function validateSecurityCatalog({ releases, securityReports, links, tags, repository }) {
  assert(securityReports && typeof securityReports === "object" && !Array.isArray(securityReports), "Website security summaries are missing.");
  assert(Object.keys(securityReports).length === releases.length, "Every release needs an explicit security summary.");
  const rendered = new Set(tags.map(tag => tag.attributes["data-security-release"]).filter(Boolean));
  let available = 0;
  for (const release of releases) {
    const summary = securityReports[release.tag];
    assert(summary && states.has(summary.state) && statuses.has(summary.status), `Invalid security summary: ${release.tag}`);
    assert(rendered.has(release.tag), `Security summary is not rendered: ${release.tag}`);
    if (summary.state !== "available") assert(summary.status === "incomplete", "Missing or invalid reports must never appear clean.");
    if (summary.lookupUrl) {
      assert(isHash(summary.archiveSha256) && summary.lookupUrl === `https://www.virustotal.com/gui/file/${summary.archiveSha256}`, "VirusTotal lookups must use the published loader checksum.");
      assert(links.has(summary.lookupUrl), `VirusTotal lookup is not rendered: ${release.tag}`);
    }
    if (summary.state === "available") {
      available++;
      assert(release.securityReport && isHash(summary.bundleSha256) && isHash(summary.archiveSha256), "Available security evidence must identify its verified archives.");
      assert(summary.workflowUrl === `https://github.com/${repository}/actions/runs/${release.build}`, "Security workflow must match the release build.");
      assert(links.has(summary.workflowUrl), `Security workflow is not linked: ${release.tag}`);
      assert(Array.isArray(summary.scanners) && summary.scanners.length === scannerIds.length, "Security summary must distinguish all five scanner scopes.");
      assert(new Set(summary.scanners.map(scanner => scanner.id)).size === scannerIds.length, "Security scanner scopes must be unique.");
      for (const scanner of summary.scanners) {
        assert(scannerIds.includes(scanner.id) && scannerStatuses.has(scanner.status) && jobResults.has(scanner.jobResult), "Security scanner state is invalid.");
        assert(scanner.findingCount === null || (Number.isSafeInteger(scanner.findingCount) && scanner.findingCount >= 0), "Security finding counts must be explicit nonnegative values or unknown.");
        if (scanner.jobResult !== "success") assert(summary.status === "incomplete", "An unsuccessful scanner job must leave the overall evidence incomplete.");
        if (["error", "skipped", "unknown"].includes(scanner.status)) assert(scanner.findingCount === null, "Unfinished scans must not imply zero findings.");
      }
    }
    if (summary.virusTotal) {
      const vt = summary.virusTotal;
      assert(summary.state === "available" && vt.sha256 === summary.archiveSha256 && vt.url === summary.lookupUrl, "VirusTotal results must match the verified release archive.");
      assert(statisticKeys.every(key => Number.isSafeInteger(vt.stats?.[key]) && vt.stats[key] >= 0), "VirusTotal engine categories must be preserved explicitly.");
      assert(links.has(vt.url), `VirusTotal report is not linked: ${release.tag}`);
    }
  }
  return available;
}
