import { test, expect } from "@playwright/test";

/**
 * Live CRUD against the real backend, **test tenant only**. Self-cleaning:
 * creates a uniquely-named account, verifies it, then deletes it — so it
 * leaves no junk behind.
 *
 * Runs in the "live" project (server smoke mode), which reuses the shared
 * session from auth.setup.ts via storageState — so NO login happens here and
 * many such specs can run in parallel off a single real TOTP login.
 * The backend enforces tenant isolation, so this only touches the test tenant.
 */
test.describe("accounts (live · test tenant · self-cleaning)", () => {
  test("create then delete an account", async ({ page }, testInfo) => {
    // Unique per worker + time so parallel live specs never touch each other's row.
    const name = `E2E temp w${testInfo.workerIndex}-${Date.now()}-${Math.floor(
      Math.random() * 1e6,
    )}`;
    await page.goto("/v2/accounts");

    // Create a uniquely-named account.
    await page.getByPlaceholder("Cuenta principal").fill(name);
    await page.getByRole("button", { name: /Crear cuenta|Guardando/ }).click();
    await expect(page.getByRole("cell", { name })).toBeVisible();

    // Delete it (cleanup) — leaves the tenant as we found it.
    await page
      .getByRole("row", { name })
      .getByRole("button", { name: "Eliminar" })
      .click();
    await expect(page.getByRole("cell", { name })).toHaveCount(0);
  });
});
