-- Exact-duplicate detection.
--
-- A personal library accumulates the same photo three times: once from
-- the camera, once from a message, once from a backup someone copied in
-- "just in case". Finding those is the first thing that reclaims real
-- space, and it needs a content hash rather than a name or a size.
--
-- BLAKE3 because it is fast enough to hash a library in the background
-- without the machine noticing, and a mismatch is decisive: two files
-- with the same hash are the same bytes.

ALTER TABLE items
    -- NULL means "not hashed yet". Hashing is a background pass, so most
    -- of a fresh library is NULL for a while and that is not an error.
    ADD COLUMN content_hash bytea,
    -- What the file looked like when it was hashed. If either has moved
    -- on, the hash is stale and the file is queued again rather than
    -- trusted.
    ADD COLUMN hashed_size bigint,
    ADD COLUMN hashed_modified_at timestamptz,
    ADD COLUMN hashed_at timestamptz;

-- Query shape: find the groups that share a hash within one library.
CREATE INDEX items_by_content_hash ON items (library_id, content_hash)
    WHERE content_hash IS NOT NULL AND trashed_at IS NULL AND missing_since IS NULL;

-- Query shape: what still needs hashing.
CREATE INDEX items_awaiting_hash ON items (library_id)
    WHERE content_hash IS NULL AND kind = 'file' AND trashed_at IS NULL AND missing_since IS NULL;
