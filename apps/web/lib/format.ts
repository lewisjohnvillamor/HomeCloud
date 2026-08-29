/** Human-readable file size. Uses decimal units, as file managers do. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "—";
  }
  if (bytes < 1000) {
    return `${bytes} B`;
  }

  const units = ["kB", "MB", "GB", "TB", "PB"];
  let value = bytes / 1000;
  let unit = 0;

  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }

  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

/**
 * Date for display. Rendered in the viewer's locale on the client; the
 * ISO string is kept as the accessible title so the exact value is
 * always available.
 */
export function formatDate(iso: string | null): string {
  if (!iso) {
    return "—";
  }

  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "—";
  }

  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

/** Joins a folder path and a name into a library-relative path. */
export function joinPath(folder: string, name: string): string {
  return folder ? `${folder}/${name}` : name;
}

/** The folder containing a path, or "" for something in the root. */
export function parentOf(path: string): string {
  const index = path.lastIndexOf("/");

  return index === -1 ? "" : path.slice(0, index);
}
