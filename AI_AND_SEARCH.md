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


## Implementation Status (August 2026)

The non-AI half of search is built and is the foundation the rest attaches to:

- text is extracted from plain-text documents and PDFs during a library scan,
  on a blocking pool, under explicit input and output limits;
- extracted text lives in `item_text` with a generated `tsvector`, and is
  searched together with file names in one ranked query that returns a
  highlighted snippet;
- everything in that table is derived: dropping it costs a rescan and nothing
  else, which is the same rule this document sets for AI-derived data;
- a file that cannot be read records *why* (unsupported, too large, damaged),
  so a scan never reopens a hopeless file, and an unreadable PDF cannot take
  the indexing task down with it.

**OCR is built.** Text recognition is the first provider behind the abstraction
this document describes, and it sets the pattern the rest follow:

- off by default, and off unless a library owner turns it on. The setting is
  per library, owner-only, and stored in `ai_settings`;
- what the owner asked for and what the machine can do are separate answers.
  A deployment without the recogniser reports the capability as absent instead
  of accepting a setting and quietly doing nothing;
- bounded per pass, and last in the scan pipeline, so recognition can never
  starve an upload or a preview;
- writing into the same `item_text` row a document extractor would, marked
  `source = 'ocr'` so search stays one query while AI-derived text can be
  deleted on its own;
- turning it off deletes what it wrote. All of it is derived: dropping it costs
  a rescan and nothing else.

Tesseract rather than a vision-language model, found on `PATH` at runtime as
FFmpeg already is. The job is narrow, a general model is gigabytes and wants a
GPU, and this is tens of megabytes on any processor. The rule the rest of this
phase follows: reach for a small specialised model per job, and only reach for
something bigger when no small tool does the job well.

Still to come, in the order they make sense:

1. **Embeddings** for semantic search, in a sibling table keyed by item. The
   first place ONNX earns its place, since there is no good command-line tool.
2. **Faces**, behind the explicit opt-in §5 requires.
3. **Ask Your Library**, which orchestrates the above plus catalog metadata.

Each step is optional at runtime: with no model configured, search continues to
work exactly as it does today.
