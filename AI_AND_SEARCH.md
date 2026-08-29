# AI and Search

## Principle

AI is an enhancement layer, never the foundation required to access data.

## MVP Commitment

Private AI ships in the MVP as the **final MVP milestone** after storage, sharing, photos, and deterministic memories are proven. This means the repository must ship a documented local AI profile and working AI experiences before the MVP is declared complete.

The MVP AI surface includes:
- OCR for supported scans/screenshots where normal extraction is insufficient;
- local text embeddings;
- local image embeddings;
- Ask Your Library natural-language retrieval across documents and photos;
- structured filter extraction so semantic search remains grounded in dates/types/paths/locations;
- image caption/tag inference;
- AI-assisted memory ranking/titling;
- model/provider health, queue, pause, delete, and rebuild controls;
- explicit opt-in face clustering as an MVP-eligible feature, isolated from the rest of AI so privacy-sensitive users can leave it disabled.

**Runtime optionality is not scope optionality.** The implementation must exist and be supported in MVP, while the core product must continue to work when AI is disabled or unavailable.

## 1. Search Pipeline

A query may execute in parallel stages:

1. command/navigation matches;
2. exact/prefix filename/path matches;
3. metadata filters;
4. PostgreSQL full-text extracted content;
5. optional semantic/vector retrieval;
6. optional reranking.

The UI can render fast deterministic results first and append semantic results with clear grouping.

## 2. Local-First Providers

Define provider traits/contracts for:
- OCR;
- text embedding;
- image embedding;
- face detection/embedding;
- image caption/tag inference;
- optional local LLM query interpretation.

No single model implementation is hard-coded into domain logic.

Provider configuration records:
- model identifier/version;
- local or remote execution;
- supported modalities;
- dimensions/token limits;
- privacy classification;
- operator enabled/disabled state.

## 3. Semantic Search

Examples:
- “sunset photos by the lake”;
- “receipt for a monitor bought last year”;
- “documents talking about renewal dates.”

Natural language should be parsed into both semantic text and deterministic filters where possible. A date like “last year” should become an explicit date range based on the server/user locale rather than relying only on embedding similarity.

## 4. OCR

Candidate files:
- scanned PDFs;
- images/screenshots;
- supported office/PDF text extraction first, OCR only where needed.

OCR text is searchable metadata and may be deleted/rebuilt. It must inherit the original item's authorization.

## 5. Faces and People

Face processing is sensitive. Requirements:
- disabled by default until explicit library-owner opt-in;
- local-only default provider;
- never expose clusters across library boundaries;
- user can merge/split/name/hide/delete face-derived metadata;
- deleting face metadata does not touch originals;
- public shares do not expose person-cluster identities unless intentionally included in public UI.

## 6. Memories Engine

Start deterministic before generative.

Signals:
- date clusters;
- location clusters;
- recurring calendar dates;
- favorites;
- album membership;
- people clusters if enabled;
- image quality/basic blur scoring;
- duplicate suppression.

AI may help title/rank a memory but the engine must function without it.

## 7. Model Upgrades

Embeddings and derived labels are versioned by model. Model changes do not overwrite old index state in place without a migration/rebuild strategy.

Use job priorities so model rebuilds never starve uploads/previews.

## 8. Remote AI

If remote providers are ever supported:
- opt-in per provider;
- explicit UI statement that selected content may leave the server;
- configurable file/type/size policy;
- do not send entire libraries for convenience;
- redact metadata where possible;
- document provider retention implications;
- allow one-click disable and deletion of locally stored remote-derived metadata.

## 9. Safety and Trust

The assistant/search UI must distinguish:
- factual metadata from the catalog;
- extracted/OCR text;
- inferred labels;
- AI-generated summaries.

When answering “where is this file?”, use authoritative catalog data, not model inference.
