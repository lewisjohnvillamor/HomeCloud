-- Previous contents of files this app replaced.
--
-- Deliberately narrow: HomeCloud can only keep a version of a change it
-- made itself. If someone edits a file with another program, the old
-- bytes are gone before any scan notices, and pretending otherwise
-- would be a promise the product cannot keep.
--
-- The bytes live in a managed directory inside the library root, moved
-- rather than copied, so replacing a file costs no extra space.

CREATE TABLE content_versions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id uuid NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    -- Name in the version store. Chosen by the server.
    storage_name text NOT NULL UNIQUE,
    size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
    content_type text,
    -- When the file was last written before it was replaced, so the list
    -- reads as a history rather than as a list of upload times.
    content_modified_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    created_by uuid REFERENCES users (id) ON DELETE SET NULL
);

-- Query shape: an item's history, newest first.
CREATE INDEX content_versions_by_item ON content_versions (item_id, created_at DESC);
