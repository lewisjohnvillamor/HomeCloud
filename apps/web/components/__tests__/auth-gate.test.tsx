import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthGate } from "@/components/session/auth-gate";
import { SessionProvider } from "@/components/session/session-provider";

function json(body: unknown, init: ResponseInit = {}) {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: { "content-type": "application/json" },
  });
}

/** Routes a mocked `fetch` by path, as the real server would. */
function server(routes: Record<string, () => Response>) {
  return vi.fn(async (input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input.toString();
    const path = url.split("?")[0] ?? url;
    const handler = routes[path];

    return handler ? handler() : json({ code: "not_found", detail: "no route" }, { status: 404 });
  });
}

function renderGate() {
  render(
    <SessionProvider>
      <AuthGate>
        <p>Private content</p>
      </AuthGate>
    </SessionProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AuthGate", () => {
  it("offers first-run setup when the deployment has no owner", async () => {
    vi.stubGlobal(
      "fetch",
      server({
        "/api/v1/session": () => json({ authenticated: false }),
        "/api/v1/bootstrap": () => json({ needs_owner: true }),
      }),
    );

    renderGate();

    expect(await screen.findByRole("heading", { name: "Set up HomeCloud" })).toBeInTheDocument();
    expect(screen.queryByText("Private content")).not.toBeInTheDocument();
  });

  it("asks for sign-in when an owner already exists", async () => {
    vi.stubGlobal(
      "fetch",
      server({
        "/api/v1/session": () => json({ authenticated: false }),
        "/api/v1/bootstrap": () => json({ needs_owner: false }),
      }),
    );

    renderGate();

    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  });

  it("renders the page once a session exists", async () => {
    vi.stubGlobal(
      "fetch",
      server({
        "/api/v1/session": () => json({ authenticated: true, display_name: "Ada" }),
        "/api/v1/libraries": () => json([{ id: "lib-1", name: "Home", role: "owner" }]),
      }),
    );

    renderGate();

    expect(await screen.findByText("Private content")).toBeInTheDocument();
  });

  it("shows an actionable error when the server cannot be reached", async () => {
    const fetchMock = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValue(json({ authenticated: false }));
    vi.stubGlobal("fetch", fetchMock);

    renderGate();

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The server is not responding");

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("signs the owner in through the setup form", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();

      if (url.includes("/api/v1/setup")) {
        expect(init?.method).toBe("POST");
        return json({ authenticated: true, display_name: "Ada" });
      }
      if (url.includes("/api/v1/session")) {
        return json({ authenticated: fetchMock.mock.calls.length > 2, display_name: "Ada" });
      }
      if (url.includes("/api/v1/bootstrap")) {
        return json({ needs_owner: true });
      }

      return json([{ id: "lib-1", name: "Home", role: "owner" }]);
    });
    vi.stubGlobal("fetch", fetchMock);

    renderGate();
    await screen.findByRole("heading", { name: "Set up HomeCloud" });

    await userEvent.type(screen.getByLabelText("Your name"), "Ada");
    await userEvent.type(screen.getByLabelText("Password"), "correct horse battery staple");
    await userEvent.click(screen.getByRole("button", { name: "Create owner account" }));

    expect(await screen.findByText("Private content")).toBeInTheDocument();
  });
});

describe("session expiry", () => {
  it("returns to sign-in when the server says the session is gone", async () => {
    let authenticated = true;
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();

      if (url.includes("/api/v1/session")) {
        return json({ authenticated, display_name: "Ada" });
      }
      if (url.includes("/api/v1/bootstrap")) {
        return json({ needs_owner: false });
      }
      if (url.includes("/api/v1/libraries")) {
        // The session expires between the first load and this call.
        authenticated = false;
        return json({ code: "unauthenticated", detail: "Sign in to continue." }, { status: 401 });
      }

      return json({});
    });
    vi.stubGlobal("fetch", fetchMock);

    renderGate();

    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  });
});
