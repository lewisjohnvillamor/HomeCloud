-- Memories a person does not want to see.
--
-- A memories engine that cannot be told "not this one" is a memories
-- engine that eventually shows somebody a week they would rather not be
-- reminded of. Hiding is per library and reversible, and it hides the
-- memory rather than the photographs: nothing here touches an item, and
-- the pictures stay exactly where they were.

CREATE TABLE hidden_memories (
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- A stable identifier for one memory — "on-this-day-08-30", or a
    -- trip keyed by its first day and place. Deterministic, so the same
    -- memory is still hidden tomorrow.
    memory_key text NOT NULL CHECK (memory_key <> ''),
    hidden_at timestamptz NOT NULL DEFAULT now(),
    hidden_by uuid REFERENCES users (id) ON DELETE SET NULL,
    PRIMARY KEY (library_id, memory_key)
);
