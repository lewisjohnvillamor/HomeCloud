-- Private AI: the switch, and where AI-derived text goes.
--
-- Off is the default and stays the default. Everything in the product
-- works with no model configured, and turning AI off must leave Files,
-- Photos, Memories, sharing, and deterministic search healthy — so
-- nothing here is a dependency of anything, only an addition to it.

CREATE TABLE ai_settings (
    library_id uuid PRIMARY KEY REFERENCES libraries (id) ON DELETE CASCADE,
    -- What the owner has turned on, in increasing cost:
    --   off    — nothing runs. The default, and the absence of a row.
    --   text   — OCR and, later, document embeddings. Cheap; a NAS can do it.
    --   photos — adds image understanding. Wants a real processor.
    --   people — adds face grouping, which §5 requires be opted into
    --            explicitly rather than arriving with an upgrade.
    profile text NOT NULL DEFAULT 'off'
        CHECK (profile IN ('off', 'text', 'photos', 'people')),
    updated_at timestamptz NOT NULL DEFAULT now(),
    updated_by uuid REFERENCES users (id) ON DELETE SET NULL
);

-- Where a piece of text came from.
--
-- Search does not care — the whole point is that OCR writes into the
-- same row a document extractor would, so one query covers both. This
-- column exists so AI-derived text can be deleted on its own when
-- someone turns the feature off, without touching anything read
-- straight out of a file.
ALTER TABLE item_text
    ADD COLUMN source text NOT NULL DEFAULT 'extracted'
        CHECK (source IN ('extracted', 'ocr'));

-- Query shape: "what has AI written here", for deletion and rebuilds.
CREATE INDEX item_text_by_source ON item_text (library_id, source)
    WHERE source <> 'extracted';
