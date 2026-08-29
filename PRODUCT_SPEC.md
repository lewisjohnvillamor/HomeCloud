# Product Specification

## 1. Product Definition

Project HomeCloud is a self-hosted personal data cloud for users who want the convenience of mainstream cloud drives and photo services without surrendering ownership of storage or requiring a hosted provider.

The primary deployment is a continuously available home server, NAS, workstation, or mini-PC. The product must also tolerate a workstation that sleeps or powers off and clearly communicate availability instead of pretending the cloud is always online.

## 2. Jobs to Be Done

### Core

1. Store any file type on hardware I control.
2. Reach my library from every device I own.
3. Send someone a safe link to a file, folder, album, or drop box.
4. Automatically back up photos and videos from my phone.
5. Find an item without remembering its exact filename or folder.
6. Enjoy photos through memories, albums, maps, and large-screen presentation.
7. Know whether my data is safe, duplicated, backed up, and reachable.
8. Export everything without proprietary recovery tooling.

### Advanced

1. Ask natural-language questions about my library without uploading private data to an AI SaaS.
2. Keep selected content on specific devices for offline access.
3. Discover duplicates and storage waste without deleting anything automatically.
4. Connect multiple storage roots or servers into one logical library.
5. Use standards-based clients through WebDAV and optional S3-compatible interfaces.

## 3. Personas

### Personal user
Owns a desktop/NAS and wants Google Drive/Photos convenience with local storage.

### Family administrator
Runs one server for several household members, with private libraries and selectively shared family spaces.

### Power user / homelabber
Wants multiple disks, reverse proxy/VPN access, monitoring, APIs, WebDAV, S3, local AI, and deterministic backup behavior.

### Creative / professional
Needs RAW images, large videos, previews, metadata search, delivery links, upload-request links, and reliable resumable transfers.

## 4. Product Surfaces

### Home
A useful “today” view rather than a generic dashboard:

- recent uploads;
- continue where you left off;
- memories;
- recent documents;
- storage/backup warnings only when actionable;
- device backup health;
- quick actions: Upload, Scan, Create share, Start slideshow.

### Files
- list/grid modes;
- column browsing on large screens;
- virtualized folders;
- quick preview panel;
- drag-and-drop and paste upload;
- copy/move/rename/tag/favorite;
- version history;
- trash and restore;
- checksum and technical info on demand;
- external/open-in-native-app actions where supported.

### Photos
- chronological timeline with year/month/day density transitions;
- albums and smart albums;
- people/place/event views;
- map exploration;
- favorites and archive;
- motion media support;
- burst grouping;
- duplicate stacks;
- non-destructive edits to app metadata;
- original download at all times.

### Memories
- On This Day;
- trips and location clusters;
- people clusters;
- seasons/year in review;
- “recently rediscovered” old photos;
- user-created stories;
- dismiss/hide controls so resurfacing remains respectful.

### Search / Command Bar
One command surface for navigation and action:

- exact filename;
- extension/type;
- metadata filters;
- date/location/person;
- full-text document content;
- natural language when semantic search is enabled;
- commands such as “upload,” “new album,” or “share current folder.”

### Shared Links
Recipients should see a beautiful, minimal page rather than an admin UI. Supported grant types:

- view/download;
- upload-only drop box;
- collaborative folder;
- album/slideshow;
- expiring one-time link;
- password-protected link.

### Devices
- last seen;
- backup status;
- available storage;
- offline pin sets;
- trusted/revoked status;
- per-device transfer history;
- eventual multi-device copy count.

### TV / Presentation
- remote/arrow-key navigation;
- QR code pairing to phone;
- photo frame mode;
- memories and album playback;
- slideshow interval and transition controls;
- no tiny text or desktop-only controls;
- optional cast handoff.

## 5. Cutting-Edge Must-Haves

### 5.1 Instant everything
- Skeletons only where real loading exists.
- Directory results stream progressively.
- Thumbnails have blurhash/placeholder then sharpen.
- Navigation is speculative/prefetched.
- Upload appears in UI immediately while transfer continues.
- Search responds with lexical results before slower semantic enrichment.

### 5.2 Zero-setup private intelligence
Private AI capability is part of the **MVP** and is delivered as its final milestone. AI remains runtime-optional, local-first, and provider-pluggable: a user can disable every AI worker and still retain a complete, healthy Files/Photos/Memories experience.

The MVP AI profile includes:
- OCR for screenshots/scans;
- image embeddings;
- document embeddings;
- face clustering with explicit opt-in;
- natural-language **Ask Your Library** search;
- image caption/tag inference;
- AI-assisted memory ranking/titles on top of deterministic memory rules;
- semantic duplicate/near-duplicate detection.

### 5.3 Content identity
Every indexed file can receive a BLAKE3 content hash asynchronously. This enables:
- exact duplicate detection;
- move/rename recognition;
- integrity checking;
- resumable transfer verification;
- copy-count visualization.

### 5.4 Storage transparency
Never tell users “it is backed up” unless the system can prove it. Show:
- original location;
- indexed vs fully processed status;
- replicas/copies known to the system;
- latest verified backup/checksum date where configured.

### 5.5 On-demand availability
If a storage node is sleeping/offline:
- mark items as temporarily unavailable without losing catalog visibility;
- show last known metadata and thumbnails where allowed;
- optionally support Wake-on-LAN hooks;
- retry safely when the node returns.

### 5.6 Universal preview
Preview common images, videos, audio, PDFs, text, Markdown, office documents, archives, code, and metadata without forcing a download. Unsupported formats fall back to safe metadata + download.

### 5.7 Upload request links
Create a secure link that lets another person send files directly to a chosen folder without seeing its contents.

### 5.8 Offline collections
A user can pin an album/folder to a device. The UI must distinguish:
- online only;
- downloading;
- available offline;
- stale/changed;
- conflict requiring attention.

### 5.9 Time-travel / versions
Files support retained versions with user-configurable policy. v1 may use copy-on-write/version objects rather than filesystem snapshots so behavior is portable.

### 5.10 Delight without gimmicks
Animations explain spatial/temporal changes. No decorative motion may block interaction. Respect `prefers-reduced-motion`.

## 6. Functional Requirements for v1

### Files
- Scan one or more configured storage roots.
- Preserve filesystem paths and originals.
- Upload single/multiple/folder.
- Resumable upload for large files.
- HTTP range download/stream.
- Create/rename/move/copy/delete/restore.
- Favorite/tag.
- Version history.
- Public/private sharing.

### Media
- EXIF/XMP read.
- Image thumbnails.
- Video poster and proxy/transcode derivative.
- Timeline and albums.
- Memories rules engine.
- Slideshow/TV mode.

### Search and AI
- Filename/path/type/date/size.
- PostgreSQL full-text for extracted text.
- Local OCR for supported scans/screenshots when needed.
- Local text and image embeddings.
- Ask Your Library natural-language retrieval across documents and photos.
- Image caption/tag inference for discoverability.
- AI-assisted memory enrichment.
- Query filters must remain usable when semantic search is disabled.
- AI-derived metadata must be versioned, removable, and rebuildable.
- Remote AI must never be required for the MVP.

### Identity
- Initial owner bootstrap.
- Passkey/WebAuthn sign-in.
- Recovery codes.
- Optional password fallback behind explicit configuration.
- Multi-user local accounts.
- Roles: owner, admin, member, guest-link capability.

### Operations
- Health/readiness endpoints.
- Structured logs and traces.
- Migration safety.
- Export metadata.
- Documented backup/restore.
- Graceful shutdown and resumable jobs.

## 7. Explicit Non-Goals for v1

- Building a full Google Docs clone.
- Real-time collaborative office editing from scratch.
- Federated social network features.
- Automatic deletion of duplicates.
- Custom distributed consensus system.
- Custom video codec.
- Custom database.
- Custom object store.
- Mandatory Kubernetes.

Office document editing can be integrated later through Collabora/OnlyOffice or “open in local application.”

## 8. Performance Targets

These are release targets, not marketing claims:

- Cached API p95 under 150 ms on a typical home LAN for small metadata requests.
- First meaningful file listing content under 500 ms for common folders after warm start.
- Support folders with 100k logical children through pagination/virtualization without loading all rows client-side.
- Range streaming must begin without buffering an entire media file.
- Resumable transfers survive browser refresh and transient network loss.
- Indexing is incremental and does not block browsing.
- Background CPU-heavy work is bounded and deprioritized during interactive requests.

## 9. Accessibility Acceptance

- WCAG 2.2 AA target for primary workflows.
- Full keyboard operation.
- Visible `:focus-visible` state.
- Focus never hidden behind sticky UI.
- Screen reader labels for icon-only controls.
- 44x44 CSS px touch targets where feasible for primary touch controls.
- Reduced-motion mode.
- TV interface operable by arrow keys + Enter + Back.

## 10. Success Metrics

For a self-hosted open-source product, success is not only active users:

- time from install to first indexed library;
- restore/export success rate;
- failed transfer rate;
- crash-free sessions;
- indexing throughput and queue age;
- issue response quality;
- upgrade success across supported versions;
- number of core tasks possible without proprietary/hosted dependencies.
