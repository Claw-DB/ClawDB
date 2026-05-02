# Security Policy

## Supported Versions

We release patches for security vulnerabilities on the latest minor release.

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✓         |

## Reporting a Vulnerability

**Do not file a public GitHub issue for security vulnerabilities.**

Please report security vulnerabilities via private disclosure:

1. Email **security@example.com** (replace with actual contact).
2. Describe the vulnerability in detail, including reproduction steps,
   affected versions, and potential impact.
3. We will acknowledge your report within 48 hours and aim to release a
   patch within 14 days for critical issues.

## Scope

- Vulnerabilities in the `clawdb`, `clawdb-server`, or `clawdb-cli` crates.
- Vulnerabilities in configuration handling, JWT/token management, or
  access-control enforcement in `claw-guard`.

## Out of Scope

- Vulnerabilities in third-party dependencies that are not under our control
  (please report those upstream).
- Issues in demo or example code not included in the published binaries.

## Important Notes

- **JWT secrets and API keys must never appear in issues, PRs, or commit messages.**
  If you accidentally committed a secret, rotate it immediately and contact us.
- We request coordinated disclosure — please allow us 90 days before making
  a vulnerability public, unless the issue is already actively exploited.
