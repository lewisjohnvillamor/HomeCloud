import { describe, expect, it, vi } from "vitest";
import { fetchBootstrapStatus } from "@/lib/api/bootstrap";
import { isRetryable, toProblem } from "@/lib/api/problem";

function jsonResponse(body: unknown, init: ResponseInit & { requestId?: string } = {}) {
  const headers = new Headers(init.headers);
  if (init.requestId) {
    headers.set("x-request-id", init.requestId);
  }

  return new Response(JSON.stringify(body), { ...init, headers });
}

describe("fetchBootstrapStatus", () => {
  it("returns typed data for a well-formed response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ needs_owner: true }));

    const result = await fetchBootstrapStatus({ fetchImpl });

    expect(result).toEqual({ ok: true, data: { needsOwner: true } });
    expect(fetchImpl).toHaveBeenCalledWith(
      "/api/v1/bootstrap",
      expect.objectContaining({ method: "GET", credentials: "same-origin" }),
    );
  });

  it("surfaces a structured problem, including the request id", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse(
        {
          code: "dependency_unavailable",
          detail: "The database is not available. Retry shortly.",
          request_id: "corr-1234",
        },
        { status: 503, requestId: "corr-1234" },
      ),
    );

    const result = await fetchBootstrapStatus({ fetchImpl });

    expect(result).toEqual({
      ok: false,
      problem: {
        code: "dependency_unavailable",
        detail: "The database is not available. Retry shortly.",
        requestId: "corr-1234",
        status: 503,
      },
    });
  });

  it("reports an unreachable server without leaking the transport error", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new TypeError("Failed to fetch: ECONNREFUSED"));

    const result = await fetchBootstrapStatus({ fetchImpl });

    expect(result.ok).toBe(false);
    if (result.ok) {
      return;
    }
    expect(result.problem.code).toBe("network");
    expect(result.problem.detail).not.toContain("ECONNREFUSED");
  });

  it("rejects a success payload that does not match the contract", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ needs_owner: "yes" }));

    const result = await fetchBootstrapStatus({ fetchImpl });

    expect(result.ok).toBe(false);
    if (result.ok) {
      return;
    }
    expect(result.problem.code).toBe("malformed_response");
  });

  it("rejects an error payload with an unknown code", async () => {
    const problem = toProblem(400, { code: "kaboom", detail: "..." });

    expect(problem.code).toBe("malformed_response");
  });

  it("treats a gateway page at a 5xx status as a retryable server failure", async () => {
    const problem = toProblem(502, "<html>Bad Gateway</html>");

    expect(problem.code).toBe("internal");
    expect(isRetryable(problem)).toBe(true);
  });
});

describe("isRetryable", () => {
  it("offers a retry only where one could succeed", () => {
    expect(isRetryable({ code: "network", detail: "" })).toBe(true);
    expect(isRetryable({ code: "dependency_unavailable", detail: "" })).toBe(true);
    expect(isRetryable({ code: "not_found", detail: "" })).toBe(false);
  });
});
