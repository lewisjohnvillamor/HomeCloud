-- Account recovery codes and password-protected share links.

ALTER TABLE users
    -- Argon2 hash of the recovery code, not a plain token: a code is
    -- chosen from a small enough space to be worth attacking offline.
    ADD COLUMN recovery_code_hash text,
    ADD COLUMN recovery_code_set_at timestamptz;

ALTER TABLE shares
    -- Optional second factor on a link, for a channel the sender does
    -- not fully trust.
    ADD COLUMN password_hash text;
