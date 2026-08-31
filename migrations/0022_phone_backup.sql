-- Phone backup: remembering which device sent what, and when.
--
-- A browser cannot read a camera roll on its own, so this is not the
-- background service a native app would give you. It is the honest
-- version of it: somebody opens a page, selects everything, and only
-- the photographs this library does not already hold are sent. What
-- makes that bearable to repeat every week is knowing what arrived
-- last time, which is what this table is for.
--
-- The photographs themselves are ordinary files in an ordinary folder,
-- as everything else here is. Nothing in this table owns any bytes, so
-- losing all of it costs a device name and a date.

CREATE TABLE backup_devices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id uuid NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- A device belongs to the person who set it up. In a family library
    -- my phone's photographs are filed under my name, and another
    -- member's backup cannot be renamed or removed by me.
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- What the person called it: "Ada's phone".
    name text NOT NULL CHECK (name <> ''),
    -- Library-relative folder its photographs land in. Stored rather
    -- than derived, so renaming a device later cannot orphan the
    -- pictures already filed under the old name.
    folder text NOT NULL CHECK (folder <> ''),
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Null until the first backup finishes. "Never" is a real answer
    -- and should read as one rather than as the epoch.
    last_backup_at timestamptz
);

-- One device of a given name per person per library. Backing up twice
-- from the same phone continues the same device rather than making a
-- second one beside it.
CREATE UNIQUE INDEX backup_devices_unique_name
    ON backup_devices (library_id, user_id, lower(name));

-- Two devices cannot share a folder, whoever owns them: that would let
-- one person's backup write into another's.
CREATE UNIQUE INDEX backup_devices_unique_folder
    ON backup_devices (library_id, folder);

-- Query shape: "what has this person got set up here", for the list in
-- More and for the backup page itself.
CREATE INDEX backup_devices_by_member ON backup_devices (library_id, user_id);
