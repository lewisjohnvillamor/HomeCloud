-- Pairing a television with the library.
--
-- A television has no keyboard, so it cannot sign in. Instead it shows a
-- short code, a signed-in person approves it from their phone, and the
-- TV receives a capability of its own: read-only, one library, photos
-- and videos only. It is deliberately narrower than a session — it
-- cannot browse files, search, upload, or see a document.

CREATE TABLE tv_devices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Who vouched for this screen. Removing them removes their TVs.
    approved_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Only the hash is stored, the same rule sessions and shares follow.
    token_hash bytea NOT NULL UNIQUE,
    name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz,
    revoked_at timestamptz
);

-- Query shape: the list of paired screens shown to a library's members.
CREATE INDEX tv_devices_live ON tv_devices (library_id, created_at DESC)
    WHERE revoked_at IS NULL;

CREATE TABLE tv_pairings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Shown on the television and read across the room, so it is short
    -- and drawn from an alphabet without characters people misread.
    code text NOT NULL UNIQUE,
    -- The TV's own secret, so that knowing the code on screen is not
    -- enough to collect the token the approval produces.
    poll_token_hash bytea NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    -- Set when someone approves the code from a signed-in device.
    device_id uuid REFERENCES tv_devices (id) ON DELETE CASCADE,
    -- The token is handed over exactly once, to whoever holds the poll
    -- secret; after that this row is spent.
    collected_at timestamptz
);

-- Query shape: sweeping codes that were never used.
CREATE INDEX tv_pairings_expiry ON tv_pairings (expires_at);
