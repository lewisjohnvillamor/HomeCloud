import { defineConfig, devices } from "@playwright/test";

/**
 * End-to-end journeys against the real stack: the Rust API, a real
 * PostgreSQL database, and the built web app.
 *
 * Kept separate from `playwright.config.ts` so the fast UI checks do not
 * need a database, and so these tests can assume a *fresh* deployment —
 * the first screen is first-run setup. `make e2e-full` recreates the
 * database before running them.
 */
const webPort = Number(process.env.PLAYWRIGHT_WEB_PORT ?? 3101);
const apiPort = Number(process.env.PLAYWRIGHT_API_PORT ?? 8081);
// `localhost` rather than an IP: WebAuthn binds credentials to a domain,
// and an IP address is not a valid relying-party id.
const baseURL = `http://localhost:${webPort}`;
const apiOrigin = `http://127.0.0.1:${apiPort}`;

const databaseUrl =
  process.env.HOMECLOUD_E2E_DATABASE_URL ??
  "postgres://homecloud:homecloud@127.0.0.1:5432/homecloud_e2e";

export default defineConfig({
  testDir: "./e2e-full",
  fullyParallel: false,
  // The journeys share one deployment, so they run in order in one worker.
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  reporter: process.env.CI ? [["html", { open: "never" }], ["list"]] : "list",
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  projects: [
    { name: "desktop-chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: [
    {
      command: "cargo run --quiet --bin homecloud-api",
      cwd: "../..",
      url: `${apiOrigin}/health/ready`,
      reuseExistingServer: false,
      timeout: 300_000,
      stdout: "pipe",
      stderr: "pipe",
      env: {
        HOMECLOUD_DATABASE_URL: databaseUrl,
        HOMECLOUD_LISTEN_ADDR: `127.0.0.1:${apiPort}`,
        HOMECLOUD_STORAGE_ROOT: "./apps/web/.playwright-library",
        // Passkeys are bound to an origin, so the journeys need the one
        // the browser actually visits.
        HOMECLOUD_PUBLIC_ORIGIN: baseURL,
        RUST_LOG: "warn,homecloud_api=info",
      },
    },
    {
      command: `pnpm build && pnpm exec next start --port ${webPort}`,
      url: baseURL,
      reuseExistingServer: false,
      timeout: 300_000,
      env: { HOMECLOUD_API_ORIGIN: apiOrigin },
    },
  ],
});
