# Data Model

This is the v1 conceptual PostgreSQL model. Exact SQL belongs in migrations and must be reviewed with the PlanetScale Postgres skill before implementation.

## Principles

- IDs are opaque UUIDv7/ULID-style sortable identifiers where the chosen Rust/DB libraries support them safely.
- Timestamps use `timestamptz`.
- Original filesystem path is metadata, not primary identity.
- Soft-deletion and trash state are explicit.
- Authorization scopes are queryable and indexable.
- Extracted/AI metadata is separable and disposable.

## Tables

### users
- id
- display_name
- status
- created_at
- updated_at

### credentials
- id
- user_id
- kind (`passkey`, recovery, future oidc mapping)
- public credential material/metadata appropriate to kind
- created_at
- last_used_at

Secrets that should not be in plaintext must never be stored unencrypted.

### sessions
- id
- user_id
- token_hash
- expires_at
- created_at
- last_seen_at
- user_agent_summary
- ip_summary optional/privacy-limited

### libraries
Logical ownership/isolation boundary.
- id
- owner_user_id
- name
- created_at

### library_members
- library_id
- user_id
- role
- created_at

### storage_roots
- id
- library_id
- backend_kind
- display_name
- config_encrypted/reference
- write_mode
- online_state
- last_seen_at
- created_at

### items
Represents logical files/folders.
- id
- library_id
- root_id
- parent_id nullable
- kind (`file`, `folder`)
- name
- relative_path
- mime_type nullable
- size_bytes nullable
- filesystem_modified_at nullable
- catalog_version bigint
- content_version_id nullable
- trashed_at nullable
- created_at
- updated_at

Important indexes:
- `(library_id, parent_id, name)` according to collation/case policy;
- `(root_id, relative_path)` unique for live current mapping where feasible;
- `(library_id, updated_at desc)`;
- partial index for non-trashed browsing.

### content_versions
- id
- item_id
- storage_locator
- size_bytes
- blake3 nullable
- hash_state
- source (`current`, `version_store`)
- created_at
- superseded_at nullable

### media_metadata
- item_id/content_version_id
- width
- height
- duration_ms
- captured_at
- latitude/longitude with privacy-aware handling
- camera metadata subset
- orientation
- raw_metadata_json only if bounded/validated

### tags
- id
- library_id
- name
- normalized_name

### item_tags
- item_id
- tag_id

### albums
- id
- library_id
- owner_user_id
- name
- description
- cover_item_id nullable
- created_at
- updated_at

### album_items
- album_id
- item_id
- position/order_hint
- added_at

### extracted_text
- content_version_id
- extractor_version
- language nullable
- text
- tsvector generated/materialized strategy determined during DB design
- created_at

### embeddings
Start with a pluggable representation. If pgvector is selected:
- content_version_id
- model_id
- modality
- vector
- created_at

Do not hard-code one embedding dimension into unrelated tables.

### people_clusters
Opt-in feature.
- id
- library_id
- display_name nullable
- hidden_from_memories boolean
- created_at

### face_observations
- id
- item_id
- people_cluster_id nullable
- bounding box
- embedding reference or vector
- confidence

### shares
- id
- library_id
- created_by_user_id
- target_type
- target_id
- secret_hash
- permission
- expires_at nullable
- password_hash nullable
- max_downloads nullable
- revoked_at nullable
- created_at

### share_events
Privacy-preserving audit of share use when enabled.

### devices
- id
- user_id
- name
- kind
- trust_state
- public_key/material where sync protocol requires
- last_seen_at
- created_at

### change_log
- sequence bigint generated
- library_id
- item_id/aggregate_id
- change_kind
- version
- created_at

Index `(library_id, sequence)`.

### jobs
- id
- kind
- version
- payload jsonb with strict app validation
- state
- priority
- attempts
- max_attempts
- lease_owner nullable
- lease_expires_at nullable
- run_after
- progress jsonb bounded
- last_error_code/message bounded
- created_at
- updated_at

Indexes for claim query must be proven with `EXPLAIN` against realistic cardinality.

### audit_events
- id
- library_id nullable
- actor_user_id nullable
- actor_kind
- action
- target_type/id
- metadata redacted/bounded
- created_at

## Database Review Requirements

Before merging schema/query changes:
1. identify expected cardinality and read/write pattern;
2. define indexes from query shapes rather than intuition;
3. capture `EXPLAIN (ANALYZE, BUFFERS)` in performance-sensitive work when safe on representative data;
4. reason about MVCC, vacuum pressure, row churn, and large JSON fields;
5. use short transactions;
6. avoid holding DB transactions open while doing filesystem/network I/O;
7. define migration lock impact and rollback/roll-forward.
