-- Public share links.
--
-- A share is a capability: it grants read access to exactly one item (and,
-- for a folder, what is inside it) and nothing else. It is deliberately
-- narrower than a session — it cannot browse the library, upload, rename,
-- delete, or see any other item.

CREATE TABLE shares (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    item_id uuid NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Only the hash is stored: a database copy must not yield working
    -- links, the same rule sessions follow.
    token_hash bytea NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- NULL means "until revoked".
    expires_at timestamptz,
    revoked_at timestamptz,
    -- Observability for the owner: "this link has been used 14 times".
    access_count bigint NOT NULL DEFAULT 0,
    last_accessed_at timestamptz
);

-- Query shape: the share list shown next to an item.
CREATE INDEX shares_by_item ON shares (item_id, created_at DESC);

-- Query shape: expiry sweeps and the owner's list of live links.
CREATE INDEX shares_live ON shares (library_id, expires_at)
    WHERE revoked_at IS NULL;
