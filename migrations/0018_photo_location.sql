-- Where a photo was taken.
--
-- The most sensitive thing in a photo's header: for most people's
-- libraries it is where they live. It is read from EXIF, shown to
-- members of the library, and deliberately never attached to a share
-- link — a link handed to a stranger must not carry someone's address.

ALTER TABLE items
    ADD COLUMN latitude double precision
        CHECK (latitude IS NULL OR (latitude >= -90 AND latitude <= 90)),
    ADD COLUMN longitude double precision
        CHECK (longitude IS NULL OR (longitude >= -180 AND longitude <= 180));

-- Query shape: the photos that have a place, for the map.
CREATE INDEX items_with_location ON items (library_id)
    WHERE latitude IS NOT NULL AND trashed_at IS NULL AND missing_since IS NULL;
