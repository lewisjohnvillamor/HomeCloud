-- Passkeys.
--
-- See docs/adr/0004-password-sessions-before-passkeys.md: the session
-- model was built credential-agnostic so this table could be added
-- without reworking authorization. A password remains a valid credential
-- for accounts that have one.

CREATE TABLE credentials (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- The authenticator's own identifier for this credential. Unique
    -- across the deployment: one authenticator, one registration.
    credential_id bytea NOT NULL UNIQUE,
    -- The public key and its metadata, as the WebAuthn library stores
    -- it. Public by definition; no secret of the user's is kept here.
    passkey jsonb NOT NULL,
    nickname text NOT NULL CHECK (char_length(nickname) BETWEEN 1 AND 64),
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz
);

-- Query shape: "which passkeys may sign this person in".
CREATE INDEX credentials_by_user ON credentials (user_id);
