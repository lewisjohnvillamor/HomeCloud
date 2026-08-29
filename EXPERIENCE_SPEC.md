# Experience Specification

## Design Objective

The interface should feel like a **personal operating system for your data**, not an admin dashboard and not a generic SaaS template.

The visual system should be quiet enough for documents, cinematic enough for memories, and dense enough for serious file management.

## How the Design Skills Apply

- **Taste Skill:** use its anti-generic composition, typography, spacing, visual hierarchy, motion, and landing/media presentation principles where applicable. Its own current guidance says the main taste skill is not intended for dashboards/data tables, so do not force landing-page patterns into the file manager.
- **Vercel Web Interface Guidelines:** binding baseline for product UI behavior, accessibility, forms, keyboard behavior, focus, error recovery, responsive behavior, and React/Next.js correctness.
- Product-specific patterns in this document override generic visual preferences when file-management usability would suffer.

## 1. Visual Language

### Core character
- editorial rather than corporate;
- generous but not wasteful spacing;
- content is the primary visual material;
- controls become quieter when not needed;
- strong typographic hierarchy instead of excessive cards;
- avoid nested rounded rectangles for every information group;
- avoid gradient-heavy “AI SaaS” visuals;
- no meaningless dashboard KPI tiles on Home.

### Surfaces
Use a small number of elevation levels:
1. page canvas;
2. interactive panel/sheet;
3. transient overlay/menu/dialog.

Do not express hierarchy by placing every section inside a card.

### Typography
- Variable sans for UI.
- Tabular figures for sizes, dates, transfer rates.
- Monospace only for paths, hashes, code, and technical metadata.
- Large display type reserved for memories/onboarding, not file tables.

## 2. Navigation

Desktop:

```text
Home
Files
Photos
Memories
Shared
Devices

──────────
Storage roots
  Home
  Archive
  External SSD

──────────
Trash
Settings
```

Mobile uses a compact bottom navigation for Home, Files, Photos, Search, and More. Do not attempt to compress the full desktop sidebar into tiny tabs.

TV uses its own navigation model; never simply stretch desktop UI.

## 3. Global Command Surface

`Cmd/Ctrl + K` opens a universal command/search surface.

It supports:
- navigation;
- recent items;
- file search;
- filters;
- actions;
- natural language when enabled.

Results are grouped clearly and remain keyboard navigable.

Example:

```text
Search files, people, places, or commands…

FILES
  Proposal.pdf                     Documents · 2 MB
  Tokyo 2025                       Album · 184 items

COMMANDS
  Upload files
  Create album
  Start slideshow
```

## 4. File Browser

### Desktop density
Offer Comfortable / Compact density settings. The default should show meaningful information without becoming a spreadsheet.

### Columns
Default:
- Name
- Modified
- Size
- Type/status

Additional optional columns:
- Owner
- Tags
- Original location
- Sync/availability
- Checksum state

### Selection
- click selects/opens according to platform-consistent behavior;
- shift range select;
- command/control multi-select;
- selection bar appears only when items are selected;
- drag selection may be added after baseline accessibility is complete.

### Preview panel
Single click on an item may open a preview side panel on large screens while preserving browsing context. On mobile, use a full-screen preview.

## 5. Photo Timeline

Scrolling should communicate time.

- Zoom/density transitions between year → month → day.
- Sticky date labels are unobtrusive.
- Thumbnail aspect ratios reflect originals where possible rather than forcing every image into identical squares.
- Fast scroll shows a time scrubber.
- Selection mode is explicit on touch.

## 6. Memories

Memories are immersive but respectful.

A memory contains:
- title generated from deterministic rules first, AI optionally;
- date/location context;
- 10–80 selected assets depending on type;
- optional map chapter;
- optional people chapter;
- soundtrack is user-chosen or disabled by default unless licensed/free content is bundled properly.

Controls:
- play/pause;
- previous/next;
- hide this memory;
- hide this person/date/location from memories;
- edit title;
- save as album;
- share.

## 7. Upload Experience

Upload is persistent and non-modal.

A transfer tray shows:
- queued;
- hashing/preparing;
- uploading;
- verifying;
- processing preview;
- complete/error.

Transfers continue when the user navigates elsewhere. Errors identify the failed item and the recovery action.

## 8. Share Experience

The share composer is progressive:

1. choose audience or “Anyone with the link”;
2. choose permission;
3. optional expiration/password/download policy;
4. create link;
5. copy/QR.

Advanced settings stay collapsed until needed.

Public album pages prioritize media and branding-neutral presentation. Public file links prioritize filename, preview, provenance, expiration state, and one primary action.

## 9. Devices and Availability

Do not expose distributed-systems jargon.

Use human descriptions:
- “Available on this server”
- “Also backed up on Archive NAS”
- “Offline — last seen 2 hours ago”
- “Pinned to your phone”
- “Only one known copy”

A technical details disclosure can show node IDs, hashes, roots, and paths.

## 10. TV Interface

Route: `/tv`

### Pairing
The TV displays a short-lived QR/pairing code. A signed-in phone can approve the TV as a limited presentation device.

### Remote model
- Left/right: previous/next
- Up/down: reveal navigation/context
- Enter: select/play/pause
- Back/Escape: go back

### Modes
- Albums
- Memories
- Favorites
- Photo Frame
- Recently Added

The TV token cannot browse private documents unless explicitly granted.

## 11. Empty States

Empty states should teach the next action, not decorate the screen.

Examples:
- Empty Photos: explain phone upload or folder import.
- Empty Shared: “Links you create will appear here” + Create Share.
- Offline device: explain how to reconnect or wake it.

## 12. Error Design

Errors answer three questions:
1. What happened?
2. What is safe/unsafe right now?
3. What can the user do next?

Never use “Something went wrong” without a useful next action when a more specific explanation exists.

## 13. Motion

Motion communicates:
- where an item moved;
- upload/processing progress;
- timeline zoom level;
- opening/closing a spatial panel;
- slideshow transitions.

Rules:
- 120–220 ms for common UI transitions;
- no animation that delays a destructive confirmation or urgent action;
- reduced motion removes non-essential transforms and cinematic effects;
- do not animate large file grids during bulk updates.

## 14. Responsive Breakpoints Are Behavioral

Do not define mobile as “desktop, narrower.”

- Mobile: selection modes, bottom sheets, full-screen preview, bottom navigation.
- Tablet: split view where space permits.
- Desktop: sidebar + content + optional preview pane.
- TV: completely distinct interaction density.

## 15. UI Acceptance Checklist

Every feature PR that changes UI must verify:
- keyboard-only flow;
- visible focus;
- screen-reader naming for controls;
- loading/empty/error/success states;
- mobile viewport;
- desktop viewport;
- reduced motion;
- long filenames and translated/expanded labels;
- slow network behavior;
- optimistic state rollback when mutations fail.
