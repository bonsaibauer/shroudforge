# Security Policy

## Supported versions

Security fixes are provided for the latest stable ShroudForge release. Older releases and development builds may not receive security updates; please update to the latest stable release before reporting an issue.

## Reporting a vulnerability

Please report suspected vulnerabilities privately. If GitHub private vulnerability reporting is enabled for this repository, use [Report a vulnerability](https://github.com/bonsaibauer/shroudforge/security/advisories/new). Do not open a public issue or include exploit details in a discussion.

If private reporting is unavailable, contact the repository maintainer through the contact information on their GitHub profile. If you cannot find a private contact route, open an issue asking for one without including sensitive technical details.

Please include the affected ShroudForge version, the relevant environment and steps to reproduce the issue. Give maintainers a reasonable opportunity to investigate and prepare a fix before publishing details.

## Automated scans

GitHub Actions runs Trivy filesystem scans on relevant code changes. Before publishing, the release workflow scans the expanded release packages with ClamAV and Microsoft Defender. A release is published only after both malware scans succeed. These checks support the release process but do not replace responsible vulnerability reporting.
