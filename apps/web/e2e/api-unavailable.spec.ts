import { expect, test } from "@playwright/test";

/**
 * The end-to-end environment runs the web app without an API server,
 * which is exactly the state a user hits when the backend is down. The
 * app must say so and offer a way forward.
 */
test("home offers a recovery action when the API is unreachable", async ({ page }) => {
  await page.goto("/");

  // Scoped to `main`: the framework's route announcer is also an alert.
  const alert = page.getByRole("main").getByRole("alert");
  await expect(alert).toBeVisible();
  await expect(alert).toContainText("The server is not responding");

  const retry = page.getByRole("button", { name: "Try again" });
  await expect(retry).toBeVisible();

  // Retrying re-runs the check rather than leaving a dead screen.
  await retry.click();
  await expect(page.getByRole("main").getByRole("alert")).toBeVisible();
});

test("a failed status check never shows raw transport detail", async ({ page }) => {
  await page.goto("/");

  const body = await page.getByRole("main").textContent();

  expect(body).not.toContain("ECONNREFUSED");
  expect(body).not.toContain("fetch failed");
});
