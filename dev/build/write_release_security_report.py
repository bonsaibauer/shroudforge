#!/usr/bin/env python3
"""Write per-scanner and unified security reports for ShroudForge releases."""

from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path
from typing import Any


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sarif_findings(path: Path) -> tuple[list[dict[str, str]], dict[str, int], str | None]:
    if not path.is_file():
        return [], {"error": 0, "warning": 0, "note": 0, "other": 0}, "The SARIF file was not generated."
    try:
        sarif = json.loads(read_text(path))
    except json.JSONDecodeError as error:
        return [], {"error": 0, "warning": 0, "note": 0, "other": 0}, f"The SARIF file contains invalid JSON: {error}"
    if not isinstance(sarif, dict):
        return [], {"error": 0, "warning": 0, "note": 0, "other": 0}, "The SARIF file must contain a JSON object."

    findings: list[dict[str, str]] = []
    counts = {"error": 0, "warning": 0, "note": 0, "other": 0}
    for run in sarif.get("runs", []) or []:
        if not isinstance(run, dict):
            continue
        rules: dict[str, str] = {}
        tool = run.get("tool") or {}
        driver = tool.get("driver") or {}
        for rule in driver.get("rules", []) or []:
            if not isinstance(rule, dict):
                continue
            description = rule.get("shortDescription")
            rules[rule.get("id", "")] = description.get("text", "") if isinstance(description, dict) else ""
        for result in run.get("results", []) or []:
            if not isinstance(result, dict):
                continue
            level = str(result.get("level", "other")).lower()
            if level not in counts:
                level = "other"
            counts[level] += 1
            location = ""
            locations = result.get("locations", []) or []
            if locations:
                physical = locations[0].get("physicalLocation") or {}
                artifact_location = physical.get("artifactLocation") or {}
                artifact = artifact_location.get("uri", "")
                region = physical.get("region") or {}
                line = region.get("startLine")
                location = f"{artifact}:{line}" if artifact and line else str(artifact)
            rule_id = str(result.get("ruleId", ""))
            message_entry = result.get("message") or {}
            message = str(message_entry.get("text", "")) if isinstance(message_entry, dict) else str(message_entry)
            findings.append(
                {
                    "level": level,
                    "rule_id": rule_id,
                    "title": rules.get(rule_id, "") or rule_id or "Finding",
                    "message": message,
                    "location": location,
                }
            )
    return findings, counts, None


def write_trivy_report(args: argparse.Namespace) -> None:
    sarif_path = Path(args.sarif)
    findings, counts, parse_error = sarif_findings(sarif_path)
    outcome = args.scan_outcome.lower()
    if outcome == "success" and parse_error is None:
        status = "findings" if findings else "clean"
    elif outcome in {"skipped", "cancelled"}:
        status = "skipped"
    else:
        status = "error"
    error = parse_error
    if outcome == "success" and parse_error:
        status = "error"

    report = {
        "scanner": "Trivy",
        "target": args.target,
        "version": args.version,
        "status": status,
        "scan_step_outcome": args.scan_outcome,
        "sarif_upload_status": args.sarif_upload_outcome,
        "sarif_file": sarif_path.name if sarif_path.is_file() else None,
        "finding_count": len(findings),
        "counts": counts,
        "findings": findings,
        "error": error,
    }
    write_json(Path(args.output), report)


def write_clamav_report(args: argparse.Namespace) -> None:
    log_path = Path(args.log)
    log = read_text(log_path)
    infected_match = re.findall(r"Infected files:\s*(\d+)", log, re.IGNORECASE)
    scanned_match = re.findall(r"Scanned files:\s*(\d+)", log, re.IGNORECASE)
    detected = [line.strip() for line in log.splitlines() if re.search(r"\sFOUND\s*$", line)]
    infected_count = int(infected_match[-1]) if infected_match else len(detected)
    scan_outcome = args.scan_outcome.lower()
    if scan_outcome == "success" and (infected_match or infected_count == 0):
        status = "findings" if infected_count else "clean"
    elif scan_outcome in {"skipped", "cancelled"}:
        status = "skipped"
    elif infected_count:
        status = "findings"
    else:
        status = "error"

    error = None
    if scan_outcome == "success" and not infected_match:
        error = "ClamAV completed the scan without a summary in the captured log."
        status = "error"
    elif scan_outcome == "failure" and not infected_count:
        error = "The ClamAV scan failed. See the attached raw log."
    if args.signature_update_outcome.lower() not in {"success", "skipped"}:
        error = error or "The ClamAV signature update did not complete successfully."
        if status == "skipped":
            status = "error"

    report = {
        "scanner": "ClamAV",
        "target": "Release ZIP files and extracted contents",
        "version": args.version or None,
        "status": status,
        "scan_step_outcome": args.scan_outcome,
        "signature_update_status": args.signature_update_outcome,
        "scanned_file_count": int(scanned_match[-1]) if scanned_match else None,
        "finding_count": infected_count,
        "findings": [{"level": "malware", "message": item} for item in detected],
        "error": error,
    }
    write_json(Path(args.output), report)


def load_report(path: Path, scanner: str, target: str, job_result: str) -> dict[str, Any]:
    if path.is_file():
        try:
            value = json.loads(read_text(path))
            if isinstance(value, dict):
                return value
        except json.JSONDecodeError as error:
            return {
                "scanner": scanner,
                "target": target,
                "status": "error",
                "finding_count": None,
                "error": f"Report JSON is invalid: {error}",
            }
    missing_status = "skipped" if job_result.lower() == "skipped" else "error"
    reason = "The scanner job was skipped before the report was created." if missing_status == "skipped" else "The scanner report is missing."
    return {
        "scanner": scanner,
        "target": target,
        "status": missing_status,
        "finding_count": None,
        "error": reason,
    }


def markdown_cell(value: Any) -> str:
    text = str(value if value is not None else "—")
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("|", "\\|")
        .replace("\r", " ")
        .replace("\n", " ")
        .replace("`", "\\`")
    )


def status_label(report: dict[str, Any]) -> str:
    status = report.get("status", "error")
    return {
        "clean": "Clean",
        "warning": "Warning",
        "findings": "Findings",
        "skipped": "Skipped",
        "error": "Error",
    }.get(str(status).lower(), str(status))


def job_result_label(value: Any) -> str:
    return {
        "success": "Succeeded",
        "failure": "Failed",
        "cancelled": "Cancelled",
        "skipped": "Skipped",
    }.get(str(value).lower(), "Unknown")


def sarif_status_label(value: Any) -> str:
    return {
        "success": "Uploaded",
        "failure": "Failed",
        "skipped": "Skipped",
        "cancelled": "Cancelled",
    }.get(str(value).lower(), "Unknown")


def signature_status_label(value: Any) -> str:
    return {
        "success": "updated",
        "warning": "Warning, using existing signatures",
        "failure": "failed",
        "skipped": "skipped",
        "not_attempted": "not started",
    }.get(str(value).lower(), str(value))


def finding_count(report: dict[str, Any]) -> Any:
    value = report.get("finding_count")
    if value is not None:
        return value
    return len(report.get("detections", []))


def report_notes(report: dict[str, Any]) -> str:
    notes: list[str] = []
    if report.get("version"):
        notes.append(f"Version: {report['version']}")
    if report.get("signature_update_status"):
        notes.append(f"Signature update: {signature_status_label(report['signature_update_status'])}")
    if report.get("signature_update_message"):
        notes.append(str(report["signature_update_message"]))
    for attempt in report.get("signature_update_attempts", []) or []:
        attempt_number = attempt.get("attempt", "?")
        attempt_status = signature_status_label(attempt.get("status", "unknown"))
        notes.append(f"Signature update attempt {attempt_number}: {attempt_status}")
        attempt_message = attempt.get("message")
        if attempt_message:
            notes.append(str(attempt_message))
    metadata = report.get("metadata", {})
    if isinstance(metadata, dict):
        if metadata.get("signature_version"):
            notes.append(f"Defender signature: {metadata['signature_version']}")
        if metadata.get("signature_last_updated"):
            notes.append(f"Updated: {metadata['signature_last_updated']}")
        if metadata.get("engine_version"):
            notes.append(f"Engine: {metadata['engine_version']}")
        if metadata.get("defender_service_status"):
            notes.append(f"Defender service: {metadata['defender_service_status']}")
        if metadata.get("defender_service_start_type"):
            notes.append(f"Defender service start type: {metadata['defender_service_start_type']}")
        if metadata.get("cloud_protection_level") is not None:
            notes.append(f"Cloud protection level: {metadata['cloud_protection_level']}")
        if metadata.get("real_time_protection_enabled") is not None:
            notes.append(f"Real-time protection: {metadata['real_time_protection_enabled']}")
    if str(report.get("scanner", "")).lower().startswith("microsoft defender"):
        if report.get("scan_attempt_count") is not None:
            notes.append(f"Scan attempts: {report['scan_attempt_count']}")
        for attempt in report.get("scan_attempt_errors", []) or []:
            attempt_number = attempt.get("attempt", "?")
            attempt_error = attempt.get("error")
            if attempt_error:
                notes.append(f"Attempt {attempt_number}: {attempt_error}")
            service_status = attempt.get("defender_service_status")
            service_start_type = attempt.get("defender_service_start_type")
            engine_version = attempt.get("engine_version")
            if service_status or service_start_type or engine_version:
                service_details = []
                if service_status:
                    service_details.append(f"service {service_status}")
                if service_start_type:
                    service_details.append(f"start type {service_start_type}")
                if engine_version:
                    service_details.append(f"engine {engine_version}")
                notes.append(f"Attempt {attempt_number} Defender state: {', '.join(service_details)}")
            recovery = attempt.get("recovery")
            if recovery:
                notes.append(f"Attempt {attempt_number} recovery: {recovery}")
    for warning in report.get("metadata_warnings", []) or []:
        notes.append(str(warning))
    if report.get("prepared_file_count") is not None:
        notes.append(f"Files prepared for scan: {report['prepared_file_count']}")
    if report.get("scanned_file_count") is not None:
        notes.append(f"Files scanned: {report['scanned_file_count']}")
    if str(report.get("scanner", "")).lower() == "virustotal":
        notes.append(f"Malicious: {report.get('malicious_count', 0)}")
        notes.append(f"Suspicious: {report.get('suspicious_count', 0)}")
        for item in report.get("files", []) or []:
            archive = item.get("archive") or "Release ZIP"
            sha256 = item.get("sha256")
            report_url = item.get("report_url")
            if sha256:
                details = f"{archive}, SHA-256 {sha256}"
                if report_url:
                    details += f", VirusTotal report {report_url}"
                notes.append(details)
    if report.get("sarif_upload_status"):
        notes.append(f"SARIF: {sarif_status_label(report['sarif_upload_status'])}")
    if report.get("error"):
        notes.append(str(report["error"]))
    return ", ".join(notes) or "—"


def count_cell(report: dict[str, Any], key: str) -> Any:
    counts = report.get("counts", {})
    if isinstance(counts, dict) and key in counts:
        return counts[key]
    return "—"


def collect_details(reports: list[dict[str, Any]]) -> list[dict[str, str]]:
    details: list[dict[str, str]] = []
    for report in reports:
        scanner = str(report.get("scanner", "Scanner"))
        if scanner.lower() == "trivy":
            scanner = f"Trivy ({report.get('target', 'Scan')})"
        entries = report.get("findings", [])
        if scanner.lower().startswith("microsoft defender"):
            entries = report.get("detections", [])
        for entry in entries or []:
            if scanner.lower().startswith("microsoft defender"):
                title = str(entry.get("threat_name") or "Defender detection")
                location = ", ".join(str(item) for item in entry.get("resources", []))
                level = "Malware"
                message = title
            else:
                title = str(entry.get("title") or entry.get("rule_id") or entry.get("message") or "Finding")
                location = str(entry.get("location", ""))
                level = str(entry.get("level", "Finding"))
                message = str(entry.get("message", ""))
            details.append({"scanner": scanner, "level": level, "finding": title, "message": message, "location": location})
    return details


def write_aggregate(args: argparse.Namespace) -> None:
    report_root = Path(args.reports_dir)
    build_result = os.environ.get("BUILD_JOB_RESULT", "unknown")
    source_job = os.environ.get("TRIVY_SOURCE_JOB_RESULT", "unknown")
    defender_job = os.environ.get("DEFENDER_JOB_RESULT", "unknown")
    release_job = os.environ.get("RELEASE_SCAN_JOB_RESULT", "unknown")
    virustotal_job = os.environ.get("VIRUSTOTAL_JOB_RESULT", "unknown")
    run_id = os.environ.get("GITHUB_RUN_ID", "")
    repository = os.environ.get("GITHUB_REPOSITORY", "")

    source = load_report(report_root / "trivy-filesystem" / "trivy-filesystem.json", "Trivy", "Source code and dependencies", source_job)
    defender = load_report(report_root / "defender" / "defender-release-report.json", "Microsoft Defender Antivirus", "Release package", defender_job)
    release_trivy = load_report(report_root / "release" / "trivy-release.json", "Trivy", "Release package", release_job)
    clamav = load_report(report_root / "release" / "clamav-release.json", "ClamAV", "ZIP and extracted release files", release_job)
    virustotal = load_report(report_root / "virustotal" / "virustotal-release.json", "VirusTotal", "Release ZIP files", virustotal_job)

    reports = [source, defender, release_trivy, clamav, virustotal]
    job_results = {
        "build_and_package": build_result,
        "trivy_filesystem": source_job,
        "defender": defender_job,
        "release_scanners": release_job,
        "virustotal": virustotal_job,
    }
    artifact_url = f"https://github.com/{repository}/actions/runs/{run_id}" if repository and run_id else None
    aggregate = {
        "run_id": run_id or None,
        "repository": repository or None,
        "ref": os.environ.get("GITHUB_REF_NAME") or None,
        "version": os.environ.get("RELEASE_VERSION") or None,
        "commit_sha": os.environ.get("GITHUB_SHA") or None,
        "job_results": job_results,
        "scanners": reports,
        "report_artifact_url": artifact_url,
    }

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    write_json(output_dir / "release-security-report.json", aggregate)

    lines = [
        "# Release security scan results",
        "",
        f"**Run:** {run_id or 'unknown'}",
        f"**Version:** {os.environ.get('RELEASE_VERSION') or 'unknown'}",
        f"**Commit:** {os.environ.get('GITHUB_SHA') or 'unknown'}",
        "",
        "## Scanner results",
        "",
        "| Scanner | Scope | Result | Findings | Errors | Warnings | Notes | SARIF upload | Scanner details |",
        "|---|---|---|---:|---:|---:|---:|---|---|",
    ]
    for report in reports:
        notes = report_notes(report)
        lines.append(
            f"| {markdown_cell(report.get('scanner'))} | {markdown_cell(report.get('target'))} | "
            f"{markdown_cell(status_label(report))} | {markdown_cell(finding_count(report))} | "
            f"{markdown_cell(count_cell(report, 'error'))} | {markdown_cell(count_cell(report, 'warning'))} | "
            f"{markdown_cell(count_cell(report, 'note'))} | {markdown_cell(report.get('sarif_upload_status', '—'))} | "
            f"{markdown_cell(notes)} |"
        )
    lines.extend(
        [
            "",
            "## Workflow job results",
            "",
            "| Job | Result |",
            "|---|---|",
            f"| Build and packaging | {markdown_cell(job_result_label(build_result))} |",
            f"| Trivy filesystem scan | {markdown_cell(job_result_label(source_job))} |",
            f"| Microsoft Defender | {markdown_cell(job_result_label(defender_job))} |",
            f"| Trivy package scan and ClamAV | {markdown_cell(job_result_label(release_job))} |",
            f"| VirusTotal release scan | {markdown_cell(job_result_label(virustotal_job))} |",
        ]
    )

    details = collect_details(reports)
    lines.extend(["", "## Findings", ""])
    if details:
        lines.extend(
            [
                "| Scanner | Severity | Finding | Message | File or location |",
                "|---|---|---|---|---|",
            ]
        )
        for item in details:
            lines.append(
                f"| {markdown_cell(item['scanner'])} | {markdown_cell(item['level'])} | "
                f"{markdown_cell(item['finding'])} | {markdown_cell(item['message'])} | "
                f"{markdown_cell(item['location'])} |"
            )
    else:
        lines.append("No individual findings were included in the available scanner reports.")

    lines.extend(
        [
            "",
            "## Included reports",
            "",
            "This bundle contains this Markdown report, the unified JSON report, available scanner JSON reports, SARIF files, raw logs, and the full VirusTotal Analysis API response.",
        ]
    )
    if artifact_url:
        lines.append(f"[Open workflow run and its artifacts]({artifact_url})")
    markdown = "\n".join(lines) + "\n"
    (output_dir / "release-security-report.md").write_text(markdown, encoding="utf-8")
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as summary:
            summary.write(markdown)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    trivy = subparsers.add_parser("trivy")
    trivy.add_argument("--target", required=True)
    trivy.add_argument("--version", default="aquasecurity/trivy-action@v0.36.0")
    trivy.add_argument("--sarif", required=True)
    trivy.add_argument("--scan-outcome", required=True)
    trivy.add_argument("--sarif-upload-outcome", required=True)
    trivy.add_argument("--output", required=True)
    trivy.set_defaults(function=write_trivy_report)

    clamav = subparsers.add_parser("clamav")
    clamav.add_argument("--scan-outcome", required=True)
    clamav.add_argument("--signature-update-outcome", required=True)
    clamav.add_argument("--version", default="")
    clamav.add_argument("--log", required=True)
    clamav.add_argument("--output", required=True)
    clamav.set_defaults(function=write_clamav_report)

    aggregate = subparsers.add_parser("aggregate")
    aggregate.add_argument("--reports-dir", required=True)
    aggregate.add_argument("--output-dir", required=True)
    aggregate.set_defaults(function=write_aggregate)

    args = parser.parse_args()
    args.function(args)


if __name__ == "__main__":
    main()
