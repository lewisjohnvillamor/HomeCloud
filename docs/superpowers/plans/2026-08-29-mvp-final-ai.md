# Final MVP Private AI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` when subagents are available, otherwise `superpowers:executing-plans`. Follow TDD, systematic debugging, code review, code simplification, and verification-before-completion.

## Goal

Complete HomeCloud's MVP by shipping local-first AI that makes a user's own library meaningfully searchable and rediscoverable without turning AI into a dependency for file access. This milestone begins only after Roadmap Phases 0–5 satisfy their exit gates.

## Product Contract

MVP AI is **required product scope but optional runtime capability**. Files, Photos, deterministic Memories, sharing, previews, and deterministic search must remain healthy when every AI provider is disabled.

The MVP must include:
- OCR for supported scans/screenshots;
- local text embeddings;
- local image embeddings;
- Ask Your Library natural-language retrieval;
- structured query filters combined with semantic retrieval;
- image caption/tag inference;
- AI-assisted memory ranking/titles;
- explicit opt-in face clustering if enabled in the release profile;
- model/version/delete/rebuild controls;
- bounded AI jobs and operator-visible resource controls;
- no remote AI dependency.

## Task 1: AI provider contracts and capability registry

**Deliverable:** provider-neutral Rust traits and capability discovery for OCR, text embedding, image embedding, image labeling, optional face embedding, and optional local LLM query interpretation.

1. Write failing contract tests for unavailable, healthy, degraded, and version-mismatch providers.
2. Define provider metadata: ID, model/version, locality, modality, dimensions/limits, privacy classification, enabled state.
3. Keep provider-specific SDK types outside domain contracts.
4. Add redacted diagnostics; never log prompts, OCR text, document bodies, or image contents.
5. Verify disabling all providers leaves API startup healthy.

## Task 2: Derived AI metadata schema

**Required skill:** PlanetScale Postgres database-skills.

**Deliverable:** migrations for disposable/versioned derived metadata without mixing it into authoritative file metadata.

1. Write migration tests first.
2. Model source item, provider/model version, modality, created time, stale/rebuild state, and authorization inheritance.
3. Add vector storage only after representative query plans are documented.
4. Ensure deletion of an item cascades/revokes derived metadata safely without touching unrelated originals.
5. Prove an AI metadata purge does not delete source files.

## Task 3: Bounded durable AI job pipeline

**Deliverable:** resumable jobs for OCR/embedding/labeling with strict concurrency and resource controls.

1. Write failing tests for claim/retry/idempotency/cancellation.
2. Prioritize interactive previews/uploads over AI rebuild work.
3. Add per-provider concurrency and memory/CPU configuration.
4. Persist progress so restart does not restart an entire library.
5. Expose queue health without leaking filenames/content in ordinary metrics.

## Task 4: OCR ingestion

**Deliverable:** local OCR for supported images/scans with authorization-safe indexing.

1. Prefer native text extraction for supported PDFs/documents before OCR.
2. Create fixtures with known text and write retrieval tests before implementation.
3. Store OCR as derived searchable text linked to source/model version.
4. Add delete/rebuild tests.
5. Add malicious/oversized input and resource-limit tests.

## Task 5: Local text embeddings

**Deliverable:** chunked text embeddings for supported extracted/OCR content.

1. Define deterministic chunking with stable content/chunk identities.
2. Write tests for changed-file invalidation and unchanged-file reuse.
3. Batch model calls with bounded queues.
4. Version embeddings by model and dimension.
5. Verify permission checks happen before result materialization.

## Task 6: Local image embeddings and labels

**Deliverable:** photo/image semantic vectors plus optional generated labels/captions.

1. Write a small representative image fixture suite and expected coarse retrieval relationships.
2. Generate from bounded derivatives rather than unnecessarily decoding originals at query time.
3. Keep inferred labels visibly separate from EXIF/user tags.
4. Version and rebuild model-derived data.
5. Ensure unsupported/corrupt images fail per-item rather than poisoning the queue.

## Task 7: Hybrid retrieval engine

**Deliverable:** deterministic + lexical + semantic retrieval with one authorization boundary.

1. Write query tests combining text with type/date/path/size/location constraints.
2. Return exact/lexical results quickly while semantic results may stream/append.
3. Never let vector similarity bypass library permissions.
4. Add relevance diagnostics for developers without exposing private content.
5. Measure p95 on a representative synthetic library.

## Task 8: Ask Your Library

**Deliverable:** natural-language search UI/API grounded in authoritative catalog facts.

Representative acceptance queries:
- “Photos from Tokyo near a train station.”
- “Find the PDF invoice from March that mentions a generator.”
- “Screenshots containing a Wi-Fi password warning.”
- “Large videos from last year.”

1. Write acceptance tests for query decomposition into semantic text + deterministic filters.
2. Distinguish catalog facts, extracted text, inferred labels, and generated wording in the response model.
3. When confidence is weak, show search results rather than hallucinating a file-specific answer.
4. Provide direct result provenance: source item, match reason, and relevant metadata.
5. Ensure feature degrades to normal search when AI providers are offline.

## Task 9: AI-assisted Memories

**Deliverable:** AI enriches deterministic Memories without controlling eligibility.

1. Deterministic engine chooses candidate media first.
2. AI may rank, label, or title candidates.
3. Write tests proving AI cannot surface hidden/excluded items.
4. Add regenerate/disable behavior.
5. Clearly mark generated titles where trust requires it.

## Task 10: Face clustering privacy boundary

**Deliverable:** optional face processing isolated behind explicit owner opt-in.

1. Default off.
2. Test library isolation and share-link non-disclosure.
3. Support merge/split/name/hide/delete derived face data.
4. Deleting face data never deletes photos.
5. If the selected local release profile cannot meet acceptable quality/resource targets, keep the module feature-gated but preserve the stable contract; do not block the rest of MVP AI.

## Task 11: AI Control Center UX

**Required guidance:** Vercel Web Interface Guidelines; Taste Skill for polished presentation without turning controls into generic AI-SaaS decoration.

**Deliverable:** user-visible AI status and privacy control surface.

Show:
- enabled capabilities and models;
- local vs remote classification;
- processing queue/progress;
- pause/resume;
- rebuild after model upgrade;
- delete derived AI data;
- per-library face-processing consent;
- resource mode such as Low / Balanced / Fast where supported.

Keyboard, screen-reader, reduced-motion, mobile, and error-state acceptance tests are required.

## Task 12: Local deployment profile

**Deliverable:** documented Compose profile for local AI with no proprietary cloud account.

1. Pin compatible model/provider versions or checksums where licensing permits.
2. Document CPU-only baseline and optional GPU acceleration separately.
3. Never silently download large models without clear operator action/visibility.
4. Add health/readiness that distinguishes “AI unavailable” from “HomeCloud unavailable.”
5. Test clean enable, restart, disable, and model rebuild workflows.

## Task 13: Security, privacy, and abuse review

Review against `SECURITY_AND_PRIVACY.md` and `AI_AND_SEARCH.md`:
- no remote egress by default;
- no AI data crossing library boundaries;
- model/parser inputs resource-limited;
- no content/prompt leakage in logs;
- share links expose only intentionally shared user-facing metadata;
- cancellation/deletion actually stops future use of deleted derived data.

## Task 14: Final MVP review, simplification, and verification

1. Run spec-compliance review against `PRODUCT_SPEC.md`, `AI_AND_SEARCH.md`, `ROADMAP.md`, and this plan.
2. Run relevant security review and browser QA.
3. Apply Anthropic code-simplifier behavior to recently changed code only after tests are green.
4. Rerun all tests after simplification.
5. Run Rust fmt/clippy/tests, frontend lint/typecheck/tests, Playwright AI flows, migration tests, Compose validation, and clean-start smoke tests.
6. Verify representative Ask Your Library queries on both document and photo fixtures.
7. Disable all AI providers and verify core Drive/Photos/Memories flows remain green.
8. Record exact evidence in the PR summary.
9. Only then mark the **MVP Completion Gate** satisfied.

## Out of Scope for This Milestone

- mandatory hosted LLM/API subscriptions;
- autonomous file deletion/mutation based on model output;
- training foundation models;
- cross-user biometric identity inference;
- AI as an authorization source;
- replacing deterministic metadata or search filters with generated guesses.
