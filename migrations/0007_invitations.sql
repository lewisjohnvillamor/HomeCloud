-- Invitations to a library.
--
-- Same rules as sessions and share links: the token is a bearer credential,
-- stored only as a hash, with a bounded lifetime.

CREATE TABLE invitations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    -- The role the invitation grants. `owner` is deliberately not
    -- allowed: a library has exactly one owner, and an invitation must
    -- not be able to create a second.
    role text NOT NULL CHECK (role IN ('member')),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    accepted_by uuid REFERENCES users (id) ON DELETE SET NULL,
    revoked_at timestamptz
);

-- Query shape: the owner's list of pending invitations.
CREATE INDEX invitations_pending ON invitations (library_id, created_at DESC)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;
