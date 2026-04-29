# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| Latest (`main`) | ✅ Yes |
| Older releases | ⚠️ Best effort |

## Reporting a Vulnerability

**Please do not open a public GitHub Issue for security vulnerabilities.**

If you discover a security issue in Aegis OS, report it privately:

1. Open a [GitHub Security Advisory](https://github.com/Gustavo324234/Aegis-Core/security/advisories/new)
2. Describe the vulnerability, steps to reproduce, and potential impact
3. You will receive a response within 7 days

We will acknowledge the report, investigate, and coordinate a fix before any public disclosure.

## Scope

Security reports are welcomed for:

- Authentication bypass or privilege escalation in the Citadel Protocol
- Multi-tenant data isolation failures (one tenant accessing another tenant's data)
- Remote code execution via any Aegis endpoint
- Vulnerabilities in the `ank-http` or `ank-core` crates
- Secrets (API keys, `AEGIS_ROOT_KEY`) exposed in logs, responses, or error messages

Out of scope:
- Issues in third-party dependencies (report those upstream)
- Vulnerabilities requiring physical access to the server

## Disclosure Policy

Once a fix is available and released, the vulnerability will be disclosed publicly
via a GitHub Security Advisory with full details and CVE assignment if applicable.

We follow coordinated disclosure and ask that reporters do the same — please allow
reasonable time for a fix before public disclosure.
