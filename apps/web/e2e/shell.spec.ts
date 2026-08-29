import { expect, test } from "@playwright/test";

const DESTINATIONS = ["Home", "Files", "Photos", "Search", "More"];

test.describe("application shell", () => {
  test("is fully operable with the keyboard", async ({ page }) => {
    await page.goto("/");

    // First stop is the skip link, then each destination in order.
    await page.keyboard.press("Tab");
    await expect(page.getByRole("link", { name: "Skip to content" })).toBeFocused();

    for (const destination of DESTINATIONS) {
      await page.keyboard.press("Tab");
      await expect(page.getByRole("link", { name: destination, exact: true })).toBeFocused();
    }

    // The focused link navigates and the destination becomes current.
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/\/more$/);
    await expect(page.getByRole("link", { name: "More", exact: true })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  test("shows a visible focus ring on the focused destination", async ({ page }) => {
    await page.goto("/");

    const files = page.getByRole("link", { name: "Files", exact: true });
    await files.focus();

    const outlineWidth = await files.evaluate(
      (element) => getComputedStyle(element).outlineWidth,
    );

    expect(Number.parseFloat(outlineWidth)).toBeGreaterThan(0);
  });

  test("never covers page content with the navigation", async ({ page }) => {
    await page.goto("/");

    const nav = page.getByRole("navigation", { name: "Primary" });
    // The content wrapper, not `main`: `main` deliberately reserves
    // padding underneath the mobile bar, so its box extends behind it.
    const content = page.locator("main > div");

    await page.mouse.wheel(0, 2000);

    const navBox = await nav.boundingBox();
    const contentBox = await content.boundingBox();
    expect(navBox).not.toBeNull();
    expect(contentBox).not.toBeNull();

    const clearVertically = contentBox!.y + contentBox!.height <= navBox!.y + 1;
    const clearHorizontally = contentBox!.x >= navBox!.x + navBox!.width - 1;

    expect(clearVertically || clearHorizontally).toBe(true);
  });

  test("renders exactly one primary navigation, not a duplicate per breakpoint", async ({
    page,
  }) => {
    await page.goto("/");

    await expect(page.getByRole("navigation", { name: "Primary" })).toHaveCount(1);
    await expect(page.getByRole("link", { name: "Files", exact: true })).toHaveCount(1);
  });

  test("every section renders a heading and a stated empty state", async ({ page }) => {
    for (const [path, heading] of [
      ["/", "HomeCloud"],
      ["/files", "Files"],
      ["/photos", "Photos"],
      ["/search", "Search"],
      ["/more", "More"],
    ] as const) {
      await page.goto(path);

      await expect(page.getByRole("heading", { level: 1 })).toHaveText(heading);
      await expect(page.getByRole("heading", { level: 2 })).toBeVisible();
    }
  });
});
