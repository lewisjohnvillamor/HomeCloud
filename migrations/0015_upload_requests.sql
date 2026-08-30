-- Upload request links.
--
-- The mirror image of a share: a share lets someone read one thing, and
-- an upload request lets someone write into one folder without seeing
-- what is already in it. "Send me the wedding photos" is the case, and
-- the person sending them should not need an account.
--
-- This is the only capability in the product that lets an unauthenticated
-- stranger write, so it carries its own limits rather than relying on
-- the ones a session gets.

CREATE TABLE upload_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- The folder files land in. Nothing else in the library is reachable.
    item_id uuid NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Only the hash is stored, the same rule sessions and shares follow.
    token_hash bytea NOT NULL UNIQUE,
    -- Shown to whoever opens the link, so they know what they are
    -- sending to and who asked.
    title text NOT NULL CHECK (title <> ''),
    created_at timestamptz NOT NULL DEFAULT now(),
    -- NULL means "until revoked".
    expires_at timestamptz,
    revoked_at timestamptz,
    -- Bounds on what one link can cost: a stranger with a link must not
    -- be able to fill the disk.
    max_files integer NOT NULL CHECK (max_files > 0),
    max_bytes bigint NOT NULL CHECK (max_bytes > 0),
    received_files integer NOT NULL DEFAULT 0 CHECK (received_files >= 0),
    received_bytes bigint NOT NULL DEFAULT 0 CHECK (received_bytes >= 0),
    last_used_at timestamptz
);

-- Query shape: the owner's list of live request links.
CREATE INDEX upload_requests_live ON upload_requests (library_id, created_at DESC)
    WHERE revoked_at IS NULL;
