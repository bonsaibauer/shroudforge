---
name: inspect-release-security-report
description: Inspect or repair the release security evidence flow across Trivy, Defender, ClamAV, VirusTotal and unified report packaging. Use for release scan failures or report contracts without weakening publication policy or silently rescanning/uploading artifacts.
---

# Inspect Release Security Evidence

Read `.agents/skills/repository/repository-working-rules.md`, `.github/workflows/build.yml`, `dev_build.yml`, and `dev/build/write_release_security_report.py`. Inspect a concrete workflow run and its downloaded reports/artifacts through the available GitHub connector or existing local paths. Distinguish completed scan findings from scanner failure, skipped job, cancelled job and missing/malformed report.

The evidence set contains source Trivy SARIF/JSON, release Trivy SARIF/JSON, Defender JSON, ClamAV JSON/raw log and VirusTotal JSON with full Analysis API response. `write_release_security_report.py` normalizes available inputs and writes unified JSON/Markdown. `package_release_security_report.py` bundles them, adds release-body.md and writes a SHA-256 sidecar. CI verifies that bundle before publication. Report generation runs even when earlier jobs fail.

Follow exact scanner behavior before changing it. Defender scans ZIPs and extracted contents, records signature/update/retry state and can recover Defender only on GitHub-hosted runners. VirusTotal uploads each release ZIP, polls the returned analysis, enforces intervals and size limits, and distinguishes advisory suspicious/single-malicious findings from its current blocking threshold. ClamAV must have a usable log summary, and Trivy SARIF upload status is separately recorded from findings. Preserve current workflow publication gates unless the task explicitly changes policy.

Use `scan_release_with_defender.ps1`, `scan_release_with_virustotal.sh`, `source-trivy-ignore.yaml` and workflow job dependencies as source evidence. Exceptions must remain narrowly justified to the actual dependency/path/target. Never suppress errors or turn missing evidence into clean results. Do not print API secrets. A report inspection does not itself authorize a new external upload, publishing or changing local antivirus settings.

For report-code changes, verify using small saved fixture reports covering clean, findings, invalid/missing, failed and skipped inputs. Inspect the resulting JSON and Markdown plus ZIP member paths/checksum. Report scanner scope, result, job outcome, raw evidence and actual publication implication separately. State whether any real scan or upload ran.
