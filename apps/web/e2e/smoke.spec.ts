import { expect, test } from "@playwright/test";

test("home page renders the application shell", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
});
