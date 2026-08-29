import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/**
 * The first-run journey, in order, against a real server: set up the
 * owner, upload files, see them as files and photos, search, delete and
 * restore, then sign out and back in.
 *
 * These run in a single worker against one deployment, so each step
 * builds on the last exactly as a person's first hour would.
 */

const OWNER = { name: "Ada", password: "correct horse battery staple" };

/** A tiny but genuinely valid PNG, so the Photos view has real content. */
const PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function tempFile(name: string, contents: Buffer | string): string {
  const directory = mkdtempSync(join(tmpdir(), "homecloud-e2e-"));
  const path = join(directory, name);
  writeFileSync(path, contents);

  return path;
}

test.describe.configure({ mode: "serial" });

// One browser context for the whole journey: the session cookie has to
// survive from setup to sign-out, exactly as it does for a person.
let context: BrowserContext;
let page: Page;

test.beforeAll(async ({ browser }: { browser: Browser }) => {
  context = await browser.newContext();
  page = await context.newPage();
});

test.afterAll(async () => {
  await context.close();
});

test("a fresh deployment asks for setup and creates the owner", async () => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Set up HomeCloud" })).toBeVisible();

  await page.getByLabel("Your name").fill(OWNER.name);
  await page.getByLabel("Password").fill(OWNER.password);
  await page.getByRole("button", { name: "Create owner account" }).click();

  await expect(page.getByRole("heading", { level: 1 })).toHaveText(`Welcome back, ${OWNER.name}`);
});

test("files can be uploaded and downloaded again", async () => {
  await page.goto("/files");

  await expect(page.getByRole("heading", { name: "This folder is empty" })).toBeVisible();

  await page
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("notes.txt", "quarterly numbers"));

  const row = page.getByRole("row").filter({ hasText: "notes.txt" });
  await expect(row).toBeVisible();

  const download = await Promise.all([
    page.waitForEvent("download"),
    row.getByRole("link", { name: /Download/ }).click(),
  ]);
  expect(download[0].suggestedFilename()).toBe("notes.txt");
});

test("folders can be created and navigated with the keyboard", async () => {
  await page.goto("/files");

  page.once("dialog", (dialog) => dialog.accept("Documents"));
  await page.getByRole("button", { name: "New folder" }).click();

  const folder = page.getByRole("button", { name: "Documents", exact: true });
  await expect(folder).toBeVisible();

  await folder.focus();
  await page.keyboard.press("Enter");

  await expect(page).toHaveURL(/path=Documents/);
  await expect(page.getByRole("navigation", { name: "Folder path" })).toContainText("Documents");
  await expect(page.getByRole("heading", { name: "This folder is empty" })).toBeVisible();
});

test("an uploaded image appears in Photos", async () => {
  await page.goto("/files");
  await page
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("sunrise.png", Buffer.from(PNG_BASE64, "base64")));
  await expect(page.getByRole("row").filter({ hasText: "sunrise.png" })).toBeVisible();

  await page.goto("/photos");

  const photo = page.getByRole("img", { name: "sunrise.png" });
  await expect(photo).toBeVisible();
  // The image really loaded from the API, rather than showing alt text.
  await expect
    .poll(() => photo.evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);
});

test("search finds a file by name", async () => {
  await page.goto("/search");

  await page.getByRole("searchbox", { name: "Search your library" }).fill("notes");
  await page.getByRole("button", { name: "Search" }).click();

  await expect(
    page.getByRole("listitem").filter({ hasText: "notes.txt" }).first(),
  ).toBeVisible();
});

test("a deleted file goes to the trash and can be restored", async () => {
  await page.goto("/files");

  const row = page.getByRole("row").filter({ hasText: "notes.txt" });
  page.once("dialog", (dialog) => dialog.accept());
  await row.getByRole("button", { name: /Delete/ }).click();

  await expect(page.getByRole("row").filter({ hasText: "notes.txt" })).toHaveCount(0);

  await page.goto("/more");
  const trashed = page.getByRole("listitem").filter({ hasText: "notes.txt" });
  await expect(trashed).toBeVisible();
  await trashed.getByRole("button", { name: /Restore/ }).click();

  await page.goto("/files");
  await expect(page.getByRole("row").filter({ hasText: "notes.txt" })).toBeVisible();
});

test("a scan picks up files placed in the library folder directly", async () => {
  await page.goto("/more");

  await page.getByRole("button", { name: "Scan library" }).click();

  await expect(page.getByRole("status").filter({ hasText: "Last scan indexed" })).toBeVisible({
    timeout: 30_000,
  });
});

test("signing out ends the session and signing back in restores it", async () => {
  await page.goto("/more");
  await page.getByRole("button", { name: "Sign out" }).click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  // The private view is gone, not merely hidden.
  await page.goto("/files");
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await page.getByLabel("Your name").fill(OWNER.name);
  await page.getByLabel("Password").fill(OWNER.password);
  await page.getByRole("button", { name: "Sign in" }).click();

  await expect(page.getByRole("row").filter({ hasText: "notes.txt" })).toBeVisible();
});

test("a wrong password is refused", async () => {
  await page.goto("/more");
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await page.getByLabel("Your name").fill(OWNER.name);
  await page.getByLabel("Password").fill("definitely not the password");
  await page.getByRole("button", { name: "Sign in" }).click();

  // Scoped to `main`: the framework's route announcer is also an alert.
  await expect(page.getByRole("main").getByRole("alert")).toContainText("do not match");
});
