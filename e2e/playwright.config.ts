import { defineConfig, devices } from "@playwright/test";

/**
 * The frontend dev server (`trunk serve`, port 8091) proxies /api, /login and
 * /logout to the Axum backend on :8090 (see frontend/Trunk.toml). Playwright
 * starts trunk for us; the backend must be running separately for the
 * auth-dependent tests (they self-skip unless E2E_BACKEND=1).
 */
export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:8091",
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: "sh -c '$HOME/.cargo/bin/trunk serve'",
    cwd: "../frontend",
    // Trunk serves the app under the /v2/ public_url; the bare root 404s.
    url: "http://localhost:8091/v2/",
    reuseExistingServer: true,
    timeout: 240_000,
  },
});
