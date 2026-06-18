import { test, expect } from "@playwright/test";

/**
 * Login-view specs. These run unauthenticated:
 *  - "renders" works with no backend (a failed /api/me falls back to login).
 *  - "invalid credentials" needs the backend (E2E_BACKEND) but only performs a
 *    *failed* login (no TOTP success), so it never contends with the shared
 *    auth.setup session.
 *
 * The successful-login assertion lives in auth.setup.ts (the single real login
 * per run), so it is intentionally not duplicated here.
 */
test.describe("login view", () => {
  test("renders the login form", async ({ page }) => {
    await page.goto("/v2/");
    await expect(page.getByText("Iniciar sesión")).toBeVisible();
    await expect(page.locator('input[autocomplete="username"]')).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Entrar|Entrando/ }),
    ).toBeVisible();
  });

  test("invalid credentials show a generic error", async ({ page }) => {
    test.skip(!process.env.E2E_BACKEND, "needs the backend (E2E_SMOKE=1)");
    await page.goto("/v2/");
    await page.locator('input[autocomplete="username"]').fill("nobody@example.com");
    await page.locator('input[inputmode="numeric"]').fill("000000");
    await page.getByRole("button", { name: "Entrar" }).click();
    await expect(
      page.getByText(/Correo o código inválido|Error de autenticación/),
    ).toBeVisible();
  });
});
