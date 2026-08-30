-- Resumable uploads.
--
-- A single request is fine for a photo and hopeless for a 40 GB video
-- over house wifi: one dropped connection and the whole thing starts
-- again. A session records where an upload had got to, so a client that
-- comes back asks "how much did you get?" and continues from there.
--
-- The bytes live in a staging file inside the library root, exactly as a
-- single-request upload's do, and are moved into place only when the
-- whole file has arrived.

CREATE TABLE upload_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Where the finished file should land. Checked again at completion,
    -- because a name can be taken while an upload is in flight.
    destination_path text NOT NULL CHECK (destination_path <> ''),
    -- What the client said the file is. The server never trusts it for
    -- anything but refusing an upload that is too big up front.
    declared_size bigint NOT NULL CHECK (declared_size >= 0),
    -- Name of the staging file, chosen by the server.
    staging_name text NOT NULL UNIQUE,
    -- Authoritative offset is the staging file's own length; this is a
    -- cache for listing sessions without touching the disk.
    received_bytes bigint NOT NULL DEFAULT 0 CHECK (received_bytes >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    completed_at timestamptz
);

-- Query shape: a person's unfinished uploads, and the expiry sweep.
CREATE INDEX upload_sessions_live ON upload_sessions (library_id, created_by, updated_at DESC)
    WHERE completed_at IS NULL;

CREATE INDEX upload_sessions_expiry ON upload_sessions (expires_at)
    WHERE completed_at IS NULL;
