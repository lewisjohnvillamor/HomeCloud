# ADR 0002: AI Is Optional and Local-First

- Status: Accepted for initial architecture
- Date: 2026-08-29

## Context

Semantic search, OCR, visual understanding, face clustering, and memory ranking can materially improve a personal library, but private data should not be silently uploaded to third parties and the product must remain useful on modest hardware.

## Decision

All core workflows function with AI disabled. AI features use provider interfaces. Default recommended providers are local. Remote providers, if added, require explicit opt-in and visible privacy policy/configuration.

## Consequences

- deterministic filename/metadata/full-text search remains first-class;
- embeddings/labels are derived and rebuildable;
- model versions are tracked;
- face clustering is explicit opt-in;
- UI must handle capabilities dynamically.
