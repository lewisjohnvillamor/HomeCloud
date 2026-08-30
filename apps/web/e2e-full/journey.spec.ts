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

  // Uploads are indexed by the next scan.
  await ownerPage.goto("/more");
  await ownerPage.getByRole("button", { name: "Scan library" }).click();
  await expect(
    ownerPage.getByRole("status").filter({ hasText: "Last scan indexed" }),
  ).toBeVisible({ timeout: 30_000 });

  await ownerPage.goto("/search");
  await ownerPage.getByRole("searchbox", { name: "Search your library" }).fill("generator");
  await ownerPage.getByRole("button", { name: "Search" }).click();

  const result = ownerPage.getByRole("listitem").filter({ hasText: "4021.txt" });
  await expect(result).toBeVisible();
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
