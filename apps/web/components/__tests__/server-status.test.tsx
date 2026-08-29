import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ServerStatus } from "@/components/server-status";

function jsonResponse(body: unknown, init: ResponseInit & { requestId?: string } = {}) {
  const headers = new Headers(init.headers);
  if (init.requestId) {
    headers.set("x-request-id", init.requestId);
  }

  return new Response(JSON.stringify(body), { ...init, headers });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ServerStatus", () => {
  it("announces the pending check before the answer arrives", async () => {
    let settle: ((response: Response) => void) | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockReturnValue(
        new Promise<Response>((resolve) => {
          settle = resolve;
        }),
      ),
    );

    render(<ServerStatus />);

    const pending = screen.getByRole("status");
    expect(pending).toHaveTextContent("Checking the server");
    expect(pending).toHaveAttribute("aria-live", "polite");

    settle?.(jsonResponse({ needs_owner: true }));
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
  });

  it("reports a deployment that still needs an owner", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse({ needs_owner: true })));

    render(<ServerStatus />);

    expect(await screen.findByRole("heading", { level: 2 })).toHaveTextContent(
      "This deployment is not set up yet",
    );
  });

  it("shows an actionable error with the request id when the server is unavailable", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        jsonResponse(
          {
            code: "dependency_unavailable",
            detail: "The database is not available. Retry shortly.",
            request_id: "corr-1234",
          },
          { status: 503 },
        ),
      )
      .mockResolvedValueOnce(jsonResponse({ needs_owner: false }));
    vi.stubGlobal("fetch", fetchMock);

    render(<ServerStatus />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The database is not available");
    expect(alert).toHaveTextContent("corr-1234");

    // The recovery action actually recovers.
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("heading", { level: 2 })).toHaveTextContent(
      "Your library is empty",
    );
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("does not offer a retry for an error that retrying cannot fix", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse({ code: "not_found", detail: "The requested resource does not exist." }, {
          status: 404,
        }),
      ),
    );

    render(<ServerStatus />);

    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
  });
});
