/**
 * The icon set.
 *
 * One family, one weight, drawn on a 24×24 grid with a 1.5 stroke in
 * `currentColor` and sized to `1em`, so an icon inherits the colour and
 * size of the text beside it and never drifts from it. Adding an icon
 * means adding a path here, not an SVG somewhere in a component.
 *
 * Icons are companions to labels, never replacements: every one of these
 * is `aria-hidden`, because the word next to it is the accessible name.
 * The rules are in `.claude/skills/taste/SKILL.md`.
 */

export type IconName =
  | "home"
  | "files"
  | "photos"
  | "search"
  | "more"
  | "folder"
  | "file"
  | "download"
  | "upload"
  | "share"
  | "copy"
  | "history"
  | "rename"
  | "trash"
  | "inbox"
  | "star"
  | "star-filled"
  | "play"
  | "check"
  | "close"
  | "back"
  | "scan"
  | "place"
  | "duplicate"
  | "tv";

/**
 * Path data only — every icon shares the same frame, stroke and colour,
 * so the differences between them stay differences of shape.
 */
const PATHS: Record<IconName, string> = {
  home: "M3 10.5 12 3l9 7.5M5.5 9.5V20h13V9.5",
  files: "M4 6.5A1.5 1.5 0 0 1 5.5 5H10l2 2.5h6.5A1.5 1.5 0 0 1 20 9v9a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18Z",
  photos: "M4 6.5A1.5 1.5 0 0 1 5.5 5h13A1.5 1.5 0 0 1 20 6.5v11a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 17.5Zm0 9 4.5-4.5 4 4 3-3L20 15m-6.5-5.5h.01",
  search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13Zm4.6-1.9L20 20",
  more: "M5 12h.01M12 12h.01M19 12h.01",
  folder: "M4 6.5A1.5 1.5 0 0 1 5.5 5H10l2 2.5h6.5A1.5 1.5 0 0 1 20 9v9a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18Z",
  file: "M13 3H7.5A1.5 1.5 0 0 0 6 4.5v15A1.5 1.5 0 0 0 7.5 21h9a1.5 1.5 0 0 0 1.5-1.5V8Zm0 0v5h5",
  download: "M12 4v11m0 0 4-4m-4 4-4-4M5 19h14",
  upload: "M12 20V9m0 0 4 4m-4-4-4 4M5 5h14",
  share: "M12 15V4m0 0 3.5 3.5M12 4 8.5 7.5M5 13v5.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V13",
  copy: "M9 9.5A1.5 1.5 0 0 1 10.5 8h7A1.5 1.5 0 0 1 19 9.5v7a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 9 16.5ZM6 16H5.5A1.5 1.5 0 0 1 4 14.5v-8A1.5 1.5 0 0 1 5.5 5h8A1.5 1.5 0 0 1 15 6.5V7",
  history: "M4 12a8 8 0 1 0 2.3-5.6M4 5v4h4m4-1v4.5l3 2",
  rename: "M4 20h4L19 9a2.1 2.1 0 0 0-3-3L5 17ZM14.5 7.5l2 2",
  trash: "M5 7h14M10 7V5.5A1.5 1.5 0 0 1 11.5 4h1A1.5 1.5 0 0 1 14 5.5V7m-7 0 .8 12.1A1.5 1.5 0 0 0 9.3 20.5h5.4a1.5 1.5 0 0 0 1.5-1.4L17 7",
  inbox: "M4 13h4l1.5 3h5L16 13h4M4 13 6.5 5.5A1.5 1.5 0 0 1 8 4.5h8a1.5 1.5 0 0 1 1.5 1L20 13v5a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18Z",
  star: "m12 4 2.5 5.2 5.5.8-4 3.9 1 5.6-5-2.7-5 2.7 1-5.6-4-3.9 5.5-.8Z",
  "star-filled": "m12 4 2.5 5.2 5.5.8-4 3.9 1 5.6-5-2.7-5 2.7 1-5.6-4-3.9 5.5-.8Z",
  play: "M9 6.5v11l9-5.5Z",
  check: "m4 12.5 5 5L20 6.5",
  close: "M6 6l12 12M18 6 6 18",
  back: "M20 12H4m0 0 6-6m-6 6 6 6",
  scan: "M4 8V5.5A1.5 1.5 0 0 1 5.5 4H8m8 0h2.5A1.5 1.5 0 0 1 20 5.5V8m0 8v2.5a1.5 1.5 0 0 1-1.5 1.5H16m-8 0H5.5A1.5 1.5 0 0 1 4 18.5V16m0-4h16",
  place: "M12 21s7-5.6 7-11a7 7 0 1 0-14 0c0 5.4 7 11 7 11Zm0-8.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z",
  duplicate: "M9 9.5A1.5 1.5 0 0 1 10.5 8h7A1.5 1.5 0 0 1 19 9.5v7a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 9 16.5ZM6 16H5.5A1.5 1.5 0 0 1 4 14.5v-8A1.5 1.5 0 0 1 5.5 5h8A1.5 1.5 0 0 1 15 6.5V7",
  tv: "M4 8.5A1.5 1.5 0 0 1 5.5 7h13A1.5 1.5 0 0 1 20 8.5v8a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 16.5ZM8.5 3.5 12 7l3.5-3.5",
};

export function Icon({ name, className }: { name: IconName; className?: string }) {
  return (
    <svg
      className={className}
      // Sized to the text it sits beside rather than to a fixed pixel
      // size, so it scales with the type and with a browser's own
      // font-size setting.
      width="1em"
      height="1em"
      viewBox="0 0 24 24"
      fill={name === "star-filled" ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
