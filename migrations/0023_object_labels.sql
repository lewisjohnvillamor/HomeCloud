-- Object labels: what a model recognised in a photograph.
--
-- Written only when a library owner has chosen photo understanding, and
-- deleted when they stop. Everything here is derived: dropping the whole
-- table costs a relabelling pass and nothing else, so it is not part of
-- what a backup must carry (see ADR 0005).
--
-- One row per image that has been looked at, including the ones where
-- nothing was found. "Nothing recognised" is an answer, and recording it
-- is what stops every scan from opening the same photograph of a sky.
--
-- What is recognised, never where: no boxes are stored. Search needs to
-- know there is a dog in the picture, and that is all this keeps.

CREATE TABLE item_objects (
    item_id uuid PRIMARY KEY REFERENCES items (id) ON DELETE CASCADE,
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Which model said so: its file name and the start of its SHA-256.
    -- A different model is a different stamp, and its labels are rebuilt
    -- rather than mixed with the old ones.
    model text NOT NULL,
    -- 'labelled' covers a picture where nothing was found; 'unreadable'
    -- is a file the decoder refused, so it is not retried every scan.
    status text NOT NULL CHECK (status IN ('labelled', 'unreadable')),
    -- Class names, most confident first. Shown to a person as inferred,
    -- never as fact.
    labels text[] NOT NULL DEFAULT '{}',
    -- The best confidence for each label, in the same order.
    confidences real[] NOT NULL DEFAULT '{}',
    -- The words search matches: each label and the other names people
    -- use for it ("puppy", "sofa"). Written by the application from the
    -- same vocabulary that produced the labels.
    search_vector tsvector NOT NULL,
    -- Size and timestamp of the file when it was looked at, so a changed
    -- photograph is looked at again.
    source_size bigint NOT NULL,
    source_modified_at timestamptz,
    labelled_at timestamptz NOT NULL DEFAULT now(),
    CHECK (cardinality(labels) = cardinality(confidences))
);

-- Query shape: the search join, `search_vector @@ tsquery`.
CREATE INDEX item_objects_search ON item_objects USING gin (search_vector);

-- Query shape: "what in this library was labelled by another model", for
-- rebuilds, and deleting a library's labels when the owner turns it off.
CREATE INDEX item_objects_by_library ON item_objects (library_id, model);
