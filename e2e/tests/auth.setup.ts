import { test as setup, expect } from "@playwright/test";
import { authenticator } from "otplib";

/**
 * Authenticate ONCE per run and persist the session (cookie) to storageState.
 * Every live/authenticated spec reuses it, so we do a single real TOTP login
 * regardless of how many tests run in parallel — no token contention.
 *
 * Only runs in server smoke mode (the config wires this project in when
 * E2E_SMOKE=1 and feeds E2E_EMAIL / E2E_TOTP_SECRET from .env.smoke).
 */
const authFile = ".auth/state.json";

setup("authenticate", async ({ page }) => {
  const email = process.env.E2E_EMAIL;
  const secret = process.env.E2E_TOTP_SECRET;
  if (!email || !secret) {
    throw new Error("auth setup needs E2E_EMAIL and E2E_TOTP_SECRET (see .env.smoke)");
  }

  await page.goto("/v2/");
  await page.getByPlaceholder("tu@correo.com").fill(email);
  await page.locator('input[inputmode="numeric"]').fill(authenticator.generate(secret));
  await page.getByRole("button", { name: "Entrar" }).click();

  // Confirms login works AND that we landed on the authenticated shell.
  await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();

  await page.context().storageState({ path: authFile });
});
