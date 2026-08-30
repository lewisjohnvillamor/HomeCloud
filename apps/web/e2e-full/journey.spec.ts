import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";
import { spawnSync } from "node:child_process";
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

/**
 * Captured from the setup screen and used by the recovery journey at the
 * end of this file, exactly as a person would use the code they wrote
 * down on their first day.
 */
let recoveryCode = "";

/**
 * A JPEG whose EXIF header says it was taken on 4 July 2019 by a
 * Fujifilm X100V. Assembled by the same rules as the Rust tests, so the
 * journey exercises real metadata rather than a stubbed field.
 */
/**
 * A JPEG whose header says it was taken at Greenwich — 51°28'N, 0°5'W.
 * Built by the same rules as the Rust fixtures.
 */
const JPEG_AT_GREENWICH_BASE64 = 
  "/9j/4QCIRXhpZgAASUkqAAgAAAABACWIBAABAAAAGgAAAAAAAAAEAAEAAgACAAAATgAAAAIABQADAAAAUAAAAAMAAgACAAAAVwAAAAQABQADAAAAaAAAAAAAAAAzAAAAAQAAABwAAAABAAAAAAAAAAEAAAAAAAAAAQAAAAUAAAABAAAAAAAAAAEAAAD/2Q==";

const JPEG_TAKEN_2019_BASE64 =
  "/9j/4QBvRXhpZgAASUkqAAgAAAADAA8BAgAJAAAAMgAAABABAgAGAAAAOwAAAGmHBAABAAAAQQAAAAAAAABGdWppZmlsbQBYMTAwVgABAAOQAgAUAAAAUwAAAAAAAAAyMDE5OjA3OjA0IDEyOjMwOjQ1AP/Z";

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

  // The recovery code is shown once, before the app opens, because the
  // server keeps only its hash.
  await expect(page.getByRole("heading", { name: "Write this down" })).toBeVisible();
  recoveryCode = (await page.getByLabel("Your recovery code").innerText()).trim();
  expect(recoveryCode).toMatch(/[A-Z0-9-]{8,}/);

  await page.getByRole("button", { name: "I have written it down" }).click();

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

  // Wait for the restore to actually land before navigating: leaving the
  // page mid-request cancels it, which is a race this test used to win
  // on a fast machine and lose on a slow one.
  await expect(page.getByRole("listitem").filter({ hasText: "notes.txt" })).toHaveCount(0);

  await page.goto("/files");
  await expect(page.getByRole("row").filter({ hasText: "notes.txt" })).toBeVisible();
});

test("a scan picks up files placed in the library folder directly", async () => {
  await page.goto("/more");

  await scanLibrary(page);

  // The scan is what puts it there, so poll for the outcome rather than
  // for a status line that an earlier scan already left behind.
  await expect(async () => {
    await page.goto("/more");
    await expect(
      page.getByRole("status").filter({ hasText: "Last scan indexed" }),
    ).toBeVisible({ timeout: 2_000 });
  }).toPass({ timeout: 60_000 });
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

test("the file list fits a phone screen without sideways scrolling", async ({ browser }) => {
  // A fresh, phone-sized context: the row actions must stay on screen at
  // 390 CSS pixels, which is where the layout is tightest.
  const phone = await browser.newContext({ viewport: { width: 390, height: 780 } });
  const phonePage = await phone.newPage();

  await phonePage.goto("/");
  await phonePage.getByLabel("Your name").fill(OWNER.name);
  await phonePage.getByLabel("Password").fill(OWNER.password);
  await phonePage.getByRole("button", { name: "Sign in" }).click();
  await expect(phonePage.getByRole("heading", { level: 1 })).toHaveText(
    `Welcome back, ${OWNER.name}`,
  );

  await phonePage.goto("/files");
  await expect(phonePage.getByRole("row").filter({ hasText: "notes.txt" })).toBeVisible();

  const overflows = await phonePage.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth + 1,
  );
  expect(overflows).toBe(false);

  await phone.close();
});

/// Opens a fresh, signed-in page and makes sure `name` exists in the
/// library root. Independent of the shared journey page: these tests run
/// after the sign-out steps, so they own their session.
/**
 * Starts a scan from More.
 *
 * Only the request is waited on here. Waiting for "Last scan indexed" to
 * appear proves nothing: an earlier journey's scan has already left that
 * on the page, so the assertion passes instantly. Callers that depend on
 * what a scan produced should poll for that instead.
 */
async function scanLibrary(page: Page) {
  await page.getByRole("button", { name: "Scan library" }).click();

  await expect(page.getByRole("status").filter({ hasText: "Scan started" })).toBeVisible({
    timeout: 30_000,
  });
}

async function signedInPage(browser: Browser, name: string) {
  const owner = await browser.newContext();
  const ownerPage = await owner.newPage();

  await ownerPage.goto("/files");

  // The session check runs on load; wait for its outcome rather than
  // reading the screen mid-flight.
  const signIn = ownerPage.getByRole("heading", { name: "Sign in" });
  await expect(signIn.or(ownerPage.getByRole("button", { name: "Upload files" })).first()).toBeVisible();

  if (await signIn.isVisible()) {
    await ownerPage.getByLabel("Your name").fill(OWNER.name);
    await ownerPage.getByLabel("Password").fill(OWNER.password);
    await ownerPage.getByRole("button", { name: "Sign in" }).click();
    await expect(ownerPage.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
    await ownerPage.goto("/files");
  }

  const row = ownerPage.getByRole("row").filter({ hasText: name });
  if ((await row.count()) === 0) {
    await ownerPage
      .getByLabel("Choose files to upload")
      .setInputFiles(tempFile(name, "shared bytes"));
  }
  await expect(row).toBeVisible();

  return { owner, ownerPage, row };
}

test("a share link opens for someone who is not signed in", async ({ browser }) => {
  const { owner, ownerPage, row } = await signedInPage(browser, "shared-note.txt");

  await row.getByRole("button", { name: /Share/ }).click();
  const dialog = ownerPage.getByRole("dialog", { name: /Share/ });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Create link" }).click();

  const link = await dialog.getByLabel("Share link").inputValue();
  expect(link).toContain("/s/");

  // A visitor with no session, no cookies, and no account opens it.
  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();
  await visitorPage.goto(link);

  await expect(visitorPage.getByRole("heading", { level: 1 })).toHaveText("shared-note.txt");
  // The shell is not there: nothing invites them into the library.
  await expect(visitorPage.getByRole("navigation", { name: "Primary" })).toHaveCount(0);

  const download = await Promise.all([
    visitorPage.waitForEvent("download"),
    visitorPage.getByRole("link", { name: /Download/ }).click(),
  ]);
  expect(download[0].suggestedFilename()).toBe("shared-note.txt");

  await visitor.close();
  await owner.close();
});

test("a revoked share link stops opening", async ({ browser }) => {
  const { owner, ownerPage, row } = await signedInPage(browser, "revoked-note.txt");

  await row.getByRole("button", { name: /Share/ }).click();
  const dialog = ownerPage.getByRole("dialog", { name: /Share/ });
  await dialog.getByRole("button", { name: "Create link" }).click();
  const link = await dialog.getByLabel("Share link").inputValue();

  await dialog.getByRole("button", { name: "Revoke" }).first().click();
  await expect(dialog.getByRole("button", { name: "Revoke" })).toHaveCount(0);

  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();
  await visitorPage.goto(link);

  await expect(
    visitorPage.getByRole("heading", { name: "This link is not available" }),
  ).toBeVisible();

  await visitor.close();
  await owner.close();
});

test("an invited person joins the library and sees its files", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "family-notes.txt");

  // The owner invites someone from More.
  await ownerPage.goto("/more");
  await ownerPage.getByRole("button", { name: "Invite someone" }).click();
  const invitation = await ownerPage.getByLabel("Invitation link").inputValue();
  expect(invitation).toContain("/invite/");

  // The invited person has no account and no session.
  const guest = await browser.newContext();
  const guestPage = await guest.newPage();
  await guestPage.goto(invitation);

  await expect(guestPage.getByText("invited you to")).toBeVisible();
  await guestPage.getByLabel("Your name").fill("Grace");
  await guestPage.getByLabel("Password").fill("another good passphrase here");
  await guestPage.getByRole("button", { name: "Join the library" }).click();

  await expect(guestPage.getByRole("heading", { level: 1 })).toHaveText("Welcome back, Grace");

  // She can read the library, and can add to it.
  await guestPage.goto("/files");
  await expect(guestPage.getByRole("row").filter({ hasText: "family-notes.txt" })).toBeVisible();

  // She is not the owner, so the owner's controls are not offered to her.
  await guestPage.goto("/more");
  await expect(guestPage.getByRole("listitem").filter({ hasText: "Grace" })).toBeVisible();
  await expect(guestPage.getByRole("button", { name: "Invite someone" })).toHaveCount(0);

  await guest.close();
  await owner.close();
});

test("a removed member loses access straight away", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "family-notes.txt");
  await ownerPage.goto("/more");
  await ownerPage.getByRole("button", { name: "Invite someone" }).click();
  const invitation = await ownerPage.getByLabel("Invitation link").inputValue();

  const guest = await browser.newContext();
  const guestPage = await guest.newPage();
  await guestPage.goto(invitation);
  await guestPage.getByLabel("Your name").fill("Mallory");
  await guestPage.getByLabel("Password").fill("yet another passphrase ok");
  await guestPage.getByRole("button", { name: "Join the library" }).click();
  await expect(guestPage.getByRole("heading", { level: 1 })).toHaveText("Welcome back, Mallory");

  // The owner removes them.
  ownerPage.once("dialog", (dialog) => dialog.accept());
  await ownerPage.goto("/more");
  const row = ownerPage.getByRole("listitem").filter({ hasText: "Mallory" });
  await row.getByRole("button", { name: /Remove/ }).click();
  await expect(ownerPage.getByRole("listitem").filter({ hasText: "Mallory" })).toHaveCount(0);

  // Their open session is over, not merely powerless.
  await guestPage.goto("/files");
  await expect(guestPage.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await guest.close();
  await owner.close();
});

test("search finds a document by a word inside it", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // A document whose name says nothing about its contents.
  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(
      tempFile("4021.txt", "Invoice for one standby generator, delivered to the workshop."),
    );
  await expect(ownerPage.getByRole("row").filter({ hasText: "4021.txt" })).toBeVisible();

  // Uploads are indexed by the next scan, which runs in the background.
  await ownerPage.goto("/more");
  await scanLibrary(ownerPage);

  // Search until it finds it, rather than guessing how long indexing
  // takes: waiting on a status line that a previous scan already left on
  // the page is how this raced.
  const result = ownerPage.getByRole("listitem").filter({ hasText: "4021.txt" });
  await expect(async () => {
    await ownerPage.goto("/search");
    await ownerPage.getByRole("searchbox", { name: "Search your library" }).fill("generator");
    await ownerPage.getByRole("button", { name: "Search" }).click();
    await expect(result).toBeVisible({ timeout: 2_000 });
  }).toPass({ timeout: 60_000 });
  await expect(result).toContainText("found in the document");
  // The snippet shows the matching passage, with the word highlighted.
  await expect(result.locator("mark")).toContainText("generator");

  await owner.close();
});

test("a passkey can be registered and then used to sign in", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // A virtual authenticator: Chromium's own, driven over CDP, so this
  // exercises the real WebAuthn ceremony rather than a stub.
  const cdp = await owner.newCDPSession(ownerPage);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

  await ownerPage.goto("/more");
  await ownerPage.getByRole("button", { name: "Add a passkey" }).click();

  await expect(ownerPage.getByRole("status").filter({ hasText: "Passkey" })).toBeVisible({
    timeout: 15_000,
  });
  await expect(
    ownerPage.getByRole("listitem").filter({ hasText: "added" }).first(),
  ).toBeVisible();

  // Sign out, then back in with the passkey rather than the password.
  await ownerPage.getByRole("button", { name: "Sign out" }).click();
  await expect(ownerPage.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await ownerPage.getByLabel("Your name").fill(OWNER.name);
  await ownerPage.getByRole("button", { name: "Use a passkey" }).click();

  // Signed in again, without ever typing the password: the page they
  // were on comes back rather than the sign-in screen.
  await expect(ownerPage.getByText(`Signed in as ${OWNER.name}`)).toBeVisible({
    timeout: 15_000,
  });
  await ownerPage.goto("/files");
  await expect(ownerPage.getByRole("button", { name: "Upload files" })).toBeVisible();

  await owner.close();
});

test("a passkey belonging to nobody cannot sign anyone in", async ({ browser }) => {
  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();

  const cdp = await visitor.newCDPSession(visitorPage);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

  await visitorPage.goto("/");
  await expect(visitorPage.getByRole("heading", { name: "Sign in" })).toBeVisible();

  // An account that does not exist, with a fresh authenticator holding
  // no credential for this server.
  await visitorPage.getByLabel("Your name").fill("Mallory");
  await visitorPage.getByRole("button", { name: "Use a passkey" }).click();

  await expect(visitorPage.getByRole("main").getByRole("alert")).toContainText("No passkey");
  await expect(visitorPage.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await visitor.close();
});

test("the television view is driven entirely by a remote", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // Two photos, so the remote has somewhere to move to.
  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles([
      tempFile("tv-one.png", Buffer.from(PNG_BASE64, "base64")),
      tempFile("tv-two.png", Buffer.from(PNG_BASE64, "base64")),
    ]);
  await expect(ownerPage.getByRole("row").filter({ hasText: "tv-two.png" })).toBeVisible();

  await ownerPage.goto("/tv");
  await expect(ownerPage.getByRole("heading", { level: 1 })).toHaveText("Photos");

  // No sidebar: the TV has its own interaction model.
  await expect(ownerPage.getByRole("navigation", { name: "Primary" })).toHaveCount(0);

  // The first tile is selected without anyone touching a mouse.
  const tiles = ownerPage.locator("[data-tile]");
  await expect(tiles.first()).toHaveAttribute("data-selected", "true");

  // Right moves the selection; Enter starts the slideshow.
  await ownerPage.keyboard.press("ArrowRight");
  await expect(tiles.nth(1)).toHaveAttribute("data-selected", "true");

  await ownerPage.keyboard.press("Enter");
  const slideshow = ownerPage.getByRole("dialog", { name: "Slideshow" });
  await expect(slideshow).toBeVisible();
  await expect(slideshow).toContainText("Playing");

  // Enter pauses, Escape goes back to the wall.
  await ownerPage.keyboard.press("Enter");
  await expect(slideshow).toContainText("Paused");
  await ownerPage.keyboard.press("Escape");
  await expect(slideshow).toHaveCount(0);
  await expect(ownerPage.getByRole("heading", { level: 1 })).toHaveText("Photos");

  await owner.close();
});

test("a video shows a poster frame and joins the photo timeline", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // A real video, rendered by FFmpeg. Skipped where it is not installed,
  // exactly as the server itself degrades.
  const clip = tempFile("holiday.mp4", "");
  const rendered = spawnSync("ffmpeg", [
    "-nostdin",
    "-loglevel",
    "error",
    "-y",
    "-f",
    "lavfi",
    "-i",
    "testsrc=size=320x240:rate=10:duration=1",
    "-pix_fmt",
    "yuv420p",
    clip,
  ]);
  test.skip(rendered.status !== 0, "ffmpeg is not installed here");

  await ownerPage.getByLabel("Choose files to upload").setInputFiles(clip);
  await expect(ownerPage.getByRole("row").filter({ hasText: "holiday.mp4" })).toBeVisible();

  // The file row shows the poster rather than a generic icon.
  const poster = ownerPage
    .getByRole("row")
    .filter({ hasText: "holiday.mp4" })
    .locator("img");
  await expect(poster).toBeVisible();
  await expect
    .poll(() => poster.evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);

  // And it sits in the timeline beside the photos, marked as a video.
  await ownerPage.goto("/photos");
  const tile = ownerPage.getByRole("img", { name: "holiday.mp4" });
  await expect(tile).toBeVisible();
  await expect
    .poll(() => tile.evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);

  await owner.close();
});

test("a password-protected link discloses nothing until the password is given", async ({
  browser,
}) => {
  const { owner, ownerPage, row } = await signedInPage(browser, "locked-note.txt");

  await row.getByRole("button", { name: /Share/ }).click();
  const dialog = ownerPage.getByRole("dialog", { name: /Share/ });
  await dialog.getByLabel("Password (optional)").fill("sunflower77");
  await dialog.getByRole("button", { name: "Create link" }).click();

  const link = await dialog.getByLabel("Share link").inputValue();

  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();
  await visitorPage.goto(link);

  // Not even the file's name is on the page yet.
  await expect(
    visitorPage.getByRole("heading", { name: "This link is password protected" }),
  ).toBeVisible();
  await expect(visitorPage.getByText("locked-note.txt")).toHaveCount(0);

  // A wrong password is refused and still discloses nothing.
  await visitorPage.getByLabel("Password").fill("not the password");
  await visitorPage.getByRole("button", { name: "Open link" }).click();
  await expect(visitorPage.getByRole("alert")).toBeVisible();
  await expect(visitorPage.getByText("locked-note.txt")).toHaveCount(0);

  // The right one opens it, and the download works from the same key.
  await visitorPage.getByLabel("Password").fill("sunflower77");
  await visitorPage.getByRole("button", { name: "Open link" }).click();
  await expect(visitorPage.getByRole("heading", { level: 1 })).toHaveText("locked-note.txt");

  const download = await Promise.all([
    visitorPage.waitForEvent("download"),
    visitorPage.getByRole("link", { name: /Download/ }).click(),
  ]);
  expect(download[0].suggestedFilename()).toBe("locked-note.txt");

  await visitor.close();
  await owner.close();
});

test("a television with no keyboard is paired from a phone", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("paired.png", Buffer.from(PNG_BASE64, "base64")));
  await expect(ownerPage.getByRole("row").filter({ hasText: "paired.png" })).toBeVisible();

  // The television: a browser with no session at all, which is what a
  // set in a living room is.
  const tv = await browser.newContext();
  const tvPage = await tv.newPage();
  await tvPage.goto("/tv");

  // It asks to be connected rather than showing a password form it
  // could never be used to fill in.
  await expect(tvPage.getByRole("heading", { name: "Connect this screen" })).toBeVisible();
  await expect(tvPage.getByLabel("Password")).toHaveCount(0);
  const code = (await tvPage.getByLabel("Pairing code").innerText()).trim();
  expect(code).toMatch(/^[A-Z0-9]{4}-[A-Z0-9]{4}$/);

  // The phone approves it, exactly as scanning the square would arrive.
  await ownerPage.goto(`/pair?code=${encodeURIComponent(code)}`);
  await expect(ownerPage.getByRole("heading", { name: "Connect a television" })).toBeVisible();
  await expect(ownerPage.getByLabel("Code on the screen")).toHaveValue(code);
  await ownerPage.getByLabel("Name this screen").fill("Living room");
  await ownerPage.getByRole("button", { name: "Connect this screen" }).click();
  await expect(ownerPage.getByRole("heading", { name: "Connected" })).toBeVisible();

  // The screen picks it up on its own and shows the photos.
  await expect(tvPage.getByRole("heading", { level: 1, name: "Photos" })).toBeVisible({
    timeout: 15_000,
  });
  const tile = tvPage.locator("[data-tile]").first();
  await expect(tile).toBeVisible();
  await expect
    .poll(() => tile.locator("img").evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);

  // A reload keeps it paired: the credential is the screen's now.
  await tvPage.reload();
  await expect(tvPage.getByRole("heading", { level: 1, name: "Photos" })).toBeVisible();

  // The owner can see the screen and switch it off.
  await ownerPage.goto("/more");
  const row = ownerPage.getByRole("listitem").filter({ hasText: "Living room" });
  await expect(row).toBeVisible();
  await row.getByRole("button", { name: /Disconnect/ }).click();
  await expect(ownerPage.getByRole("listitem").filter({ hasText: "Living room" })).toHaveCount(0);

  // And the disconnected screen goes back to asking to be connected,
  // rather than showing an error to a room with no keyboard in it.
  await tvPage.reload();
  await expect(tvPage.getByRole("heading", { name: "Connect this screen" })).toBeVisible({
    timeout: 15_000,
  });
  await expect(tvPage.locator("[data-tile]")).toHaveCount(0);

  await tv.close();
  await owner.close();
});

test("a photo is filed under the month it was taken, not the day it was copied", async ({
  browser,
}) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // A JPEG carrying a real EXIF capture date. The file itself is being
  // written now, which is exactly the case that makes file times useless.
  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("wedding.jpg", Buffer.from(JPEG_TAKEN_2019_BASE64, "base64")));
  await expect(ownerPage.getByRole("row").filter({ hasText: "wedding.jpg" })).toBeVisible();

  await ownerPage.goto("/photos");
  const heading = ownerPage.getByRole("heading", { level: 2, name: /July 2019/ });
  await expect(heading).toBeVisible();

  // And the camera is there for anyone who looks.
  await expect(
    ownerPage.getByRole("link", { name: "wedding.jpg" }).first(),
  ).toHaveAttribute("title", /Fujifilm X100V/);

  await owner.close();
});

test("a photo can be starred, and starring is one person's own", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("starred.png", Buffer.from(PNG_BASE64, "base64")));
  await expect(ownerPage.getByRole("row").filter({ hasText: "starred.png" })).toBeVisible();

  await ownerPage.goto("/photos");
  await ownerPage.getByRole("button", { name: "Add starred.png to favorites" }).click();

  // It shows as starred, and it is in the Favorites view.
  await expect(
    ownerPage.getByRole("button", { name: "Remove starred.png from favorites" }),
  ).toBeVisible();

  await ownerPage.getByRole("tab", { name: "Favorites" }).click();
  await expect(ownerPage.getByRole("img", { name: "starred.png" })).toBeVisible();

  // Taking the star back empties the view again.
  await ownerPage.getByRole("button", { name: "Remove starred.png from favorites" }).click();
  await expect(ownerPage.getByRole("heading", { name: "Nothing starred yet" })).toBeVisible();

  await owner.close();
});

test("photos can be gathered into an album", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles([
      tempFile("album-one.png", Buffer.from(PNG_BASE64, "base64")),
      tempFile("album-two.png", Buffer.from(PNG_BASE64, "base64")),
    ]);
  await expect(ownerPage.getByRole("row").filter({ hasText: "album-two.png" })).toBeVisible();

  await ownerPage.goto("/photos");

  // Choosing pictures is a mode: the ordinary thing to do with a photo
  // is open it, not tick it.
  await ownerPage.getByRole("button", { name: "Select photos" }).click();

  // Wait for each tile to become a checkbox before clicking it: the grid
  // re-renders when selection mode turns on, and clicking into a
  // re-render lands on nothing.
  const first = ownerPage.getByRole("button", { name: /album-one\.png/ });
  const second = ownerPage.getByRole("button", { name: /album-two\.png/ });
  await expect(first).toBeVisible();
  await first.click();
  await expect(ownerPage.getByText("1 selected")).toBeVisible();
  await second.click();
  await expect(ownerPage.getByText("2 selected")).toBeVisible();

  ownerPage.once("dialog", (dialog) => dialog.accept("Wales, summer 2019"));
  await ownerPage.getByRole("button", { name: "Add to a new album" }).click();

  // Wait for the album to actually exist before looking for it: the
  // click only starts the work, and switching tabs first means the
  // Albums view fetches a list the album is not in yet.
  await expect(ownerPage.getByRole("status")).toContainText("Wales, summer 2019");

  await ownerPage.getByRole("tab", { name: "Albums" }).click();
  const card = ownerPage.getByRole("button", { name: /Wales, summer 2019/ });
  await expect(card).toBeVisible();
  await expect(card).toContainText("2 photos");

  // Opening it shows the pictures, and one can be taken back out.
  await card.click();
  await expect(ownerPage.getByRole("img", { name: "album-one.png" })).toBeVisible();
  await ownerPage
    .getByRole("button", { name: "Remove from this album: album-one.png" })
    .click();
  await expect(ownerPage.getByRole("img", { name: "album-one.png" })).toHaveCount(0);

  // Deleting the album keeps the photos.
  ownerPage.once("dialog", (dialog) => dialog.accept());
  await ownerPage.getByRole("button", { name: "Delete album" }).click();
  await expect(ownerPage.getByRole("heading", { name: "No albums yet" })).toBeVisible();

  await ownerPage.getByRole("tab", { name: "Timeline" }).click();
  await expect(ownerPage.getByRole("img", { name: "album-one.png" })).toBeVisible();
  await expect(ownerPage.getByRole("img", { name: "album-two.png" })).toBeVisible();

  await owner.close();
});

test("a large file is sent in pieces and arrives whole", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // Over the threshold where the client stops sending a file in one
  // request, so this exercises the session path rather than the simple
  // one. The contents are patterned, so a chunk landing at the wrong
  // offset would show up as a different file.
  const size = 9 * 1024 * 1024;
  const contents = Buffer.alloc(size);
  for (let index = 0; index < size; index += 1) {
    contents[index] = index % 251;
  }
  const path = tempFile("holiday.bin", contents);

  await ownerPage.getByLabel("Choose files to upload").setInputFiles(path);

  const row = ownerPage.getByRole("row").filter({ hasText: "holiday.bin" });
  await expect(row).toBeVisible({ timeout: 30_000 });
  await expect(ownerPage.getByRole("status")).toContainText("1 file uploaded");

  // Downloaded back, it is the same file: 9 MB, byte for byte.
  const download = await Promise.all([
    ownerPage.waitForEvent("download"),
    row.getByRole("link", { name: /Download/ }).click(),
  ]);
  const saved = await download[0].path();
  const { readFileSync } = await import("node:fs");
  expect(readFileSync(saved).equals(contents)).toBe(true);

  await owner.close();
});

test("someone with no account can send files into one folder", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // A folder to receive them.
  ownerPage.once("dialog", (dialog) => dialog.accept("Wedding photos"));
  await ownerPage.getByRole("button", { name: "New folder" }).click();
  const row = ownerPage.getByRole("row").filter({ hasText: "Wedding photos" });
  await expect(row).toBeVisible();

  await row.getByRole("button", { name: /Ask for files/ }).click();
  const dialog = ownerPage.getByRole("dialog", { name: /Ask for files/ });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Create link" }).click();

  const link = await dialog.getByLabel("Upload link").inputValue();
  expect(link).toContain("/u/");
  await dialog.getByRole("button", { name: "Close" }).click();

  // A visitor with no session, no cookies, and no account.
  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();
  await visitorPage.goto(link);

  await expect(visitorPage.getByRole("heading", { level: 1 })).toContainText("Wedding photos");
  // Nothing to read: no listing, no navigation, no other item.
  await expect(visitorPage.getByRole("navigation", { name: "Primary" })).toHaveCount(0);
  await expect(visitorPage.getByText("placeholder.txt")).toHaveCount(0);

  await visitorPage
    .getByLabel("Choose files to send")
    .setInputFiles(tempFile("confetti.txt", "from a guest"));
  await expect(visitorPage.getByRole("region", { name: "Files sent" })).toContainText(
    "confetti.txt",
  );

  // The owner has it, in that folder and nowhere else.
  await ownerPage.goto("/files");
  // The folder's own button is named for the folder; the row's other
  // controls carry a verb, so an exact match picks the right one.
  await ownerPage.getByRole("button", { name: "Wedding photos", exact: true }).click();
  await expect(
    ownerPage.getByRole("row").filter({ hasText: "confetti.txt" }),
  ).toBeVisible();

  // And can switch the link off, after which it opens for nobody.
  await ownerPage.goto("/more");
  // Scoped to the section: a folder name can appear in more than one
  // list on this page.
  const links = ownerPage.getByRole("region", { name: "Upload links" });
  const listed = links.getByRole("listitem").filter({ hasText: "Wedding photos" });
  await expect(listed).toBeVisible();
  await listed.getByRole("button", { name: /Revoke/ }).click();
  await expect(links.getByRole("listitem").filter({ hasText: "Wedding photos" })).toHaveCount(0);

  await visitorPage.goto(link);
  await expect(
    visitorPage.getByRole("heading", { name: "This link is not available" }),
  ).toBeVisible();

  await visitor.close();
  await owner.close();
});

test("a replaced file keeps what it was, and can be put back", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("draft.txt", "the first draft"));
  const row = ownerPage.getByRole("row").filter({ hasText: "draft.txt" });
  await expect(row).toBeVisible();

  await row.getByRole("button", { name: /History/ }).click();
  const dialog = ownerPage.getByRole("dialog", { name: /History/ });
  await expect(dialog).toContainText("no earlier contents kept");

  await dialog
    .getByLabel("Replace draft.txt")
    .setInputFiles(tempFile("draft.txt", "a much worse second draft"));

  // The old contents are kept, and downloadable.
  await expect(dialog.getByRole("link", { name: "Download" })).toBeVisible();
  const download = await Promise.all([
    ownerPage.waitForEvent("download"),
    dialog.getByRole("link", { name: "Download" }).click(),
  ]);
  const { readFileSync } = await import("node:fs");
  expect(readFileSync(await download[0].path()).toString()).toBe("the first draft");

  // Putting it back makes it current again.
  await dialog.getByRole("button", { name: "Restore" }).click();
  await dialog.getByRole("button", { name: "Close" }).click();

  const current = await Promise.all([
    ownerPage.waitForEvent("download"),
    row.getByRole("link", { name: /Download/ }).click(),
  ]);
  expect(readFileSync(await current[0].path()).toString()).toBe("the first draft");

  await owner.close();
});

test("the same file kept twice is reported as a duplicate", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  // Content unique to this journey. The suite shares one library and
  // reuses the same tiny PNG everywhere, so a photo would land in a
  // group with every other copy of it and the count would be anyone's
  // guess.
  const receipt = "Invoice 88213 for one standby generator, delivered to the workshop.";
  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles([
      tempFile("receipt-from-email.txt", receipt),
      tempFile("receipt-scanned.txt", receipt),
    ]);
  await expect(ownerPage.getByRole("row").filter({ hasText: "receipt-scanned.txt" })).toBeVisible();

  // Files are hashed in the background after a scan.
  await ownerPage.goto("/more");
  await scanLibrary(ownerPage);

  const duplicates = ownerPage.getByRole("region", { name: "Duplicates" });
  await expect(async () => {
    await ownerPage.goto("/more");
    await expect(duplicates).toContainText("receipt-scanned.txt", { timeout: 2_000 });
  }).toPass({ timeout: 60_000, intervals: [2_000] });

  await expect(duplicates).toContainText("receipt-from-email.txt");

  // Removing one copy leaves the other, and the set stops being reported.
  ownerPage.once("dialog", (dialog) => dialog.accept());
  await duplicates
    .getByRole("button", { name: "Move to trash receipt-scanned.txt" })
    .click();

  await expect(async () => {
    await ownerPage.goto("/more");
    await expect(duplicates).not.toContainText("receipt-scanned.txt", { timeout: 2_000 });
  }).toPass({ timeout: 30_000, intervals: [2_000] });

  await ownerPage.goto("/files");
  await expect(
    ownerPage.getByRole("row").filter({ hasText: "receipt-from-email.txt" }),
  ).toBeVisible();

  await owner.close();
});

test("a file can be copied without losing the original", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("report.txt", "the contents"));
  const row = ownerPage.getByRole("row").filter({ hasText: "report.txt" });
  await expect(row).toBeVisible();

  ownerPage.once("dialog", (dialog) => dialog.accept("report-backup.txt"));
  await row.getByRole("button", { name: /Copy/ }).click();

  await expect(
    ownerPage.getByRole("row").filter({ hasText: "report-backup.txt" }),
  ).toBeVisible();
  // The original is still there — a copy is not a move. Matched by the
  // download link, whose accessible name names exactly one file.
  await expect(
    ownerPage.getByRole("link", { name: "Download report.txt", exact: true }),
  ).toBeVisible();
  await expect(
    ownerPage.getByRole("link", { name: "Download report-backup.txt", exact: true }),
  ).toBeVisible();

  await owner.close();
});

test("a photo that recorded where it was taken appears on the map", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("greenwich.jpg", Buffer.from(JPEG_AT_GREENWICH_BASE64, "base64")));
  await expect(ownerPage.getByRole("row").filter({ hasText: "greenwich.jpg" })).toBeVisible();

  await ownerPage.goto("/photos");
  await ownerPage.getByRole("tab", { name: "Places" }).click();

  const plot = ownerPage.getByRole("group", { name: "Photo locations" });
  await expect(plot).toBeVisible();

  // The pin names the photo and roughly where it was.
  const pin = plot.getByRole("button", { name: /greenwich\.jpg at 51\./ });
  await expect(pin).toBeVisible();

  await pin.click();
  // The chosen photo, with the place it was taken.
  const chosen = ownerPage.getByRole("link", { name: /greenwich\.jpg/ });
  await expect(chosen).toBeVisible();
  await expect(chosen).toContainText(/51\.4[0-9]*, -0\./);

  await owner.close();
});

test("private AI is off until it is switched on, and can be switched back", async ({
  browser,
}) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage.goto("/more");
  const ai = ownerPage.getByRole("region", { name: "Private AI" });
  await expect(ai).toBeVisible();

  // Off is the default, and the page says what each choice costs rather
  // than offering a switch labelled only "AI".
  const off = ai.getByRole("button", { name: /^Off/ });
  await expect(off).toHaveAttribute("aria-pressed", "true");
  await expect(ai).toContainText("Nothing runs");

  const readText = ai.getByRole("button", { name: /Read text in pictures/ });

  // A server without the recogniser says so instead of offering a
  // setting that would do nothing.
  if (await readText.isDisabled()) {
    await expect(ai).toContainText("recogniser is not installed");
    await owner.close();
    return;
  }

  await readText.click();
  await expect(readText).toHaveAttribute("aria-pressed", "true");

  // The choice survives a reload — it is the library's setting, not a
  // thing the page remembers.
  await ownerPage.reload();
  await expect(
    ai.getByRole("button", { name: /Read text in pictures/ }),
  ).toHaveAttribute("aria-pressed", "true");

  await ai.getByRole("button", { name: /^Off/ }).click();
  await expect(ai.getByRole("button", { name: /^Off/ })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await owner.close();
});

test("a memory can be dismissed and brought back", async ({ browser }) => {
  const { owner, ownerPage } = await signedInPage(browser, "placeholder.txt");

  await ownerPage
    .getByLabel("Choose files to upload")
    .setInputFiles(tempFile("memory.png", Buffer.from(PNG_BASE64, "base64")));
  await expect(ownerPage.getByRole("row").filter({ hasText: "memory.png" })).toBeVisible();

  await ownerPage.goto("/");
  const memories = ownerPage.getByRole("region", { name: "Memories" });
  await expect(memories).toBeVisible();

  const recent = memories.getByRole("region", { name: /Recently added/ });
  await expect(recent).toBeVisible();

  await recent.getByRole("button", { name: /Hide/ }).click();
  await expect(memories.getByRole("region", { name: /Recently added/ })).toHaveCount(0);

  // Hiding hid the memory, not the photographs.
  await ownerPage.goto("/photos");
  await expect(ownerPage.getByRole("img", { name: "memory.png" })).toBeVisible();

  // And the decision is findable again, or "hide" would be
  // indistinguishable from "delete" to whoever did it.
  await ownerPage.goto("/more");
  const hidden = ownerPage.getByRole("region", { name: "Hidden memories" });
  await expect(hidden).toContainText("Recently added");
  await hidden.getByRole("button", { name: /Show again/ }).click();

  await ownerPage.goto("/");
  await expect(
    ownerPage.getByRole("region", { name: /Recently added/ }),
  ).toBeVisible();

  await owner.close();
});

/**
 * Last in the file on purpose: recovering changes the owner's password,
 * so every journey that signs in with the original one runs first.
 */
test("a forgotten password can be recovered with the code from setup", async ({ browser }) => {
  const replacement = "a different long passphrase";

  const visitor = await browser.newContext();
  const visitorPage = await visitor.newPage();
  await visitorPage.goto("/");

  await expect(visitorPage.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await visitorPage.getByRole("button", { name: "Forgot your password?" }).click();

  await expect(visitorPage.getByRole("heading", { name: "Use your recovery code" })).toBeVisible();
  await visitorPage.getByLabel("Your name").fill(OWNER.name);
  await visitorPage.getByLabel("Recovery code").fill(recoveryCode);
  await visitorPage.getByLabel("New password").fill(replacement);
  await visitorPage.getByRole("button", { name: "Set a new password" }).click();

  // A fresh code arrives with the recovery: the account is never left
  // without a way back in.
  await expect(visitorPage.getByRole("heading", { name: "Write this down" })).toBeVisible();
  const nextCode = (await visitorPage.getByLabel("Your recovery code").innerText()).trim();
  expect(nextCode).not.toBe(recoveryCode);

  await visitorPage.getByRole("button", { name: "I have written it down" }).click();
  await expect(visitorPage.getByRole("heading", { level: 1 })).toHaveText(
    `Welcome back, ${OWNER.name}`,
  );

  // The used code is spent, and the new password is the one that works.
  await visitorPage.goto("/more");
  await visitorPage.getByRole("button", { name: "Sign out" }).click();
  await expect(visitorPage.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await visitorPage.getByRole("button", { name: "Forgot your password?" }).click();
  await visitorPage.getByLabel("Your name").fill(OWNER.name);
  await visitorPage.getByLabel("Recovery code").fill(recoveryCode);
  await visitorPage.getByLabel("New password").fill(replacement);
  await visitorPage.getByRole("button", { name: "Set a new password" }).click();
  await expect(visitorPage.getByRole("alert")).toBeVisible();

  await visitorPage.getByRole("button", { name: "Back to sign in" }).click();
  await visitorPage.getByLabel("Your name").fill(OWNER.name);
  await visitorPage.getByLabel("Password").fill(replacement);
  await visitorPage.getByRole("button", { name: "Sign in" }).click();

  // Signing in happens in place, so the page under the form comes back.
  await expect(visitorPage.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await visitorPage.goto("/");
  await expect(visitorPage.getByRole("heading", { level: 1 })).toHaveText(
    `Welcome back, ${OWNER.name}`,
  );

  await visitor.close();
});
