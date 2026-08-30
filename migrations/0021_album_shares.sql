-- Sharing an album.
--
-- A share has always pointed at one item. An album is not an item — it
-- owns no bytes and has no path — so a link to one needs its own
-- reference rather than a folder standing in for it, which would share
-- whatever else happened to be in that folder.

ALTER TABLE shares
    ALTER COLUMN item_id DROP NOT NULL,
    ADD COLUMN album_id uuid REFERENCES albums (id) ON DELETE CASCADE,
    -- Exactly one of the two. A share of both, or of neither, is not a
    -- thing the reader knows how to answer.
    ADD CONSTRAINT shares_point_at_one_thing CHECK (
        (item_id IS NOT NULL AND album_id IS NULL)
        OR (item_id IS NULL AND album_id IS NOT NULL)
    );

-- Query shape: the share list shown next to an album.
CREATE INDEX shares_by_album ON shares (album_id, created_at DESC)
    WHERE album_id IS NOT NULL;
