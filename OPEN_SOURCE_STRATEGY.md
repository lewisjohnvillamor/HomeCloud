# Open-Source Strategy

## Goal

Build a project that users can genuinely self-host, inspect, fork, migrate away from, and contribute to without depending on a proprietary control plane.

## License Direction

**Recommended starting position:** AGPL-3.0 for the core server/web application, subject to maintainer review before the first public release.

Why it fits:
- the product is primarily network software;
- improvements made to a hosted modified version remain available to its users under the AGPL network-use provision;
- it protects the community from a closed hosted fork absorbing the core without returning modifications.

Trade-off:
- some companies avoid AGPL dependencies, which can reduce commercial embedding/adoption.

If maximum permissive adoption is the priority instead, Apache-2.0 is the strongest alternative to evaluate because it is permissive and includes an explicit patent license. Do not mix licenses casually; choose deliberately before accepting substantial external contributions.

This document is project strategy, not legal advice. Publish the actual `LICENSE` file only after the maintainers make the final choice.

## What “Open” Means Here

The community edition must not be crippleware. Core self-hosting includes:
- file storage and retrieval;
- accounts/authentication;
- photo timeline and albums;
- memories;
- sharing;
- search;
- mobile/PWA access;
- TV/presentation mode;
- documented backup/restore;
- local AI integrations;
- APIs/protocol integrations needed for normal use.

A future hosted service may sell convenience—managed backups, hosted relay, managed updates, support, or turnkey infrastructure—without making local ownership artificial.

## No Mandatory Cloud Control Plane

A local install must be able to:
- start;
- authenticate local users;
- index/browse files;
- process media;
- search;
- create LAN/private-network shares;
- back up/export;

without contacting project-operated servers.

Optional services such as update checks, relay, remote push, or managed domains must be individually configurable and documented.

## Telemetry

- off by default for self-hosted installs;
- opt-in with a human-readable event schema;
- never collect filenames, paths, media, extracted text, embeddings, faces, share secrets, or search queries;
- provide a local diagnostics export so maintainers can troubleshoot without surveillance telemetry.

## Governance for Early Releases

Before the project has enough contributors for formal governance:
- maintainers own release decisions;
- architecture-changing decisions use ADRs;
- public roadmap and issue labels communicate status;
- security reports follow `SECURITY.md`;
- backwards-incompatible changes require migration notes;
- contributors can challenge decisions with evidence and alternatives.

Later, document maintainer nomination/removal and voting rules rather than inventing bureaucracy before it is needed.

## Extension Philosophy

Prefer stable integration surfaces over forks:
- documented HTTP API;
- WebDAV where appropriate;
- OIDC;
- S3-compatible backend support;
- webhook/event API after security design;
- provider interfaces for OCR/embeddings/AI;
- future plugin SDK only after extension use cases are proven.

Do not create an unrestricted in-process plugin system early; it expands the security and compatibility surface dramatically.

## Release Artifacts

Stable releases should target:
- container image;
- Docker Compose example;
- checksums;
- SBOM;
- signed release provenance/signatures when release infrastructure is mature;
- migration notes;
- backup compatibility notes;
- supported architecture matrix.

Native packages can follow based on community demand.

## Community Quality Bar

The project should be welcoming without lowering engineering standards:
- reproducible bug reports;
- regression tests for fixes;
- design discussion for large features;
- accessibility acceptance criteria;
- no performance claims without measurements;
- no security claims without a defined threat model;
- no “AI feature” that silently sends personal files to a remote provider.
