-- Extracted document text.
--
-- Derived data: everything here can be rebuilt by rescanning the library,
-- so it enriches search without becoming something a user could lose.

CREATE TABLE item_text (
    item_id uuid PRIMARY KEY REFERENCES items (id) ON DELETE CASCADE,
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Bounded at extraction time; long documents are truncated rather
    -- than refused, because the beginning is usually what identifies a file.
    content text NOT NULL,
    -- Why a file has no text: unsupported format, too large, or damaged.
    -- Recorded so a scan does not retry the same hopeless file forever.
    status text NOT NULL CHECK (status IN ('indexed', 'unsupported', 'too_large', 'failed')),
    -- Size and timestamp of the source when it was read, so a scan can
    -- skip files that have not changed.
    source_size bigint NOT NULL,
    source_modified_at timestamptz,
    extracted_at timestamptz NOT NULL DEFAULT now(),
    -- `simple` rather than a language configuration: a library holds
    -- documents in whatever languages its owner uses, and stemming the
    -- wrong one is worse than not stemming at all.
    search_vector tsvector GENERATED ALWAYS AS (to_tsvector('simple', content)) STORED
);

CREATE INDEX item_text_search ON item_text USING gin (search_vector);

-- Query shape: "which items in this library still need text extracted".
CREATE INDEX item_text_by_library ON item_text (library_id, extracted_at);
