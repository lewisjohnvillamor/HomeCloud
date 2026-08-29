/**
 * Wire types, mirrored from the Rust API, plus the parsers that keep an
 * unexpected payload from flowing into the UI as `undefined`.
 */

export type Item = {
  id: string;
  name: string;
  path: string;
  kind: "file" | "folder";
  sizeBytes: number;
  contentType: string | null;
  modifiedAt: string | null;
  isImage: boolean;
  trashed: boolean;
};

export type Breadcrumb = { name: string; path: string };

export type Browse = {
  folder: Item | null;
  breadcrumb: Breadcrumb[];
  items: Item[];
};

export type Library = {
  id: string;
  name: string;
  role: string;
  rootPath: string | null;
};

export type Session = {
  authenticated: boolean;
  userId: string | null;
  displayName: string | null;
};

export type ScanStatus = {
  running: boolean;
  finishedAt: string | null;
  scanned: number | null;
  missing: number | null;
  error: string | null;
};

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function text(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

export function parseItem(value: unknown): Item | undefined {
  const raw = record(value);
  if (!raw) {
    return undefined;
  }

  const { id, name, path, kind } = raw;
  if (
    typeof id !== "string" ||
    typeof name !== "string" ||
    typeof path !== "string" ||
    (kind !== "file" && kind !== "folder")
  ) {
    return undefined;
  }

  return {
    id,
    name,
    path,
    kind,
    sizeBytes: typeof raw.size_bytes === "number" ? raw.size_bytes : 0,
    contentType: text(raw.content_type),
    modifiedAt: text(raw.modified_at),
    isImage: raw.is_image === true,
    trashed: raw.trashed === true,
  };
}

export function parseItems(value: unknown): Item[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const items: Item[] = [];
  for (const entry of value) {
    const item = parseItem(entry);
    if (!item) {
      return undefined;
    }
    items.push(item);
  }

  return items;
}

export function parseBrowse(value: unknown): Browse | undefined {
  const raw = record(value);
  if (!raw) {
    return undefined;
  }

  const items = parseItems(raw.items);
  if (!items) {
    return undefined;
  }

  const folder = raw.folder === null || raw.folder === undefined ? null : parseItem(raw.folder);
  if (folder === undefined) {
    return undefined;
  }

  const breadcrumb: Breadcrumb[] = [];
  if (Array.isArray(raw.breadcrumb)) {
    for (const entry of raw.breadcrumb) {
      const crumb = record(entry);
      if (!crumb || typeof crumb.name !== "string" || typeof crumb.path !== "string") {
        return undefined;
      }
      breadcrumb.push({ name: crumb.name, path: crumb.path });
    }
  }

  return { folder, breadcrumb, items };
}

export function parseLibraries(value: unknown): Library[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const libraries: Library[] = [];
  for (const entry of value) {
    const raw = record(entry);
    if (!raw || typeof raw.id !== "string" || typeof raw.name !== "string") {
      return undefined;
    }
    libraries.push({
      id: raw.id,
      name: raw.name,
      role: typeof raw.role === "string" ? raw.role : "member",
      rootPath: text(raw.root_path),
    });
  }

  return libraries;
}

export function parseSession(value: unknown): Session | undefined {
  const raw = record(value);
  if (!raw || typeof raw.authenticated !== "boolean") {
    return undefined;
  }

  return {
    authenticated: raw.authenticated,
    userId: text(raw.user_id),
    displayName: text(raw.display_name),
  };
}

export function parseScanStatus(value: unknown): ScanStatus | undefined {
  const raw = record(value);
  if (!raw || typeof raw.running !== "boolean") {
    return undefined;
  }

  const summary = record(raw.last_summary);

  return {
    running: raw.running,
    finishedAt: text(raw.finished_at),
    scanned: typeof summary?.scanned === "number" ? summary.scanned : null,
    missing: typeof summary?.missing === "number" ? summary.missing : null,
    error: text(raw.last_error),
  };
}
