-- Bootstrap schema.
--
-- Deliberately minimal: only the state the server needs to describe the
-- deployment itself. Feature tables arrive with the features that use
-- them so migrations stay reviewable.

-- Exactly one row describes this deployment. The `only_row` primary key
-- is constrained to TRUE, so a second insert is rejected by the database
-- rather than by application code that could be bypassed.
CREATE TABLE deployment (
    only_row boolean PRIMARY KEY DEFAULT TRUE CHECK (only_row),
    installed_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO deployment (only_row) VALUES (TRUE);
