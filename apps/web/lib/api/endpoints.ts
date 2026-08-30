/**
 * Every server call the app makes, in one place, so the set of things
 * the UI can ask for is reviewable at a glance.
 */

import {
  asUnknown,
  contentUrl,
  thumbnailUrl,
  deleteJson,
  getJson,
  patchFile,
  patchJson,
  postFile,
  postJson,
  putFile,
  putJson,
  type ApiResult,
  type RequestOptions,
} from "./client";
import {
  parseBrowse,
  parseAlbum,
  parseItem,
  parseItems,
  parseLibraries,
  parsePublicShare,
  parseChallenge,
  parseMemories,
  parsePasskeys,
  parseScanStatus,
  parseSearchResults,
  parseInvitation,
  parseInvitationPreview,
  parseInvitations,
  parseMembers,
  parseAiSettings,
  parseAlbumContents,
  parseAlbums,
  parseDuplicateGroups,
  parseFileVersions,
  parsePairing,
  parsePublicUploadRequest,
  parsePairingStatus,
  parseSession,
  parseShare,
  parseShares,
  parseTvDevice,
  parseTvDevices,
  parseUploadRequest,
  parseUploadRequests,
  type Browse,
  type Item,
  type Library,
  type PublicShare,
  type Invitation,
  type InvitationPreview,
  type Member,
  type Challenge,
  type MemoryGroup,
  type RegisteredPasskey,
  type ScanStatus,
  type SearchResult,
  type AiSettings,
  type Album,
  type AlbumContents,
  type DuplicateGroup,
  type FileVersion,
  type Pairing,
  type PairingStatus,
  type Session,
  type Share,
  type PublicUploadRequest,
  type TvDevice,
  type UploadRequest,
} from "./types";

export type BootstrapStatus = { needsOwner: boolean };

function parseBootstrap(value: unknown): BootstrapStatus | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const raw = value as Record<string, unknown>;

  return typeof raw.needs_owner === "boolean" ? { needsOwner: raw.needs_owner } : undefined;
}

export function fetchBootstrapStatus(
  options?: RequestOptions,
): Promise<ApiResult<BootstrapStatus>> {
  return getJson("/api/v1/bootstrap", parseBootstrap, options);
}

export function fetchSession(options?: RequestOptions): Promise<ApiResult<Session>> {
  return getJson("/api/v1/session", parseSession, options);
}

export function createOwner(
  input: { displayName: string; password: string; libraryName: string },
  options?: RequestOptions,
): Promise<ApiResult<Session>> {
  return postJson(
    "/api/v1/setup",
    {
      display_name: input.displayName,
      password: input.password,
      library_name: input.libraryName,
    },
    parseSession,
    options,
  );
}

export function signIn(
  input: { displayName: string; password: string },
  options?: RequestOptions,
): Promise<ApiResult<Session>> {
  return postJson(
    "/api/v1/auth/login",
    { display_name: input.displayName, password: input.password },
    parseSession,
    options,
  );
}

export function signOut(options?: RequestOptions): Promise<ApiResult<Session>> {
  return postJson("/api/v1/auth/logout", {}, parseSession, options);
}

export function fetchLibraries(options?: RequestOptions): Promise<ApiResult<Library[]>> {
  return getJson("/api/v1/libraries", parseLibraries, options);
}

export function browse(
  library: string,
  path: string,
  options?: RequestOptions,
): Promise<ApiResult<Browse>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/browse?path=${encodeURIComponent(path)}`,
    parseBrowse,
    options,
  );
}

export function fetchPhotos(library: string, options?: RequestOptions): Promise<ApiResult<Item[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/photos`,
    parseItems,
    options,
  );
}

export function fetchMemories(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<MemoryGroup[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/memories`,
    parseMemories,
    options,
  );
}

export function searchLibrary(
  library: string,
  query: string,
  options?: RequestOptions,
): Promise<ApiResult<SearchResult[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/search?q=${encodeURIComponent(query)}`,
    parseSearchResults,
    options,
  );
}

export function fetchTrash(library: string, options?: RequestOptions): Promise<ApiResult<Item[]>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/trash`, parseItems, options);
}

export function startScan(library: string, options?: RequestOptions): Promise<ApiResult<ScanStatus>> {
  return postJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/scan`,
    {},
    parseScanStatus,
    options,
  );
}

export function fetchScanStatus(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<ScanStatus>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/scan`, parseScanStatus, options);
}

export function createFolder(
  library: string,
  path: string,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/folders`,
    { path },
    parseItem,
    options,
  );
}

export function uploadFile(
  library: string,
  path: string,
  file: File,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postFile(
    `/api/v1/libraries/${encodeURIComponent(library)}/upload?path=${encodeURIComponent(path)}`,
    file,
    parseItem,
    options,
  );
}

export function moveItem(
  item: string,
  path: string,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postJson(`/api/v1/items/${encodeURIComponent(item)}/move`, { path }, parseItem, options);
}

export function trashItem(item: string, options?: RequestOptions): Promise<ApiResult<Item>> {
  return deleteJson(`/api/v1/items/${encodeURIComponent(item)}`, parseItem, options);
}

export function restoreItem(item: string, options?: RequestOptions): Promise<ApiResult<Item>> {
  return postJson(`/api/v1/items/${encodeURIComponent(item)}/restore`, {}, parseItem, options);
}

// --- Sharing ---

export function createShare(
  item: string,
  expiresInDays: number | null,
  password: string | null,
  options?: RequestOptions,
): Promise<ApiResult<Share>> {
  return postJson(
    `/api/v1/items/${encodeURIComponent(item)}/shares`,
    { expires_in_days: expiresInDays, password },
    parseShare,
    options,
  );
}

export function fetchSharesForItem(
  item: string,
  options?: RequestOptions,
): Promise<ApiResult<Share[]>> {
  return getJson(`/api/v1/items/${encodeURIComponent(item)}/shares`, parseShares, options);
}

export function fetchSharesForLibrary(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Share[]>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/shares`, parseShares, options);
}

export function revokeShare(share: string, options?: RequestOptions): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/shares/${encodeURIComponent(share)}`, asUnknown, options);
}

/**
 * Query string for a public request. The unlock key travels as a query
 * parameter rather than a header because `<img>` and download links
 * cannot send headers; the server logs paths only.
 */
export function publicQuery(item?: string, key?: string | null): string {
  const parts: string[] = [];
  if (item) {
    parts.push(`item=${encodeURIComponent(item)}`);
  }
  if (key) {
    parts.push(`key=${encodeURIComponent(key)}`);
  }

  return parts.length > 0 ? `?${parts.join("&")}` : "";
}

/** What a visitor with a link can see. Takes no session. */
export function fetchPublicShare(
  token: string,
  item?: string,
  key?: string | null,
  options?: RequestOptions,
): Promise<ApiResult<PublicShare>> {
  return getJson(
    `/api/v1/public/${encodeURIComponent(token)}${publicQuery(item, key)}`,
    parsePublicShare,
    options,
  );
}

/** Proves the password on a protected link, returning a key good for an hour. */
export function unlockShare(
  token: string,
  password: string,
  options?: RequestOptions,
): Promise<ApiResult<{ key: string }>> {
  return postJson(
    `/api/v1/public/${encodeURIComponent(token)}/unlock`,
    { password },
    parseUnlockKey,
    options,
  );
}

export function publicContentUrl(token: string, item?: string, key?: string | null): string {
  return `/api/v1/public/${encodeURIComponent(token)}/content${publicQuery(item, key)}`;
}

export function publicThumbnailUrl(token: string, item?: string, key?: string | null): string {
  return `/api/v1/public/${encodeURIComponent(token)}/thumbnail${publicQuery(item, key)}`;
}

// --- People ---

export function fetchMembers(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Member[]>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/members`, parseMembers, options);
}

export function removeMember(
  library: string,
  member: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/members/${encodeURIComponent(member)}`,
    asUnknown,
    options,
  );
}

export function createInvitation(
  library: string,
  expiresInDays: number,
  options?: RequestOptions,
): Promise<ApiResult<Invitation>> {
  return postJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/invitations`,
    { expires_in_days: expiresInDays },
    parseInvitation,
    options,
  );
}

export function fetchInvitations(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Invitation[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/invitations`,
    parseInvitations,
    options,
  );
}

export function revokeInvitation(
  invitation: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/invitations/${encodeURIComponent(invitation)}`, asUnknown, options);
}

/** Takes no session: the person accepting usually has no account yet. */
export function previewInvitation(
  token: string,
  options?: RequestOptions,
): Promise<ApiResult<InvitationPreview>> {
  return getJson(
    `/api/v1/invitations/${encodeURIComponent(token)}/preview`,
    parseInvitationPreview,
    options,
  );
}

export function acceptInvitation(
  token: string,
  account: { displayName: string; password: string } | null,
  options?: RequestOptions,
): Promise<ApiResult<Session>> {
  return postJson(
    `/api/v1/invitations/${encodeURIComponent(token)}/accept`,
    account ? { display_name: account.displayName, password: account.password } : {},
    parseSession,
    options,
  );
}

// --- Passkeys ---

export function fetchPasskeys(options?: RequestOptions): Promise<ApiResult<RegisteredPasskey[]>> {
  return getJson("/api/v1/auth/passkeys", parsePasskeys, options);
}

export function startPasskeyRegistration(
  options?: RequestOptions,
): Promise<ApiResult<Challenge>> {
  return postJson("/api/v1/auth/passkeys/register/options", {}, parseChallenge, options);
}

export function finishPasskeyRegistration(
  ceremonyId: string,
  nickname: string,
  credential: unknown,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return postJson(
    "/api/v1/auth/passkeys/register/verify",
    { ceremony_id: ceremonyId, nickname, credential },
    asUnknown,
    options,
  );
}

export function startPasskeySignIn(
  displayName: string,
  options?: RequestOptions,
): Promise<ApiResult<Challenge>> {
  return postJson(
    "/api/v1/auth/passkeys/login/options",
    { display_name: displayName },
    parseChallenge,
    options,
  );
}

export function finishPasskeySignIn(
  ceremonyId: string,
  credential: unknown,
  options?: RequestOptions,
): Promise<ApiResult<Session>> {
  return postJson(
    "/api/v1/auth/passkeys/login/verify",
    { ceremony_id: ceremonyId, credential },
    parseSession,
    options,
  );
}

export function removePasskey(
  passkey: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/auth/passkeys/${encodeURIComponent(passkey)}`, asUnknown, options);
}

/** The private AI switch. Readable by members, changed by the owner. */
export function fetchAiSettings(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<AiSettings>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/ai`, parseAiSettings, options);
}

export function updateAiSettings(
  library: string,
  profile: AiSettings["profile"],
  options?: RequestOptions,
): Promise<ApiResult<AiSettings>> {
  return putJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/ai`,
    { profile },
    parseAiSettings,
    options,
  );
}

/** Photos that recorded where they were taken. Members only. */
export function fetchPlaces(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Item[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/places`,
    parseItems,
    options,
  );
}

/** Exact duplicates: same bytes, same hash. */
export function fetchDuplicates(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<DuplicateGroup[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/duplicates`,
    parseDuplicateGroups,
    options,
  );
}

/** Copies a file, leaving the original where it is. */
export function copyItem(
  item: string,
  path: string,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postJson(
    `/api/v1/items/${encodeURIComponent(item)}/copy`,
    { path },
    parseItem,
    options,
  );
}

// --- What a file used to be ---

export function fetchVersions(
  item: string,
  options?: RequestOptions,
): Promise<ApiResult<FileVersion[]>> {
  return getJson(
    `/api/v1/items/${encodeURIComponent(item)}/versions`,
    parseFileVersions,
    options,
  );
}

/** Replaces a file's contents, keeping the old ones as a version. */
export function replaceContent(
  item: string,
  file: File,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return putFile(`/api/v1/items/${encodeURIComponent(item)}/content`, file, parseItem, options);
}

export function versionContentUrl(item: string, version: string): string {
  return `/api/v1/items/${encodeURIComponent(item)}/versions/${encodeURIComponent(version)}/content`;
}

export function restoreVersion(
  item: string,
  version: string,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postJson(
    `/api/v1/items/${encodeURIComponent(item)}/versions/${encodeURIComponent(version)}/restore`,
    {},
    parseItem,
    options,
  );
}

// --- Upload request links ---

/** The mirror image of a share: write into one folder, read nothing. */
export function createUploadRequest(
  item: string,
  input: { title?: string; expiresInDays: number | null },
  options?: RequestOptions,
): Promise<ApiResult<UploadRequest>> {
  return postJson(
    `/api/v1/items/${encodeURIComponent(item)}/upload-requests`,
    { title: input.title, expires_in_days: input.expiresInDays },
    parseUploadRequest,
    options,
  );
}

export function fetchUploadRequests(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<UploadRequest[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/upload-requests`,
    parseUploadRequests,
    options,
  );
}

export function revokeUploadRequest(
  id: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/upload-requests/${encodeURIComponent(id)}`, asUnknown, options);
}

/** What the link is for. Takes no session. */
export function fetchPublicUploadRequest(
  token: string,
  options?: RequestOptions,
): Promise<ApiResult<PublicUploadRequest>> {
  return getJson(
    `/api/v1/public/upload-requests/${encodeURIComponent(token)}`,
    parsePublicUploadRequest,
    options,
  );
}

/** Sends one file through a link. Takes no session. */
export function sendToUploadRequest(
  token: string,
  name: string,
  file: File,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postFile(
    `/api/v1/public/upload-requests/${encodeURIComponent(token)}/files?name=${encodeURIComponent(name)}`,
    file,
    parseItem,
    options,
  );
}

// --- Resumable uploads ---

export type UploadSession = {
  id: string;
  path: string;
  /** Where to continue from, as the server counted it. */
  offset: number;
  sizeBytes: number;
  maxChunkBytes: number;
  expiresAt: string;
};

function parseUploadSession(value: unknown): UploadSession | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const raw = value as Record<string, unknown>;
  if (typeof raw.id !== "string" || typeof raw.offset !== "number") {
    return undefined;
  }

  return {
    id: raw.id,
    path: typeof raw.path === "string" ? raw.path : "",
    offset: raw.offset,
    sizeBytes: typeof raw.size_bytes === "number" ? raw.size_bytes : 0,
    maxChunkBytes:
      typeof raw.max_chunk_bytes === "number" ? raw.max_chunk_bytes : 8 * 1024 * 1024,
    expiresAt: typeof raw.expires_at === "string" ? raw.expires_at : "",
  };
}

function parseUploadSessions(value: unknown): UploadSession[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const sessions: UploadSession[] = [];
  for (const entry of value) {
    const session = parseUploadSession(entry);
    if (!session) {
      return undefined;
    }
    sessions.push(session);
  }

  return sessions;
}

export function createUploadSession(
  input: { library: string; path: string; sizeBytes: number },
  options?: RequestOptions,
): Promise<ApiResult<UploadSession>> {
  return postJson(
    "/api/v1/uploads",
    { library_id: input.library, path: input.path, size_bytes: input.sizeBytes },
    parseUploadSession,
    options,
  );
}

/** How much the server actually has, which is the only offset that counts. */
export function fetchUploadStatus(
  session: string,
  options?: RequestOptions,
): Promise<ApiResult<UploadSession>> {
  return getJson(`/api/v1/uploads/${encodeURIComponent(session)}`, parseUploadSession, options);
}

export function appendUploadChunk(
  session: string,
  offset: number,
  chunk: Blob,
  options?: RequestOptions,
): Promise<ApiResult<UploadSession>> {
  return patchFile(
    `/api/v1/uploads/${encodeURIComponent(session)}?offset=${offset}`,
    chunk,
    parseUploadSession,
    options,
  );
}

export function completeUpload(
  session: string,
  options?: RequestOptions,
): Promise<ApiResult<Item>> {
  return postJson(
    `/api/v1/uploads/${encodeURIComponent(session)}/complete`,
    {},
    parseItem,
    options,
  );
}

export function abortUpload(session: string, options?: RequestOptions): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/uploads/${encodeURIComponent(session)}`, asUnknown, options);
}

/** Unfinished uploads, so one can be picked up after a reload. */
export function fetchUploadSessions(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<UploadSession[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/uploads`,
    parseUploadSessions,
    options,
  );
}

// --- Favorites and albums ---

/** Starring is idempotent, so a client that retries is safe. */
export function addFavorite(item: string, options?: RequestOptions): Promise<ApiResult<unknown>> {
  return putJson(`/api/v1/items/${encodeURIComponent(item)}/favorite`, {}, asUnknown, options);
}

export function removeFavorite(
  item: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/items/${encodeURIComponent(item)}/favorite`, asUnknown, options);
}

/** One person's own favorites. Not the library's. */
export function fetchFavorites(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Item[]>> {
  return getJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/favorites`,
    parseItems,
    options,
  );
}

export function fetchAlbums(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<Album[]>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/albums`, parseAlbums, options);
}

export function createAlbum(
  library: string,
  name: string,
  options?: RequestOptions,
): Promise<ApiResult<Album>> {
  return postJson(
    `/api/v1/libraries/${encodeURIComponent(library)}/albums`,
    { name },
    parseAlbum,
    options,
  );
}

export function fetchAlbum(
  album: string,
  options?: RequestOptions,
): Promise<ApiResult<AlbumContents>> {
  return getJson(`/api/v1/albums/${encodeURIComponent(album)}`, parseAlbumContents, options);
}

export function renameAlbum(
  album: string,
  name: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return patchJson(`/api/v1/albums/${encodeURIComponent(album)}`, { name }, asUnknown, options);
}

export function deleteAlbum(album: string, options?: RequestOptions): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/albums/${encodeURIComponent(album)}`, asUnknown, options);
}

export function addToAlbum(
  album: string,
  items: string[],
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return postJson(
    `/api/v1/albums/${encodeURIComponent(album)}/items`,
    { items },
    asUnknown,
    options,
  );
}

export function removeFromAlbum(
  album: string,
  item: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(
    `/api/v1/albums/${encodeURIComponent(album)}/items/${encodeURIComponent(item)}`,
    asUnknown,
    options,
  );
}

// --- Account recovery ---

export type RecoveryStatus = { hasCode: boolean; createdAt: string | null };

function parseRecoveryStatus(value: unknown): RecoveryStatus | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const raw = value as Record<string, unknown>;
  if (typeof raw.has_code !== "boolean") {
    return undefined;
  }

  return {
    hasCode: raw.has_code,
    createdAt: typeof raw.created_at === "string" ? raw.created_at : null,
  };
}

function parseRecoveryCode(value: unknown): { code: string } | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const raw = value as Record<string, unknown>;

  return typeof raw.code === "string" ? { code: raw.code } : undefined;
}

function parseUnlockKey(value: unknown): { key: string } | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const raw = value as Record<string, unknown>;

  return typeof raw.key === "string" ? { key: raw.key } : undefined;
}

export function fetchRecoveryStatus(
  options?: RequestOptions,
): Promise<ApiResult<RecoveryStatus>> {
  return getJson("/api/v1/auth/recovery", parseRecoveryStatus, options);
}

/** Replaces any existing code. The new one is shown once. */
export function regenerateRecoveryCode(
  options?: RequestOptions,
): Promise<ApiResult<{ code: string }>> {
  return postJson("/api/v1/auth/recovery", {}, parseRecoveryCode, options);
}

/**
 * Sets a new password from a recovery code. Takes no session, ends every
 * existing one, and hands back a fresh code in the same response.
 */
export function recoverAccount(
  input: { displayName: string; recoveryCode: string; newPassword: string },
  options?: RequestOptions,
): Promise<ApiResult<Session>> {
  return postJson(
    "/api/v1/auth/recover",
    {
      display_name: input.displayName,
      recovery_code: input.recoveryCode,
      new_password: input.newPassword,
    },
    parseSession,
    options,
  );
}

// --- Television ---

/** A screen with no keyboard asks to be paired. Takes no session. */
export function startPairing(options?: RequestOptions): Promise<ApiResult<Pairing>> {
  return postJson("/api/v1/tv/pairings", {}, parsePairing, options);
}

/** Has anyone approved this screen yet? Polled by the television. */
export function pollPairing(
  pollToken: string,
  options?: RequestOptions,
): Promise<ApiResult<PairingStatus>> {
  return getJson(
    `/api/v1/tv/pairings/${encodeURIComponent(pollToken)}`,
    parsePairingStatus,
    options,
  );
}

/** The human step: someone signed in vouches for the code on the screen. */
export function approvePairing(
  code: string,
  input: { library: string; name: string },
  options?: RequestOptions,
): Promise<ApiResult<TvDevice>> {
  return postJson(
    `/api/v1/tv/pairings/${encodeURIComponent(code)}/approve`,
    { library_id: input.library, name: input.name },
    parseTvDevice,
    options,
  );
}

export function fetchTvDevices(
  library: string,
  options?: RequestOptions,
): Promise<ApiResult<TvDevice[]>> {
  return getJson(`/api/v1/libraries/${encodeURIComponent(library)}/tv`, parseTvDevices, options);
}

export function unpairTvDevice(
  device: string,
  options?: RequestOptions,
): Promise<ApiResult<unknown>> {
  return deleteJson(`/api/v1/tv/devices/${encodeURIComponent(device)}`, asUnknown, options);
}

/** The wall a paired screen shows, through its own credential. */
export function fetchTvMemories(
  token: string,
  options?: RequestOptions,
): Promise<ApiResult<MemoryGroup[]>> {
  return getJson(`/api/v1/tv/memories?token=${encodeURIComponent(token)}`, parseMemories, options);
}

/**
 * As with share links, the credential travels as a query parameter: an
 * `<img>` on the photo wall cannot send a header.
 */
export function tvThumbnailUrl(token: string, item: string): string {
  return `/api/v1/tv/thumbnail?token=${encodeURIComponent(token)}&item=${encodeURIComponent(item)}`;
}

export function tvContentUrl(token: string, item: string): string {
  return `/api/v1/tv/content?token=${encodeURIComponent(token)}&item=${encodeURIComponent(item)}`;
}

export { contentUrl, thumbnailUrl };
