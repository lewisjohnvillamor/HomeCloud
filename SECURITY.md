# Security Policy

## Supported Versions

Before the first stable release, security fixes target the latest published release/branch unless maintainers explicitly document additional supported versions.

## Reporting a Vulnerability

Do not publish unpatched exploit details in a public issue.

Once the repository is created, enable GitHub Private Vulnerability Reporting and list the exact security contact here. Until then, this document is a policy template and must be finalized before public launch.

A useful report includes:
- affected version/commit;
- impact;
- prerequisites;
- reproduction steps or proof of concept;
- suggested mitigation if known.

## Areas of Special Interest

- path traversal / symlink escape;
- cross-user or cross-library access;
- public share capability bypass;
- session/passkey/auth weaknesses;
- upload/download authorization;
- SSRF;
- stored/reflected XSS through filenames/metadata;
- parser/transcoder sandbox escapes;
- unsafe archive handling;
- secrets in logs/telemetry;
- denial of service through indexing/transcoding/AI/upload.

## Disclosure

Maintainers should acknowledge private reports, reproduce, assess severity, prepare a fix, and coordinate disclosure. Exact response-time promises should only be published once the project has maintainer capacity to meet them.
