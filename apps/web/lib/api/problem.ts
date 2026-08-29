/**
 * The API's error contract, mirrored on the client.
 *
 * The frontend never surfaces a raw exception, response body, or status
 * line: everything becomes one of these, so every call site has the same
 * shape to render.
 */
export const ERROR_CODES = [
  "bad_request",
  "not_found",
  "payload_too_large",
  "dependency_unavailable",
  "internal",
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

/** Codes the client adds for failures that never reached the server. */
export type ClientErrorCode = "network" | "malformed_response";

export type ApiProblem = {
  code: ErrorCode | ClientErrorCode;
  /** Safe, human-readable sentence suitable for display. */
  detail: string;
  /** Correlation id, when the server assigned one. */
  requestId?: string;
  /** HTTP status, absent when the request never completed. */
  status?: number;
};

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && (ERROR_CODES as readonly string[]).includes(value);
}

/**
 * Converts a server response body into a problem. A body that does not
 * match the contract is reported as `malformed_response` rather than
 * being partially trusted.
 */
export function toProblem(status: number, body: unknown, requestId?: string): ApiProblem {
  if (typeof body === "object" && body !== null) {
    const candidate = body as Record<string, unknown>;

    if (isErrorCode(candidate.code) && typeof candidate.detail === "string") {
      return {
        code: candidate.code,
        detail: candidate.detail,
        requestId: typeof candidate.request_id === "string" ? candidate.request_id : requestId,
        status,
      };
    }
  }

  // A body that does not match the contract at a 5xx status is typically
  // a reverse proxy or gateway answering instead of the API. That is a
  // server-side failure a retry can recover from, not a client bug.
  if (status >= 500) {
    return {
      code: "internal",
      detail: "The server could not complete the request.",
      requestId,
      status,
    };
  }

  return {
    code: "malformed_response",
    detail: "The server sent a response this version of the app does not understand.",
    requestId,
    status,
  };
}

/** True when retrying the same request could plausibly succeed. */
export function isRetryable(problem: ApiProblem): boolean {
  return (
    problem.code === "network" ||
    problem.code === "dependency_unavailable" ||
    problem.code === "internal"
  );
}
