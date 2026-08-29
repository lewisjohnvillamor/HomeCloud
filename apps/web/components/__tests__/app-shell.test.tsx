import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppShell } from "@/components/app-shell";

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
}));

describe("AppShell", () => {
  it("exposes a main landmark that the skip link targets", () => {
    render(
      <AppShell>
        <h1>Page</h1>
      </AppShell>,
    );

    const main = screen.getByRole("main");
    const skipLink = screen.getByRole("link", { name: "Skip to content" });

    expect(main).toHaveAttribute("id", "main-content");
    expect(skipLink).toHaveAttribute("href", "#main-content");
  });

  it("puts the skip link first in the tab order", async () => {
    const user = userEvent.setup();
    render(
      <AppShell>
        <h1>Page</h1>
      </AppShell>,
    );

    await user.tab();

    expect(screen.getByRole("link", { name: "Skip to content" })).toHaveFocus();
  });
});
