import { createHash } from "node:crypto";
import { unzipSync } from "fflate";
import { validateRepository } from "./github-releases.mjs";

const maximumReportBytes = 5 * 1024 * 1024;
const maximumChecksumBytes = 4096;
const unifiedReportPath = "unified/release-security-report.json";
const scanStatuses = new Set(["clean", "findings", "warning", "error", "skipped", "unknown"]);
const jobStatuses = new Set(["success", "failure", "cancelled", "skipped", "unknown"]);
const statKeys = ["malicious", "suspicious", "undetected", "harmless", "timeout", "confirmed_timeout", "failure", "type_unsupported", "other"];
const definitions = [
  { id: "trivy-source", name: "Trivy", scope: "Source code and dependencies", job: "trivy_filesystem" },
  { id: "defender", name: "Microsoft Defender Antivirus", scope: "Release ZIP files and extracted contents", job: "defender" },
  { id: "trivy-release", name: "Trivy", scope: "Release package", job: "release_scanners" },
  { id: "clamav", name: "ClamAV", scope: "Release ZIP files and extracted contents", job: "release_scanners" },
  { id: "virustotal", name: "VirusTotal", scope: "Release ZIP files", job: "virustotal" },
];

class EvidenceError extends Error {
  constructor(state, message) {
    super(message);
    this.state = state;
  }
}

function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function sha256(value) {
  return typeof value === "string" && /^[a-f\d]{64}$/i.test(value) ? value.toLowerCase() : null;
}

function count(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function requireValid(condition, message = "The published security report is malformed.") {
  if (!condition) throw new EvidenceError("invalid", message);
}

function summary(state, message, { archiveSha256 = null, bundleSha256 = null } = {}) {
  archiveSha256 = sha256(archiveSha256);
  return {
    state,
    status: "incomplete",
    message,
    scanners: [],
    virusTotal: null,
    lookupUrl: archiveSha256 ? `https://www.virustotal.com/gui/file/${archiveSha256}` : null,
    workflowUrl: null,
    archiveSha256,
    bundleSha256: sha256(bundleSha256),
  };
}

function scannerId(report) {
  if (report.scanner === "Trivy" && report.target === "Source code and dependencies") return "trivy-source";
  if (report.scanner === "Trivy" && report.target === "Release package") return "trivy-release";
  if (report.scanner === "Microsoft Defender Antivirus") return "defender";
  if (report.scanner === "ClamAV") return "clamav";
  if (report.scanner === "VirusTotal") return "virustotal";
  return null;
}

function normalizeScanner(raw, definition, jobResults) {
  const job = jobResults[definition.job];
  const jobResult = jobStatuses.has(job) ? job : "unknown";
  let status = scanStatuses.has(raw?.status) ? raw.status : "unknown";
  const rawCount = definition.id === "defender" ? raw?.detection_count : raw?.finding_count;
  if (rawCount !== undefined && rawCount !== null) requireValid(count(rawCount));
  let findingCount = rawCount ?? null;
  if (raw?.scanned_file_count !== undefined && raw.scanned_file_count !== null) requireValid(count(raw.scanned_file_count));
  if (raw?.counts !== undefined) {
    requireValid(object(raw.counts));
    for (const value of Object.values(raw.counts)) requireValid(count(value));
  }
  if (status === "clean") {
    requireValid(findingCount === null || findingCount === 0, "The security report contains conflicting scan results.");
    if (findingCount === null) status = "unknown";
    if (definition.id === "defender" && raw.signature_update_status === "warning") status = "warning";
  }
  if (["error", "skipped", "unknown"].includes(status)) findingCount = null;
  return {
    id: definition.id,
    name: definition.name,
    scope: definition.scope,
    status,
    findingCount,
    jobResult,
    scannedFileCount: raw?.scanned_file_count ?? null,
  };
}

function analysisDate(value) {
  if (value === undefined || value === null) return null;
  const date = typeof value === "number" ? new Date(value * 1000) : typeof value === "string" ? new Date(value) : null;
  requireValid(date && Number.isFinite(date.getTime()), "VirusTotal analysis date is invalid.");
  return date.toISOString();
}

function normalizeVirusTotal(report, release, archiveSha256) {
  if (!report) return null;
  requireValid(Array.isArray(report.files), "VirusTotal archive evidence is missing.");
  if (report.files.length === 0 && ["error", "skipped", "unknown"].includes(report.status)) return null;
  requireValid(report.files.length === 1 && object(report.files[0]), "VirusTotal archive evidence is ambiguous.");
  const file = report.files[0];
  requireValid(file.archive === release.asset.name, "VirusTotal evidence belongs to a different release archive.");
  if (!file.sha256 && file.status === "error") return null;
  requireValid(sha256(file.sha256) === archiveSha256, "VirusTotal archive checksum does not match the published release.");
  const url = `https://www.virustotal.com/gui/file/${archiveSha256}`;
  requireValid(file.report_url === url, "VirusTotal report URL does not match the release archive checksum.");
  if (file.status === "error" && file.stats === null) return null;
  requireValid(object(file.stats), "VirusTotal analysis statistics are missing.");
  const stats = {};
  for (const key of statKeys) {
    requireValid(count(file.stats[key]), "VirusTotal analysis statistics are invalid.");
    stats[key] = file.stats[key];
  }
  const expectedStatus = stats.malicious > 0 ? "findings" : stats.suspicious > 0 ? "warning" : "clean";
  requireValid(file.status === expectedStatus && report.status === expectedStatus, "VirusTotal status conflicts with its analysis statistics.");
  for (const [key, expected] of [["finding_count", stats.malicious + stats.suspicious], ["malicious_count", stats.malicious], ["suspicious_count", stats.suspicious], ["undetected_count", stats.undetected], ["harmless_count", stats.harmless]]) {
    if (report[key] !== undefined) requireValid(count(report[key]) && report[key] === expected, "VirusTotal totals conflict with the archive analysis.");
  }
  if (file.finding_count !== undefined) requireValid(count(file.finding_count) && file.finding_count === stats.malicious + stats.suspicious);
  if (file.api_response !== undefined) {
    const response = file.api_response;
    requireValid(object(response) && object(response.data) && object(response.data.attributes), "VirusTotal raw analysis metadata is malformed.");
    const attributes = response.data.attributes;
    if (attributes.status !== undefined) requireValid(attributes.status === "completed", "VirusTotal analysis has not completed.");
    const metadata = response.meta?.file_info;
    if (metadata?.sha256 !== undefined) requireValid(sha256(metadata.sha256) === archiveSha256, "VirusTotal analysis identifies a different archive checksum.");
    if (metadata?.size !== undefined) requireValid(count(metadata.size) && metadata.size === release.asset.size, "VirusTotal analysis identifies a different archive size.");
    const rawStats = attributes.last_analysis_stats ?? attributes.stats;
    if (rawStats !== undefined) {
      requireValid(object(rawStats));
      for (const key of statKeys) {
        const rawKey = key === "confirmed_timeout" ? "confirmed-timeout" : key === "type_unsupported" ? "type-unsupported" : key;
        const value = rawStats[rawKey] ?? 0;
        requireValid(count(value) && value === stats[key], "VirusTotal raw statistics conflict with the report.");
      }
    }
  }
  return { url, sha256: archiveSha256, stats, analysisDate: analysisDate(file.analysis_date) };
}

export function normalizeSecurityReport(report, release, repository, checksums = {}) {
  const context = { archiveSha256: sha256(checksums.archiveSha256), bundleSha256: sha256(checksums.bundleSha256) };
  if (!release.securityReport) return summary("missing", "No security report was published for this release.", context);
  try {
    requireValid(context.archiveSha256 && context.bundleSha256, "The release security checksums are missing or invalid.");
    requireValid(object(report) && Array.isArray(report.scanners) && object(report.job_results));
    requireValid(report.repository === repository && report.version === release.version && report.run_id === release.build,
      "The security report does not belong to this repository, version and build.");
    const reports = new Map();
    for (const scanner of report.scanners) {
      requireValid(object(scanner));
      const id = scannerId(scanner);
      if (id) {
        requireValid(!reports.has(id), "The security report contains duplicate scanner results.");
        reports.set(id, scanner);
      }
    }
    const scanners = definitions.map(definition => normalizeScanner(reports.get(definition.id), definition, report.job_results));
    const virusTotal = normalizeVirusTotal(reports.get("virustotal"), release, context.archiveSha256);
    const workflowUrl = `https://github.com/${repository}/actions/runs/${release.build}`;
    requireValid(report.report_artifact_url === undefined || report.report_artifact_url === null || report.report_artifact_url === workflowUrl,
      "The security report workflow URL does not match its build.");
    let incomplete = report.job_results.build_and_package !== "success" ||
      scanners.some(scanner => ["error", "skipped", "unknown"].includes(scanner.status) || scanner.jobResult !== "success") || !virusTotal;
    for (const raw of reports.values()) {
      if (raw.scan_step_outcome !== undefined && raw.scan_step_outcome !== "success") incomplete = true;
      if (raw.sarif_upload_status !== undefined && raw.sarif_upload_status !== "success") incomplete = true;
    }
    const status = incomplete ? "incomplete" : scanners.some(scanner => scanner.status === "findings") ? "findings" : scanners.some(scanner => scanner.status === "warning") ? "warning" : "clean";
    const message = {
      incomplete: "Some scan evidence is missing, unsuccessful or incomplete. Review the individual results.",
      findings: "The published scan report contains findings. Review the individual results and VirusTotal analysis.",
      warning: "The published scan report contains advisory warnings. Review the individual results.",
      clean: "No findings reported.",
    }[status];
    return { ...summary("available", message, context), status, scanners, virusTotal, workflowUrl };
  } catch (error) {
    return summary("invalid", error instanceof EvidenceError ? error.message : "The published security report is malformed.", context);
  }
}

function validDownloadUrl(value, repository, release, name) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "github.com" && !url.port && !url.username && !url.password &&
      !url.search && !url.hash && decodeURIComponent(url.pathname) === `/${repository}/releases/download/${release.tag}/${name}`;
  } catch {
    return false;
  }
}

async function fetchBytes(url, maximumBytes) {
  let response;
  try {
    response = await fetch(url, { signal: AbortSignal.timeout(15_000), headers: { "User-Agent": "ShroudForge-download-website" } });
  } catch {
    throw new EvidenceError("unavailable", "Published security evidence could not be downloaded.");
  }
  if (!response.ok) {
    await response.body?.cancel();
    throw new EvidenceError("unavailable", "Published security evidence is temporarily unavailable.");
  }
  const length = Number(response.headers.get("content-length"));
  if (length > maximumBytes) {
    await response.body?.cancel();
    throw new EvidenceError("invalid", "Published security evidence exceeds the permitted size.");
  }
  if (!response.body) throw new EvidenceError("unavailable", "Published security evidence has no readable response body.");
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > maximumBytes) {
        await reader.cancel();
        throw new EvidenceError("invalid", "Published security evidence exceeds the permitted size.");
      }
      chunks.push(value);
    }
  } catch (error) {
    if (error instanceof EvidenceError) throw error;
    throw new EvidenceError("unavailable", "Published security evidence could not be read completely.");
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

function parseChecksum(bytes, expectedName) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes).trimEnd();
  } catch {
    throw new EvidenceError("invalid", "A published checksum file is malformed.");
  }
  const match = text.match(/^([a-f\d]{64})[ \t]+\*?([^\r\n]+)$/i);
  requireValid(match && match[2] === expectedName, "A published checksum file does not identify the expected archive.");
  return match[1].toLowerCase();
}

function readUnifiedReport(bytes) {
  let selected = 0;
  let totalSize = 0;
  try {
    const files = unzipSync(bytes, { filter: file => {
      requireValid(count(file.size) && count(file.originalSize));
      totalSize += file.originalSize;
      requireValid(totalSize <= maximumReportBytes && file.size <= maximumReportBytes, "The uncompressed security report exceeds the permitted size.");
      if (file.name !== unifiedReportPath) return false;
      selected += 1;
      requireValid(selected === 1, "The security report bundle contains duplicate unified reports.");
      return true;
    } });
    requireValid(selected === 1 && files[unifiedReportPath]?.byteLength <= maximumReportBytes, "The security report bundle does not contain its unified JSON report.");
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(files[unifiedReportPath]));
  } catch (error) {
    if (error instanceof EvidenceError) throw error;
    throw new EvidenceError("invalid", "The published security report bundle is malformed.");
  }
}

async function readReleaseReport(release, repository) {
  const context = { archiveSha256: null, bundleSha256: null };
  try {
    requireValid(release.checksum?.name === `${release.asset.name}.sha256` && validDownloadUrl(release.checksum.url, repository, release, release.checksum.name),
      "The release checksum URL is invalid.");
    context.archiveSha256 = parseChecksum(await fetchBytes(release.checksum.url, maximumChecksumBytes), release.asset.name);
  } catch (error) {
    if (!release.securityReport) return summary("missing", "No security report was published and the release checksum could not be verified.");
    return summary(error instanceof EvidenceError ? error.state : "unavailable", error instanceof EvidenceError ? error.message : "Published security evidence could not be read.");
  }
  if (!release.securityReport) return summary("missing", "No security report was published. The VirusTotal link opens a lookup for the published archive checksum.", context);
  try {
    const reportName = `security-report-${release.version}-${release.build}.zip`;
    requireValid(release.securityReport.name === reportName && validDownloadUrl(release.securityReport.url, repository, release, reportName),
      "The published security report URL is invalid.");
    context.bundleSha256 = parseChecksum(await fetchBytes(`${release.securityReport.url}.sha256`, maximumChecksumBytes), reportName);
    const bytes = await fetchBytes(release.securityReport.url, maximumReportBytes);
    requireValid(createHash("sha256").update(bytes).digest("hex") === context.bundleSha256, "The security report bundle checksum does not match its published checksum.");
    return normalizeSecurityReport(readUnifiedReport(bytes), release, repository, context);
  } catch (error) {
    return summary(error instanceof EvidenceError ? error.state : "invalid", error instanceof EvidenceError ? error.message : "The published security evidence is malformed.", context);
  }
}

export async function readSecurityReports(releases, repository) {
  validateRepository(repository);
  const results = new Array(releases.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, releases.length) }, async () => {
    while (next < releases.length) {
      const index = next++;
      results[index] = await readReleaseReport(releases[index], repository);
    }
  }));
  const summaries = Object.fromEntries(releases.map((release, index) => [release.tag, results[index]]));
  const counts = {};
  for (const result of results) counts[result.state] = (counts[result.state] || 0) + 1;
  console.log(`Read release security evidence: ${Object.entries(counts).map(([state, total]) => `${total} ${state}`).join(", ") || "no releases"}.`);
  return summaries;
}
