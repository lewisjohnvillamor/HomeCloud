import { toProblem, type ApiProblem } from "./problem";

/**
 * Result of an API call. Errors are values, not exceptions, so a caller
 * cannot forget to handle one.
 */
export type ApiResult<T> = { ok: true; data: T } | { ok: false; problem: ApiProblem };

/** Where the API lives. Same-origin by default; `/api` is proxied in development. */
const API_BASE_URL = process.env.NEXT_PUBLIC_API_BASE_URL ?? "";

const REQUEST_ID_HEADER = "x-request-id";

/**
 * Endpoints where a 401 is an answer rather than an expired session:
 * signing in with the wrong password, or asking who is signed in.
 */
const AUTHENTICATION_PATHS = ["/api/v1/session", "/api/v1/auth/login", "/api/v1/setup"];

type SessionEndedListener = () => void;

let sessionEndedListener: SessionEndedListener | null = null;

/**
 * Registers what to do when the server says the session is gone.
 *
 * A session can expire or be revoked while a page is open; without this,
 * the next action would show a confusing error instead of the sign-in
 * screen. The session provider owns the handler.
 */
export function onSessionEnded(listener: SessionEndedListener | null): void {
  sessionEndedListener = listener;
}

export type RequestOptions = {
  signal?: AbortSignal;
  /** Injected in tests; defaults to the platform `fetch`. */
  fetchImpl?: typeof fetch;
};

/** Parses a value that the caller expects to be present but not typed. */
export type Parser<T> = (value: unknown) => T | undefined;

/** Accepts any JSON body; use only where the server owns the shape. */
export const asUnknown: Parser<unknown> = (value) => value ?? null;

async function request<T>(
  method: string,
  path: string,
  parse: Parser<T>,
  init: { body?: BodyInit; contentType?: string } = {},
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  const doFetch = options.fetchImpl ?? fetch;

  const headers: Record<string, string> = { accept: "application/json" };
  if (init.contentType) {
    headers["content-type"] = init.contentType;
  }

  let response: Response;
  try {
    response = await doFetch(`${API_BASE_URL}${path}`, {
      method,
      headers,
      body: init.body,
      // The session cookie is `HttpOnly`; the browser attaches it, and
      // page scripts never see it.
      credentials: "same-origin",
      signal: options.signal,
    });
  } catch {
    // Includes offline, DNS failure, TLS failure, and a server that is
    // not listening yet. The underlying message is not shown: it is
    // browser-specific and not actionable.
    return {
      ok: false,
      problem: {
        code: "network",
        detail: "The HomeCloud server could not be reached.",
      },
    };
  }

  const requestId = response.headers.get(REQUEST_ID_HEADER) ?? undefined;

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    body = undefined;
  }

  if (!response.ok) {
    const problem = toProblem(response.status, body, requestId);

    if (
      problem.code === "unauthenticated" &&
      !AUTHENTICATION_PATHS.some((candidate) => path.startsWith(candidate))
    ) {
      sessionEndedListener?.();
    }

    return { ok: false, problem };
  }

  const parsed = parse(body);
  if (parsed === undefined) {
    return {
      ok: false,
      problem: {
        code: "malformed_response",
        detail: "The server sent a response this version of the app does not understand.",
        requestId,
        status: response.status,
      },
    };
  }

  return { ok: true, data: parsed };
}

export function getJson<T>(
  path: string,
  parse: Parser<T>,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  return request("GET", path, parse, {}, options);
}

export function postJson<T>(
  path: string,
  payload: unknown,
  parse: Parser<T>,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  return request(
    "POST",
    path,
    parse,
    { body: JSON.stringify(payload ?? {}), contentType: "application/json" },
    options,
  );
}

export function deleteJson<T>(
  path: string,
  parse: Parser<T>,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  return request("DELETE", path, parse, {}, options);
}

/** Sends a file body. The browser streams it; nothing is buffered here. */
export function postFile<T>(
  path: string,
  file: Blob,
  parse: Parser<T>,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  return request(
    "POST",
    path,
    parse,
    { body: file, contentType: file.type || "application/octet-stream" },
    options,
  );
}

/** URL a browser can point an `<img>`, `<video>`, or download link at. */
export function contentUrl(itemId: string): string {
  return `${API_BASE_URL}/api/v1/items/${encodeURIComponent(itemId)}/content`;
}
