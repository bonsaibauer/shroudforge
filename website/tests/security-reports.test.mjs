import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { zipSync, strToU8 } from "fflate";
import { normalizeSecurityReport, readSecurityReports } from "../tools/security-reports.mjs";

const repository = "example/shroudforge";
const archiveSha256 = "a".repeat(64);
const bundleSha256 = "b".repeat(64);
const checksums = { archiveSha256, bundleSha256 };

function releaseFixture(build = "42") {
  const tag = `v1.2.3-build.${build}`;
  const base = `https://github.com/${repository}/releases/download/${tag}`;
  const name = `shroudforge-1.2.3-${build}.zip`;
  const reportName = `security-report-1.2.3-${build}.zip`;
  return {
    tag, version: "1.2.3", build, prerelease: false,
    asset: { name, url: `${base}/${name}`, size: 1234, downloadCount: 0 },
    checksum: { name: `${name}.sha256`, url: `${base}/${name}.sha256` },
    securityReport: { name: reportName, url: `${base}/${reportName}` },
  };
}

function reportFixture() {
  const stats = {
    malicious: 0, suspicious: 0, undetected: 66, harmless: 0,
    timeout: 1, confirmed_timeout: 0, failure: 1, type_unsupported: 7, other: 0,
  };
  return {
    repository, version: "1.2.3", run_id: "42", ref: "1.2.3", commit_sha: "c".repeat(40),
    report_artifact_url: `https://github.com/${repository}/actions/runs/42`,
    job_results: { build_and_package: "success", trivy_filesystem: "success", defender: "success", release_scanners: "success", virustotal: "success" },
    scanners: [
      { scanner: "Trivy", target: "Source code and dependencies", status: "clean", finding_count: 0, scan_step_outcome: "success", sarif_upload_status: "success" },
      { scanner: "Microsoft Defender Antivirus", status: "clean", detection_count: 0, scanned_file_count: 128, signature_update_status: "success" },
      { scanner: "Trivy", target: "Release package", status: "clean", finding_count: 0, scan_step_outcome: "success", sarif_upload_status: "success" },
      { scanner: "ClamAV", status: "clean", finding_count: 0, scanned_file_count: 129, scan_step_outcome: "success" },
      {
        scanner: "VirusTotal", status: "clean", finding_count: 0, malicious_count: 0, suspicious_count: 0,
        files: [{
          archive: "shroudforge-1.2.3-42.zip", sha256: archiveSha256, status: "clean", finding_count: 0,
          report_url: `https://www.virustotal.com/gui/file/${archiveSha256}`, stats,
          analysis_date: 1793669673,
          api_response: { data: { attributes: { status: "completed" } }, meta: { file_info: { sha256: archiveSha256, size: 1234 } } },
        }],
      },
    ],
  };
}

function normalize(report = reportFixture(), release = releaseFixture(), hashes = checksums) {
  return normalizeSecurityReport(report, release, repository, hashes);
}

test("completed scans preserve VT engine outcomes without claiming that undetected means safe", () => {
  const result = normalize();
  assert.equal(result.state, "available");
  assert.equal(result.status, "clean");
  assert.deepEqual(result.scanners.map(scanner => scanner.id), ["trivy-source", "defender", "trivy-release", "clamav", "virustotal"]);
  assert.equal(result.virusTotal.stats.undetected, 66);
  assert.equal(result.virusTotal.stats.timeout, 1);
  assert.equal(result.virusTotal.stats.failure, 1);
  assert.equal(result.virusTotal.stats.type_unsupported, 7);
  assert.equal(result.virusTotal.analysisDate, "2026-11-03T01:34:33.000Z");
  assert.equal(result.lookupUrl, `https://www.virustotal.com/gui/file/${archiveSha256}`);
  assert.equal(result.message, "No findings reported.");
  assert.equal(JSON.stringify(result).includes("api_response"), false);
});

test("one malicious VT finding remains a finding regardless of the publication threshold", () => {
  const report = reportFixture();
  const vt = report.scanners[4];
  Object.assign(vt, { status: "findings", finding_count: 1, malicious_count: 1 });
  Object.assign(vt.files[0], { status: "findings", finding_count: 1 });
  vt.files[0].stats.malicious = 1;
  const result = normalize(report);
  assert.equal(result.state, "available");
  assert.equal(result.status, "findings");
  assert.equal(result.virusTotal.stats.malicious, 1);
});

test("suspicious VT detections are visible as advisory warnings", () => {
  const report = reportFixture();
  const vt = report.scanners[4];
  Object.assign(vt, { status: "warning", finding_count: 1, suspicious_count: 1 });
  Object.assign(vt.files[0], { status: "warning", finding_count: 1 });
  vt.files[0].stats.suspicious = 1;
  assert.equal(normalize(report).status, "warning");
});

test("scanner errors never expose a zero-findings claim or raw error details", () => {
  const report = reportFixture();
  report.scanners[1].status = "error";
  report.scanners[1].error = "VT_API_KEY=private-secret-value";
  report.job_results.defender = "failure";
  const result = normalize(report);
  assert.equal(result.status, "incomplete");
  assert.equal(result.scanners[1].status, "error");
  assert.equal(result.scanners[1].findingCount, null);
  assert.equal(result.scanners[1].jobResult, "failure");
  assert.equal(JSON.stringify(result).includes("private-secret-value"), false);
});

test("a failed VT attempt with a literal zero in the raw report has no reported finding count", () => {
  const report = reportFixture();
  report.scanners[4] = { scanner: "VirusTotal", status: "error", finding_count: 0, files: [] };
  report.job_results.virustotal = "failure";
  const result = normalize(report);
  assert.equal(result.state, "available");
  assert.equal(result.status, "incomplete");
  assert.equal(result.scanners[4].findingCount, null);
  assert.equal(result.virusTotal, null);
  assert.equal(result.lookupUrl, `https://www.virustotal.com/gui/file/${archiveSha256}`);
});

test("skipped and cancelled jobs retain their outcome and cannot become clean", () => {
  const report = reportFixture();
  Object.assign(report.scanners[0], { status: "skipped", finding_count: 0, scan_step_outcome: "cancelled" });
  report.job_results.trivy_filesystem = "cancelled";
  const result = normalize(report);
  assert.equal(result.status, "incomplete");
  assert.equal(result.scanners[0].status, "skipped");
  assert.equal(result.scanners[0].findingCount, null);
  assert.equal(result.scanners[0].jobResult, "cancelled");
});

test("a completed scan and a failed workflow job remain separate facts", () => {
  const report = reportFixture();
  report.job_results.trivy_filesystem = "failure";
  report.scanners[0].sarif_upload_status = "failure";
  const result = normalize(report);
  assert.equal(result.scanners[0].status, "clean");
  assert.equal(result.scanners[0].jobResult, "failure");
  assert.equal(result.status, "incomplete");
});

test("a failed package build prevents an overall clean summary even when scanner jobs succeeded", () => {
  const report = reportFixture();
  report.job_results.build_and_package = "failure";
  const result = normalize(report);
  assert.equal(result.state, "available");
  assert.equal(result.status, "incomplete");
  assert.equal(result.scanners.every(scanner => scanner.status === "clean"), true);
});

test("legacy releases expose only a checksum lookup when no security bundle was published", () => {
  const release = releaseFixture();
  release.securityReport = null;
  const result = normalize(null, release, { archiveSha256 });
  assert.equal(result.state, "missing");
  assert.equal(result.status, "incomplete");
  assert.equal(result.virusTotal, null);
  assert.equal(result.archiveSha256, archiveSha256);
  assert.equal(result.lookupUrl, `https://www.virustotal.com/gui/file/${archiveSha256}`);
});

test("missing scanner evidence creates unknown placeholders instead of clean results", () => {
  const report = reportFixture();
  report.scanners.splice(2, 1);
  const result = normalize(report);
  assert.equal(result.state, "available");
  assert.equal(result.status, "incomplete");
  assert.equal(result.scanners[2].id, "trivy-release");
  assert.equal(result.scanners[2].status, "unknown");
  assert.equal(result.scanners[2].findingCount, null);
});

test("malformed scanner structures and fractional counts invalidate the report", () => {
  const malformed = reportFixture();
  malformed.scanners = "not a scanner array";
  assert.equal(normalize(malformed).state, "invalid");
  const fractional = reportFixture();
  fractional.scanners[4].files[0].stats.malicious = 0.5;
  assert.equal(normalize(fractional).state, "invalid");
});

test("repository, version and build are bound to the exact release", () => {
  for (const [key, value] of [["repository", "untrusted/repo"], ["version", "9.9.9"], ["run_id", "43"]]) {
    const report = reportFixture();
    report[key] = value;
    assert.equal(normalize(report).state, "invalid", key);
  }
});

test("VT archive names and checksums must identify the selected loader package", () => {
  const wrongArchive = reportFixture();
  wrongArchive.scanners[4].files[0].archive = "security-report-1.2.3-42.zip";
  assert.equal(normalize(wrongArchive).state, "invalid");
  const wrongChecksum = reportFixture();
  wrongChecksum.scanners[4].files[0].sha256 = "d".repeat(64);
  assert.equal(normalize(wrongChecksum).state, "invalid");
});

test("untrusted VT links and unfinished analyses are rejected", () => {
  const wrongUrl = reportFixture();
  wrongUrl.scanners[4].files[0].report_url = `https://www.virustotal.com.evil.invalid/gui/file/${archiveSha256}`;
  assert.equal(normalize(wrongUrl).state, "invalid");
  const queued = reportFixture();
  queued.scanners[4].files[0].api_response.data.attributes.status = "queued";
  assert.equal(normalize(queued).state, "invalid");
});

function mockEvidence(t, { bundle, bundleChecksum, archiveChecksum, reportStatus = 200 } = {}) {
  const release = releaseFixture();
  bundle ??= zipSync({ "unified/release-security-report.json": strToU8(JSON.stringify(reportFixture())) });
  bundleChecksum ??= createHash("sha256").update(bundle).digest("hex");
  const calls = [];
  t.mock.method(globalThis, "fetch", async (url, options) => {
    calls.push({ url, options });
    if (url === release.checksum.url) return new Response(archiveChecksum ?? `${archiveSha256}  ${release.asset.name}\n`);
    if (url === `${release.securityReport.url}.sha256`) return new Response(`${bundleChecksum}  ${release.securityReport.name}\n`);
    if (url === release.securityReport.url) return new Response(bundle, { status: reportStatus });
    throw new Error("Unexpected network request");
  });
  return { release, calls };
}

test("reader verifies both sidecars and reads only the unified JSON from the existing bundle", async t => {
  const { release, calls } = mockEvidence(t);
  const result = (await readSecurityReports([release], repository))[release.tag];
  assert.equal(result.state, "available");
  assert.equal(result.status, "clean");
  assert.equal(result.archiveSha256, archiveSha256);
  assert.match(result.bundleSha256, /^[a-f\d]{64}$/);
  assert.deepEqual(calls.map(call => call.url), [release.checksum.url, `${release.securityReport.url}.sha256`, release.securityReport.url]);
  assert.equal(calls.some(call => call.options.headers.Authorization), false);
});

test("bundle corruption is invalid evidence rather than a clean scan", async t => {
  const { release } = mockEvidence(t, { bundleChecksum: "f".repeat(64) });
  const result = (await readSecurityReports([release], repository))[release.tag];
  assert.equal(result.state, "invalid");
  assert.equal(result.status, "incomplete");
  assert.equal(result.virusTotal, null);
});

test("a SHA sidecar naming another package cannot provide a scan lookup", async t => {
  const { release, calls } = mockEvidence(t, { archiveChecksum: `${archiveSha256}  another.zip\n` });
  const result = (await readSecurityReports([release], repository))[release.tag];
  assert.equal(result.state, "invalid");
  assert.equal(result.lookupUrl, null);
  assert.equal(calls.length, 1);
});

test("report download failures remain unavailable and preserve a separately verified lookup", async t => {
  const { release } = mockEvidence(t, { reportStatus: 503 });
  const result = (await readSecurityReports([release], repository))[release.tag];
  assert.equal(result.state, "unavailable");
  assert.equal(result.status, "incomplete");
  assert.equal(result.archiveSha256, archiveSha256);
  assert.equal(result.virusTotal, null);
});

test("a ZIP without the canonical unified report is invalid", async t => {
  const bundle = zipSync({ "another/release-security-report.json": strToU8(JSON.stringify(reportFixture())) });
  const { release } = mockEvidence(t, { bundle });
  assert.equal((await readSecurityReports([release], repository))[release.tag].state, "invalid");
});

test("an oversized uncompressed ZIP member is rejected before extraction", async t => {
  const bundle = zipSync({ "unified/release-security-report.json": new Uint8Array(5 * 1024 * 1024 + 1) });
  const { release } = mockEvidence(t, { bundle });
  const result = (await readSecurityReports([release], repository))[release.tag];
  assert.equal(result.state, "invalid");
  assert.match(result.message, /uncompressed.*size/);
});

test("legacy checksum lookups use at most four concurrent public downloads", async t => {
  const releases = Array.from({ length: 9 }, (_, index) => ({ ...releaseFixture(String(index + 1)), securityReport: null }));
  let active = 0;
  let maximumActive = 0;
  let calls = 0;
  t.mock.method(globalThis, "fetch", async url => {
    active++;
    calls++;
    maximumActive = Math.max(active, maximumActive);
    await new Promise(resolve => setTimeout(resolve, 2));
    active--;
    const name = decodeURIComponent(new URL(url).pathname.split("/").at(-1)).slice(0, -".sha256".length);
    return new Response(`${archiveSha256}  ${name}\n`);
  });
  const results = await readSecurityReports(releases, repository);
  assert.equal(calls, releases.length);
  assert.equal(maximumActive, 4);
  for (const result of Object.values(results)) {
    assert.equal(result.state, "missing");
    assert.equal(result.virusTotal, null);
    assert.equal(result.lookupUrl, `https://www.virustotal.com/gui/file/${archiveSha256}`);
  }
});
