import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PrimaryNav } from "@/components/navigation/primary-nav";
import { isCurrent, NAVIGATION_ITEMS } from "@/components/navigation/navigation-items";

const pathname = vi.hoisted(() => ({ current: "/" }));

vi.mock("next/navigation", () => ({
  usePathname: () => pathname.current,
}));

describe("PrimaryNav", () => {
  beforeEach(() => {
    pathname.current = "/";
  });

  it("renders one labelled navigation landmark with every destination", () => {
    render(<PrimaryNav />);

    const nav = screen.getByRole("navigation", { name: "Primary" });
    const links = screen.getAllByRole("link");

    expect(nav).toBeInTheDocument();
    expect(links).toHaveLength(NAVIGATION_ITEMS.length);
    expect(links.map((link) => link.textContent)).toEqual(
      NAVIGATION_ITEMS.map((item) => item.label),
    );
  });

  it("marks the active destination for assistive technology", () => {
    pathname.current = "/photos";

    render(<PrimaryNav />);

    expect(screen.getByRole("link", { name: "Photos" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Home" })).not.toHaveAttribute("aria-current");
  });

  it("reaches every destination with the keyboard alone, in visual order", async () => {
    const user = userEvent.setup();
    render(<PrimaryNav />);

    for (const item of NAVIGATION_ITEMS) {
      await user.tab();

      expect(screen.getByRole("link", { name: item.label })).toHaveFocus();
    }
  });
});

describe("isCurrent", () => {
  it("treats a nested route as being inside its section", () => {
    expect(isCurrent("/files/holiday", "/files")).toBe(true);
    expect(isCurrent("/files/holiday", "/")).toBe(false);
  });

  it("does not match a sibling with a shared prefix", () => {
    expect(isCurrent("/files-archive", "/files")).toBe(false);
  });

  it("matches home only exactly", () => {
    expect(isCurrent("/", "/")).toBe(true);
    expect(isCurrent("/photos", "/")).toBe(false);
  });
});
