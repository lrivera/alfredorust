import { defineConfig, devices } from "@playwright/test";
import dotenv from "dotenv";

// Smoke/server credentials live in the repo-root .env.smoke (gitignored):
//   SPCLI_BASE_URL (login host), SPCLI_TENANT, SPCLI_EMAIL, SPCLI_TOTP_SECRET
dotenv.config({ path: "../.env.smoke" });

/**
 * Run modes:
 *
 *  - **Local (default)** — runs against `trunk serve` (port 8091, /v2 base).
 *    Only the "app" project: mocked specs + login-view specs. Hermetic.
 *
 *  - **Server smoke** — `E2E_SMOKE=1`. Targets the deployed SPA on the **test**
 *    tenant. Adds:
 *      • "setup"  — logs in ONCE (real TOTP) and saves the session to
 *                   storageState, so every live spec shares one token (no
 *                   per-test login, no TOTP contention under parallelism).
 *      • "live"   — *.live.spec.ts, reuse the shared session. Each live spec
 *                   must operate on its OWN uniquely-named data so parallel
 *                   workers never touch a row another is deleting.
 *
 * The backend scopes everything to the test tenant; other tenants are untouched.
 * The TOTP secret stays in .env.smoke / CI secrets — never committed.
 */
const useServer = process.env.E2E_SMOKE === "1" || !!process.env.E2E_BASE_URL;

const tenant = process.env.SPCLI_TENANT ?? "test";
// app.alfredorivera.dev -> test.alfredorivera.dev (swap login label for tenant)
const serverBase = (process.env.SPCLI_BASE_URL ?? "https://app.alfredorivera.dev").replace(
  /:\/\/[^.]+\./,
  `://${tenant}.`,
);
const baseURL = process.env.E2E_BASE_URL ?? (useServer ? serverBase : "http://localhost:8091");

if (useServer && process.env.SPCLI_EMAIL) {
  process.env.E2E_BACKEND ??= "1";
  process.env.E2E_EMAIL ??= process.env.SPCLI_EMAIL;
  process.env.E2E_TOTP_SECRET ??= process.env.SPCLI_TOTP_SECRET;
}

const chrome = { ...devices["Desktop Chrome"] };

const liveProjects = useServer
  ? [
      { name: "setup", testMatch: /auth\.setup\.ts/, use: chrome },
      {
        name: "live",
        testMatch: /\.live\.spec\.ts/,
        dependencies: ["setup"],
        use: { ...chrome, storageState: ".auth/state.json" },
      },
    ]
  : [];

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"], ["html", { open: "never" }]],
  use: { baseURL, trace: "on-first-retry" },
  projects: [
    ...liveProjects,
    {
      name: "app",
      testMatch: /.*\.spec\.ts/,
      testIgnore: /\.live\.spec\.ts/,
      use: chrome,
    },
  ],
  webServer: useServer
    ? undefined
    : {
        command: "sh -c '$HOME/.cargo/bin/trunk serve'",
        cwd: "../frontend",
        url: "http://localhost:8091/v2/",
        reuseExistingServer: true,
        timeout: 240_000,
      },
});
