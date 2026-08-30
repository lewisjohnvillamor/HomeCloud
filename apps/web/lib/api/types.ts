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
  /** When the picture was taken, as the camera recorded it. */
  takenAt: string | null;
  /** The camera, as one line: "Fujifilm X100V". */
  camera: string | null;
  isImage: boolean;
  isVideo: boolean;
  /** Where the picture was taken. Never present on a shared link. */
  latitude: number | null;
  longitude: number | null;
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
  /**
   * Present only on the response that creates or recovers an account.
   * Shown once and never returned again, so the UI has to display it
   * there and then.
   */
  recoveryCode: string | null;
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
    takenAt: text(raw.taken_at),
    camera: text(raw.camera),
    latitude: typeof raw.latitude === "number" ? raw.latitude : null,
    longitude: typeof raw.longitude === "number" ? raw.longitude : null,
    isImage: raw.is_image === true,
    isVideo: raw.is_video === true,
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
    recoveryCode: text(raw.recovery_code),
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

export type Share = {
  id: string;
  itemId: string;
  itemName: string;
  createdAt: string;
  expiresAt: string | null;
  accessCount: number;
  /** Whether opening the link also needs a password. */
  protected: boolean;
  /** Present only on the response that created the share. */
  token: string | null;
};

export function parseShare(value: unknown): Share | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string" || typeof raw.item_id !== "string") {
    return undefined;
  }

  return {
    id: raw.id,
    itemId: raw.item_id,
    itemName: typeof raw.item_name === "string" ? raw.item_name : "",
    createdAt: text(raw.created_at) ?? "",
    expiresAt: text(raw.expires_at),
    accessCount: typeof raw.access_count === "number" ? raw.access_count : 0,
    protected: raw.protected === true,
    token: text(raw.token),
  };
}

export function parseShares(value: unknown): Share[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const shares: Share[] = [];
  for (const entry of value) {
    const share = parseShare(entry);
    if (!share) {
      return undefined;
    }
    shares.push(share);
  }

  return shares;
}

/** What a visitor holding a share link can see. */
export type PublicShare = {
  item: Item;
  items: Item[];
  relativePath: string;
};

export function parsePublicShare(value: unknown): PublicShare | undefined {
  const raw = record(value);
  if (!raw) {
    return undefined;
  }

  const item = parseItem(raw.item);
  const items = parseItems(raw.items);
  if (!item || !items) {
    return undefined;
  }

  return {
    item,
    items,
    relativePath: typeof raw.relative_path === "string" ? raw.relative_path : "",
  };
}

export type Member = {
  userId: string;
  displayName: string;
  role: "owner" | "member";
  addedAt: string;
  isYou: boolean;
};

export function parseMembers(value: unknown): Member[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const members: Member[] = [];
  for (const entry of value) {
    const raw = record(entry);
    if (!raw || typeof raw.user_id !== "string" || typeof raw.display_name !== "string") {
      return undefined;
    }

    members.push({
      userId: raw.user_id,
      displayName: raw.display_name,
      role: raw.role === "owner" ? "owner" : "member",
      addedAt: text(raw.added_at) ?? "",
      isYou: raw.is_you === true,
    });
  }

  return members;
}

export type Invitation = {
  id: string;
  libraryName: string;
  invitedBy: string;
  createdAt: string;
  expiresAt: string;
  /** Present only on the response that created it. */
  token: string | null;
};

export function parseInvitation(value: unknown): Invitation | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string") {
    return undefined;
  }

  return {
    id: raw.id,
    libraryName: text(raw.library_name) ?? "",
    invitedBy: text(raw.invited_by) ?? "",
    createdAt: text(raw.created_at) ?? "",
    expiresAt: text(raw.expires_at) ?? "",
    token: text(raw.token),
  };
}

export function parseInvitations(value: unknown): Invitation[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const invitations: Invitation[] = [];
  for (const entry of value) {
    const invitation = parseInvitation(entry);
    if (!invitation) {
      return undefined;
    }
    invitations.push(invitation);
  }

  return invitations;
}

/** What someone holding an invitation is told before accepting. */
export type InvitationPreview = {
  libraryName: string;
  invitedBy: string;
  expiresAt: string;
};

export function parseInvitationPreview(value: unknown): InvitationPreview | undefined {
  const raw = record(value);
  if (!raw || typeof raw.library_name !== "string") {
    return undefined;
  }

  return {
    libraryName: raw.library_name,
    invitedBy: text(raw.invited_by) ?? "",
    expiresAt: text(raw.expires_at) ?? "",
  };
}

/** A search hit: an item plus why it matched. */
export type SearchResult = Item & {
  matched: "name" | "content" | "name_and_content";
  /** Passage around a content match, marked with `<<` and `>>`. */
  snippet: string | null;
};

export function parseSearchResults(value: unknown): SearchResult[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const results: SearchResult[] = [];
  for (const entry of value) {
    const item = parseItem(entry);
    const raw = record(entry);
    if (!item || !raw) {
      return undefined;
    }

    const matched = raw.matched;

    results.push({
      ...item,
      matched:
        matched === "content" || matched === "name_and_content" || matched === "name"
          ? matched
          : "name",
      snippet: text(raw.snippet),
    });
  }

  return results;
}

/**
 * Splits a server-highlighted snippet into plain and matched segments.
 *
 * The server marks matches with `<<` and `>>` rather than HTML so a
 * document's own contents can never become markup on the page.
 */
export function snippetSegments(snippet: string): { text: string; matched: boolean }[] {
  const segments: { text: string; matched: boolean }[] = [];
  let rest = snippet;

  while (rest.length > 0) {
    const start = rest.indexOf("<<");
    const end = start === -1 ? -1 : rest.indexOf(">>", start);

    if (start === -1 || end === -1) {
      segments.push({ text: rest, matched: false });
      break;
    }

    if (start > 0) {
      segments.push({ text: rest.slice(0, start), matched: false });
    }
    segments.push({ text: rest.slice(start + 2, end), matched: true });
    rest = rest.slice(end + 2);
  }

  return segments.filter((segment) => segment.text.length > 0);
}

export type RegisteredPasskey = {
  id: string;
  nickname: string;
  createdAt: string;
  lastUsedAt: string | null;
};

export function parsePasskeys(value: unknown): RegisteredPasskey[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const passkeys: RegisteredPasskey[] = [];
  for (const entry of value) {
    const raw = record(entry);
    if (!raw || typeof raw.id !== "string" || typeof raw.nickname !== "string") {
      return undefined;
    }

    passkeys.push({
      id: raw.id,
      nickname: raw.nickname,
      createdAt: text(raw.created_at) ?? "",
      lastUsedAt: text(raw.last_used_at),
    });
  }

  return passkeys;
}

/** A WebAuthn challenge, passed to the browser untouched. */
export type Challenge = { ceremonyId: string; options: unknown };

export function parseChallenge(value: unknown): Challenge | undefined {
  const raw = record(value);
  if (!raw || typeof raw.ceremony_id !== "string" || raw.options === undefined) {
    return undefined;
  }

  return { ceremonyId: raw.ceremony_id, options: raw.options };
}

/** A deterministic collection shown on the TV and the home screen. */
export type MemoryGroup = {
  /** Stable across requests, so hiding one keeps it hidden. */
  key: string;
  title: string;
  subtitle: string;
  items: Item[];
};

export function parseMemories(value: unknown): MemoryGroup[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const groups: MemoryGroup[] = [];
  for (const entry of value) {
    const raw = record(entry);
    const items = raw ? parseItems(raw.items) : undefined;
    if (!raw || !items || typeof raw.title !== "string") {
      return undefined;
    }

    groups.push({
      key: typeof raw.key === "string" ? raw.key : "",
      title: raw.title,
      subtitle: typeof raw.subtitle === "string" ? raw.subtitle : "",
      items,
    });
  }

  return groups;
}

/** A television paired with a library. */
export type TvDevice = {
  id: string;
  name: string;
  createdAt: string;
  lastSeenAt: string | null;
};

export function parseTvDevice(value: unknown): TvDevice | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string" || typeof raw.name !== "string") {
    return undefined;
  }

  return {
    id: raw.id,
    name: raw.name,
    createdAt: text(raw.created_at) ?? "",
    lastSeenAt: text(raw.last_seen_at),
  };
}

export function parseTvDevices(value: unknown): TvDevice[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const devices: TvDevice[] = [];
  for (const entry of value) {
    const device = parseTvDevice(entry);
    if (!device) {
      return undefined;
    }
    devices.push(device);
  }

  return devices;
}

/** What a television is shown while it waits to be approved. */
export type Pairing = { code: string; pollToken: string; expiresAt: string };

export function parsePairing(value: unknown): Pairing | undefined {
  const raw = record(value);
  if (!raw || typeof raw.code !== "string" || typeof raw.poll_token !== "string") {
    return undefined;
  }

  return {
    code: raw.code,
    pollToken: raw.poll_token,
    expiresAt: text(raw.expires_at) ?? "",
  };
}

/** The answer to "has anyone approved this screen yet?" */
export type PairingStatus = {
  status: "pending" | "approved";
  token: string | null;
  libraryName: string | null;
};

export function parsePairingStatus(value: unknown): PairingStatus | undefined {
  const raw = record(value);
  if (!raw || (raw.status !== "pending" && raw.status !== "approved")) {
    return undefined;
  }

  return {
    status: raw.status,
    token: text(raw.token),
    libraryName: text(raw.library_name),
  };
}

/** A curated set of pictures inside a library. */
export type Album = {
  id: string;
  name: string;
  createdAt: string;
  itemCount: number;
  coverItemId: string | null;
};

export function parseAlbum(value: unknown): Album | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string" || typeof raw.name !== "string") {
    return undefined;
  }

  return {
    id: raw.id,
    name: raw.name,
    createdAt: text(raw.created_at) ?? "",
    itemCount: typeof raw.item_count === "number" ? raw.item_count : 0,
    coverItemId: text(raw.cover_item_id),
  };
}

export function parseAlbums(value: unknown): Album[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const albums: Album[] = [];
  for (const entry of value) {
    const album = parseAlbum(entry);
    if (!album) {
      return undefined;
    }
    albums.push(album);
  }

  return albums;
}

export type AlbumContents = { album: Album; items: Item[] };

export function parseAlbumContents(value: unknown): AlbumContents | undefined {
  const raw = record(value);
  if (!raw) {
    return undefined;
  }

  const album = parseAlbum(raw.album);
  const items = parseItems(raw.items);

  return album && items ? { album, items } : undefined;
}

/** A link that lets someone send files into one folder. */
export type UploadRequest = {
  id: string;
  itemId: string;
  folderName: string;
  title: string;
  createdAt: string;
  expiresAt: string | null;
  maxFiles: number;
  maxBytes: number;
  receivedFiles: number;
  receivedBytes: number;
  /** Present only on the response that created the link. */
  token: string | null;
};

export function parseUploadRequest(value: unknown): UploadRequest | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string" || typeof raw.title !== "string") {
    return undefined;
  }

  const count = (field: unknown) => (typeof field === "number" ? field : 0);

  return {
    id: raw.id,
    itemId: text(raw.item_id) ?? "",
    folderName: text(raw.folder_name) ?? "",
    title: raw.title,
    createdAt: text(raw.created_at) ?? "",
    expiresAt: text(raw.expires_at),
    maxFiles: count(raw.max_files),
    maxBytes: count(raw.max_bytes),
    receivedFiles: count(raw.received_files),
    receivedBytes: count(raw.received_bytes),
    token: text(raw.token),
  };
}

export function parseUploadRequests(value: unknown): UploadRequest[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const links: UploadRequest[] = [];
  for (const entry of value) {
    const link = parseUploadRequest(entry);
    if (!link) {
      return undefined;
    }
    links.push(link);
  }

  return links;
}

/** What someone holding an upload link is told. Never the contents. */
export type PublicUploadRequest = {
  title: string;
  folderName: string;
  filesLeft: number;
  bytesLeft: number;
};

export function parsePublicUploadRequest(value: unknown): PublicUploadRequest | undefined {
  const raw = record(value);
  if (!raw || typeof raw.title !== "string") {
    return undefined;
  }

  return {
    title: raw.title,
    folderName: text(raw.folder_name) ?? "",
    filesLeft: typeof raw.files_left === "number" ? raw.files_left : 0,
    bytesLeft: typeof raw.bytes_left === "number" ? raw.bytes_left : 0,
  };
}

/** What a file used to be. */
export type FileVersion = {
  id: string;
  sizeBytes: number;
  contentType: string | null;
  contentModifiedAt: string | null;
  replacedAt: string;
};

export function parseFileVersion(value: unknown): FileVersion | undefined {
  const raw = record(value);
  if (!raw || typeof raw.id !== "string") {
    return undefined;
  }

  return {
    id: raw.id,
    sizeBytes: typeof raw.size_bytes === "number" ? raw.size_bytes : 0,
    contentType: text(raw.content_type),
    contentModifiedAt: text(raw.content_modified_at),
    replacedAt: text(raw.replaced_at) ?? "",
  };
}

export function parseFileVersions(value: unknown): FileVersion[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const versions: FileVersion[] = [];
  for (const entry of value) {
    const version = parseFileVersion(entry);
    if (!version) {
      return undefined;
    }
    versions.push(version);
  }

  return versions;
}

/** A set of files that are byte-for-byte the same. */
export type DuplicateGroup = {
  sizeBytes: number;
  /** What deleting every copy but one would free. */
  reclaimableBytes: number;
  items: Item[];
};

export function parseDuplicateGroups(value: unknown): DuplicateGroup[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const groups: DuplicateGroup[] = [];
  for (const entry of value) {
    const raw = record(entry);
    const items = raw ? parseItems(raw.items) : undefined;
    if (!raw || !items) {
      return undefined;
    }

    groups.push({
      sizeBytes: typeof raw.size_bytes === "number" ? raw.size_bytes : 0,
      reclaimableBytes: typeof raw.reclaimable_bytes === "number" ? raw.reclaimable_bytes : 0,
      items,
    });
  }

  return groups;
}

/** The private AI switch, and what the machine can actually do. */
export type AiSettings = {
  profile: "off" | "text" | "photos" | "people";
  /** Whether this server can read text out of pictures at all. */
  ocrAvailable: boolean;
  /** The most this deployment can honour, whatever was asked for. */
  supportedProfile: "off" | "text" | "photos" | "people";
  /** Pictures waiting to be read, so enabling is not a silent commitment. */
  pendingItems: number;
};

function parseProfile(value: unknown): AiSettings["profile"] {
  return value === "text" || value === "photos" || value === "people" ? value : "off";
}

export function parseAiSettings(value: unknown): AiSettings | undefined {
  const raw = record(value);
  if (!raw) {
    return undefined;
  }

  return {
    profile: parseProfile(raw.profile),
    ocrAvailable: raw.ocr_available === true,
    supportedProfile: parseProfile(raw.supported_profile),
    pendingItems: typeof raw.pending_items === "number" ? raw.pending_items : 0,
  };
}
