import { toProblem, type ApiProblem } from "./problem";

/**
 * Result of an API call. Errors are values, not exceptions, so a caller
 * cannot forget to handle one.
 */
export type ApiResult<T> = { ok: true; data: T } | { ok: false; problem: ApiProblem };

/** Where the API lives. Same-origin by default; `/api` is proxied in development. */
const API_BASE_URL = process.env.NEXT_PUBLIC_API_BASE_URL ?? "";

const REQUEST_ID_HEADER = "x-request-id";

export type RequestOptions = {
  signal?: AbortSignal;
  /** Injected in tests; defaults to the platform `fetch`. */
  fetchImpl?: typeof fetch;
};

/**
 * Performs a JSON GET and narrows the response with `parse`.
 *
 * `parse` is supplied by the caller so an unexpected payload is caught at
 * the boundary instead of flowing into the UI as `undefined`.
 */
export async function getJson<T>(
  path: string,
  parse: (value: unknown) => T | undefined,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  const doFetch = options.fetchImpl ?? fetch;

  let response: Response;
  try {
    response = await doFetch(`${API_BASE_URL}${path}`, {
      method: "GET",
      headers: { accept: "application/json" },
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
    return { ok: false, problem: toProblem(response.status, body, requestId) };
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
