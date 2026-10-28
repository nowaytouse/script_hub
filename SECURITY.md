# Security

## Reporting a vulnerability

Please report suspected vulnerabilities through GitHub's private repository security advisory flow:

<https://github.com/nowaytouse/script_hub/security/advisories/new>

If private reporting is unavailable, contact the maintainers through an established private channel before sharing details. Do not post exploit steps, credentials, personal data, or other sensitive information in a public issue. Do not assume that the advisory feature is enabled or that a report has been received until GitHub confirms submission.

When reporting, include the affected version or commit, platform, concise impact, and reproduction details that do not expose real secrets or user data. Avoid attaching private workspace files or unredacted logs.

## Scope and limitations

This repository has not completed a full security audit or a comprehensive third-party dependency/source inventory. A passing build or CI job is not a security certification. Script Hub's explicit-action network model does not make upstream sources or scripts inherently trustworthy; review configured sources and generated output before use.
