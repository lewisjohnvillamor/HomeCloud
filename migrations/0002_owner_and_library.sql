-- Owner and library boundary.
--
-- Only the tables the bootstrap flow needs. File, media, and share tables
-- arrive with the features that use them.

CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 64),
    -- The single account that administers this deployment.
    is_deployment_owner boolean NOT NULL DEFAULT FALSE,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- One deployment owner, enforced by the database so two concurrent
-- bootstrap attempts cannot both succeed.
CREATE UNIQUE INDEX users_one_deployment_owner
    ON users (is_deployment_owner)
    WHERE is_deployment_owner;

CREATE TABLE libraries (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE library_members (
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- RESTRICT: deleting an account must be an explicit decision about
    -- its libraries, never a silent cascade over someone's files.
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    role text NOT NULL CHECK (role IN ('owner', 'member')),
    added_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (library_id, user_id)
);

-- Mirrors the domain rule that a library has exactly one owner.
CREATE UNIQUE INDEX library_members_one_owner
    ON library_members (library_id)
    WHERE role = 'owner';

-- Query shape: "which libraries may this user see", run on every
-- authorization decision.
CREATE INDEX library_members_by_user ON library_members (user_id);
