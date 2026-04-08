# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | Yes                |

## Reporting a Vulnerability

**Do not open a public issue for security vulnerabilities.**

Please email **aisprkl@gmail.com** with:

1. A description of the vulnerability
2. Affected version or commit
3. Steps to reproduce
4. Potential impact assessment
5. Any suggested fixes (optional)

This email address is the current reporting channel for the project.

## Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 7 days
- **Fix timeline**: Within 90 days of confirmed report
- **Public disclosure**: After fix is released, coordinated with reporter

## What Qualifies as a Security Issue

- Arbitrary code execution via crafted status files
- Hook script injection or command injection
- Privilege escalation through the tray app
- Credential or token exposure in status files
- Path traversal in file watcher or focus handlers
- Unauthorized process control via focus commands

## What Does NOT Qualify

- Bugs that do not have a security impact
- Feature requests
- Issues requiring physical access to the machine
- Vulnerabilities in dependencies that are already patched upstream
