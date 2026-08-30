# Storage and Sync

## 1. Storage Philosophy

The default deployment is **filesystem-first**.

If the configured root is:

```text
/mnt/data/Family
```

then a user-created folder can remain:

```text
/mnt/data/Family/Photos/2026/Tokyo
```

The catalog enriches it; it does not trap it.

## 2. Root Types

### Managed filesystem root
The application may create, rename, move, version, delete, and restore files under this root.

### Imported read/write root
An existing directory. The application indexes it and may mutate only after explicit operator permission.

### Imported read-only root
Index/preview/search only. No mutation.

### S3-compatible root
Objects are managed through an S3 API. Native path recovery guarantees differ from filesystem roots.

## 3. Identity

Do not use path as permanent file identity.

Each catalog item gets an internal stable ID. Identity signals include:
- root ID;
- platform file identifier/inode where safe as a hint;
- current normalized relative path;
- size + modified time;
- BLAKE3 content hash when computed.

A hash is content identity, not logical document identity. Two separate logical files may intentionally have identical bytes.

## 4. Hashing

BLAKE3 is computed in background with bounded I/O concurrency.

Hash states:
- pending;
- computing;
- verified;
- stale because file changed;
- failed.

Before trusting a computed hash, compare stat information captured before/after hashing. If content changed during hashing, discard the result and retry later.

## 5. Duplicates

Duplicate detection has levels:

1. exact content hash;
2. image perceptual similarity;
3. video/media fingerprint similarity;
4. semantic near-duplicate suggestion.

Only level 1 can be called an exact duplicate. No duplicate feature automatically deletes originals.

## 6. Derivatives

Derivatives are disposable/cacheable:
- thumbnails;
- blur placeholders;
- video posters;
- proxy video;
- extracted text;
- embeddings;
- waveform previews.

The user can rebuild them from originals. A backup strategy may exclude derivatives.

## 7. Path Safety

- Canonicalize configured roots at startup.
- Reject traversal outside a root.
- Avoid following symlinks by default; configurable policies must be explicit.
- Treat filesystem names as untrusted data.
- Never use user-controlled filenames directly as temporary paths.
- Normalize for comparison without silently renaming original Unicode filenames.
- Handle case sensitivity differences across filesystems.

## 7a. One Root, Several Disks

A library root is one path, but it need not be one filesystem. Mounting an
external drive at `library/photos` while the rest of the root sits on the
system disk is an ordinary way to run this, and it breaks the two syscalls
the storage layer relies on: `rename` and `hard_link` both refuse to cross a
filesystem boundary.

Both fall back to a copy, and the fallback keeps the guarantee the syscall
was chosen for:

- **Linking** (finishing an upload, restoring a version) falls back when the
  destination is on another filesystem, and also when the filesystem has no
  hard links at all — exFAT and FAT32, which is how most external drives are
  sold. Those report the attempt as `EPERM`, indistinguishable from a genuine
  refusal, so both fall back; a copy that truly is not permitted fails the
  same way and reports the same error.
- **Renaming** (moving, trashing, keeping a version) falls back only on a
  genuine cross-device error. Renaming works on every filesystem, so any other
  failure is real, and copying instead would turn a refusal into a duplicate.

The copy opens its destination with `O_EXCL`, so it still refuses an existing
name in one atomic step — the property that stops two simultaneous uploads of
one name losing each other. A copy that fails partway removes what it wrote,
because a truncated file looks like a real one to every later scan. Nothing is
removed from the source until its copy is complete, so an interrupted move
leaves the original where it was rather than somewhere between two disks.

## 8. Watchers and Reconciliation

Watchers reduce latency but are not authoritative.

Reconciliation loop:
1. enumerate root incrementally;
2. compare with catalog snapshot/cursor;
3. insert/update/move-mark/delete-mark;
4. enqueue enrichment;
5. report drift/errors.

Large roots use checkpoints so rescans are resumable.

## 9. Offline and Device Sync Model

Do not attempt full peer-to-peer arbitrary filesystem synchronization in the first release. v1 introduces the primitives cleanly.

### Server change feed
Every user-visible mutation creates a monotonically ordered change record within the relevant library scope.

A device stores a cursor and asks for changes after that cursor.

### Offline pin
A device can request a set:
- folder;
- album;
- individual file.

The client downloads current versions and tracks server version IDs.

### Conflict
If both client and server changed the same logical file from the same base version:
- preserve both versions;
- never silently overwrite;
- present a conflict with date/device context;
- offer Keep Both, Use Local, Use Server after ensuring the other copy is retained in history.

## 10. Device-Aware Copies — Future Phase

Later, nodes may advertise storage roots and copy availability. The catalog can represent:

```text
asset_id -> content_hash -> replicas[]
```

This enables:
- “only one known copy” warnings;
- request copy to another node;
- offline catalog with online/offline replica state;
- local-first routing to nearest available copy.

This must not be marketed as backup unless the replica is independently verified and backup policy says it qualifies.

## 11. Wake-on-LAN

Wake-on-LAN is an optional availability hook, not part of file integrity.

- store MAC/network wake configuration encrypted where appropriate;
- explicit operator enablement;
- throttle wake requests;
- UI says “Waking server…” and falls back to “Still offline.”

Remote WOL typically needs a network-side relay; document that clearly.

## 12. Version Retention

Suggested defaults:
- keep all versions for 24 hours;
- daily versions for 30 days;
- weekly versions for 12 weeks;
- never exceed operator-defined storage budget without warning.

Actual default should be tested with users before release. Retention cleanup produces candidates, then executes based on deterministic policy with audit records.

## 13. Backup Contract

HomeCloud must provide a backup command/report that identifies:
- PostgreSQL metadata;
- encryption/recovery configuration;
- original storage roots;
- application-managed versions;
- whether derivatives are included;
- restore order.

“Export” must be distinct from “backup.” Export prioritizes portability; backup prioritizes faithful recovery.
