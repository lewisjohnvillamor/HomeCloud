# ADR 0001: Filesystem-First Default Storage

- Status: Accepted for initial architecture
- Date: 2026-08-29

## Context

The project promises self-hosting and data ownership. Many object-storage systems provide excellent scalability but make a normal human-browsable directory tree no longer the direct source of truth.

## Decision

The default backend stores originals as ordinary files under operator-configured filesystem roots. PostgreSQL stores catalog metadata and application state. S3-compatible/RustFS storage is an optional backend profile, not a prerequisite.

## Consequences

Positive:
- uninstalling the application does not make originals inaccessible;
- existing folders can be indexed/imported;
- backups can use normal filesystem tools;
- users can understand where their bytes are.

Costs:
- filesystem portability issues (case sensitivity, permissions, symlinks, network filesystems);
- harder multi-node consistency than object storage;
- application must reconcile external changes.

## Rejected Alternative

Making S3/RustFS mandatory from day one. This improves object semantics but weakens the default “ordinary files survive the app” guarantee and increases deployment complexity.
