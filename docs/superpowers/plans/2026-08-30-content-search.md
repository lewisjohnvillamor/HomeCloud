# Content Search Implementation Plan

> Phase 6 groundwork from `ROADMAP.md`. Search currently matches file names
> only, so "the invoice that mentions a generator" is unfindable.

**Goal:** Search finds a document by what is written inside it, with a snippet
showing why it matched — with no AI model required, and with a seam where
embeddings and OCR will attach later.

**Architecture:** A `search` crate that extracts text from a file's bytes under
explicit limits (pure CPU, no I/O), an `item_text` table with a generated
`tsvector`, extraction folded into the existing library scan, and one query
that ranks name and content matches together.

## Tasks

1. **Extraction** — plain text and PDF, decided by content and extension, with
   caps on input size and extracted length. A file that cannot be read is
   recorded as such rather than retried forever.
2. **Storage** — `item_text` keyed by item, carrying the extracted text, a
   generated `tsvector`, and a status. Rebuildable: dropping the table costs a
   rescan and nothing else.
3. **Indexing** — runs as part of a scan, after reconciliation, on the blocking
   pool, skipping files whose size and timestamp have not changed.
4. **Query** — one search that matches names and content, ranks them, and
   returns a highlighted snippet for content matches.
5. **UI** — search results show why they matched.
6. **Adversarial tests** — a huge file, a PDF that is not a PDF, a file full of
   null bytes, and hostile query strings all behave.

## Self-Review

- OCR for scanned documents and embeddings for semantic search are the next
  two steps and both attach to `item_text`; neither is in this plan.
- Office formats (docx, xlsx, pptx) are deliberately absent: each is a zip of
  XML with its own parser surface, and they deserve their own review.
