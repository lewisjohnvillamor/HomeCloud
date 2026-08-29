-- Credentials and sessions.
--
-- See docs/adr/0004-password-sessions-before-passkeys.md: a password is the
-- MVP credential, and the session model is credential-agnostic so passkeys
-- can be added without reworking authorization.

ALTER TABLE users ADD COLUMN password_hash text;

CREATE TABLE sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- The token itself is never stored: a database leak must not hand an
    -- attacker usable sessions.
    token_hash bytea NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);

-- Query shape: "sign out everywhere", and expiry sweeps.
CREATE INDEX sessions_by_user ON sessions (user_id);
CREATE INDEX sessions_by_expiry ON sessions (expires_at);
