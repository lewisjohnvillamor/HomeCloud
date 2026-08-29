-- Account names identify a person at sign-in, so two accounts must not
-- share one. Compared case-insensitively: "Ada" and "ada" are the same
-- person to everyone except a byte comparison.
CREATE UNIQUE INDEX users_unique_display_name ON users (lower(display_name));
