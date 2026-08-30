-- Catalog of library contents.
--
-- The filesystem stays authoritative: rows here describe files, they do
-- not own them. A path is data, not identity — items keep their id when
-- they are renamed or moved.

ALTER TABLE libraries ADD COLUMN root_path text;

CREATE TABLE items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    parent_id uuid REFERENCES items (id) ON DELETE CASCADE,
    -- Library-relative path with `/` separators. The root itself is not
    -- an item; its children have single-segment paths.
    relative_path text NOT NULL CHECK (relative_path <> ''),
    name text NOT NULL CHECK (name <> ''),
    kind text NOT NULL CHECK (kind IN ('file', 'folder')),
    size_bytes bigint NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
    content_type text,
    modified_at timestamptz,
    indexed_at timestamptz NOT NULL DEFAULT now(),
    -- Set when a scan no longer finds the file. Never means "deleted from
    -- disk by HomeCloud"; see the trash flow for that.
    missing_since timestamptz,
    trashed_at timestamptz,
    -- Where the bytes live while the item is in the trash. Only the
    -- trashed item itself has one; its descendants moved with it.
    trash_path text
);

-- One live row per path per library. Trashed rows keep their path so a
-- restore knows where the item came from, hence the partial index.
CREATE UNIQUE INDEX items_unique_live_path
    ON items (library_id, relative_path)
    WHERE trashed_at IS NULL;

-- Query shape: listing a folder, sorted with folders first then by name.
CREATE INDEX items_by_parent ON items (library_id, parent_id, kind, name)
    WHERE trashed_at IS NULL AND missing_since IS NULL;

-- Query shape: the Photos timeline, newest first.
CREATE INDEX items_by_content_type ON items (library_id, content_type, modified_at DESC)
    WHERE trashed_at IS NULL AND missing_since IS NULL;

-- Query shape: name search. `simple` rather than a language configuration:
-- filenames are not prose and must not be stemmed.
CREATE INDEX items_name_search
    ON items
    USING gin (to_tsvector('simple', replace(name, '.', ' ')));

-- Query shape: the trash view, most recently trashed first.
CREATE INDEX items_trashed ON items (library_id, trashed_at DESC)
    WHERE trashed_at IS NOT NULL;
