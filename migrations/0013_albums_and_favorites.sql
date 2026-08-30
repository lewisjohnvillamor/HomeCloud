-- Curating a library: favorites and albums.
--
-- The two are deliberately different in scope. A favorite is one
-- person's opinion — in a shared family library, what someone stars is
-- theirs, not everyone's. An album is a thing people make together, so
-- it belongs to the library and every member can see it.
--
-- Neither owns any bytes. Both point at catalogued items, so an album
-- survives a file being renamed or moved, and deleting an album never
-- deletes a photo.

CREATE TABLE item_favorites (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id uuid NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, item_id)
);

-- Query shape: one person's favorites, most recently starred first.
CREATE INDEX item_favorites_by_user ON item_favorites (user_id, created_at DESC);

CREATE TABLE albums (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (name <> ''),
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Query shape: the album list for a library.
CREATE INDEX albums_by_library ON albums (library_id, lower(name));

CREATE TABLE album_items (
    album_id uuid NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
    item_id uuid NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    -- Where this picture sits in the album. An album is an arrangement,
    -- not a filter, so the order is the point.
    position bigint NOT NULL,
    added_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (album_id, item_id)
);

-- Query shape: an album's contents, in order.
CREATE INDEX album_items_in_order ON album_items (album_id, position);
