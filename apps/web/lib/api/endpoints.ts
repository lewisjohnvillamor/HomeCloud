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
  postFile,
  postJson,
  type ApiResult,
  type RequestOptions,
} from "./client";
import {
  parseBrowse,
  parseItem,
  parseItems,
  parseLibraries,
  parsePublicShare,
  parseScanStatus,
  parseSearchResults,
  parseInvitation,
  parseInvitationPreview,
  parseInvitations,
  parseMembers,
  parseSession,
  parseShare,
  parseShares,
  type Browse,
  type Item,
  type Library,
  type PublicShare,
  type Invitation,
  type InvitationPreview,
  type Member,
  type ScanStatus,
  type SearchResult,
  type Session,
  type Share,
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
  options?: RequestOptions,
): Promise<ApiResult<Share>> {
  return postJson(
    `/api/v1/items/${encodeURIComponent(item)}/shares`,
    { expires_in_days: expiresInDays },
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

/** What a visitor with a link can see. Takes no session. */
export function fetchPublicShare(
  token: string,
  item?: string,
  options?: RequestOptions,
): Promise<ApiResult<PublicShare>> {
  const suffix = item ? `?item=${encodeURIComponent(item)}` : "";

  return getJson(`/api/v1/public/${encodeURIComponent(token)}${suffix}`, parsePublicShare, options);
}

export function publicContentUrl(token: string, item?: string): string {
  const suffix = item ? `?item=${encodeURIComponent(item)}` : "";

  return `/api/v1/public/${encodeURIComponent(token)}/content${suffix}`;
}

export function publicThumbnailUrl(token: string, item?: string): string {
  const suffix = item ? `?item=${encodeURIComponent(item)}` : "";

  return `/api/v1/public/${encodeURIComponent(token)}/thumbnail${suffix}`;
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

export { contentUrl, thumbnailUrl };
