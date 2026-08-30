-- What a photo says about itself.
--
-- A file's modification time is not when the picture was taken: copy a
-- folder of holiday photos to a new disk and every one of them claims to
-- be from today. The camera wrote the real date into the file, so the
-- timeline uses that where it exists and falls back to the file time
-- where it does not.

ALTER TABLE items
    -- When the picture was taken, as the camera recorded it.
    ADD COLUMN taken_at timestamptz,
    -- Make and model as one line, for the detail view.
    ADD COLUMN camera text,
    -- Set once a file has been looked at, so a photo with no metadata at
    -- all is not re-read on every scan.
    ADD COLUMN photo_metadata_at timestamptz;

-- Query shape: the Photos timeline and memories, newest first, by the
-- date the picture was actually taken.
CREATE INDEX items_by_capture_date
    ON items (library_id, (COALESCE(taken_at, modified_at)) DESC)
    WHERE trashed_at IS NULL AND missing_since IS NULL;
