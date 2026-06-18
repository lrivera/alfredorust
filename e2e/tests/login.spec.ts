import { test, expect } from "@playwright/test";
import { authenticator } from "otplib";

/**
 * Frontend-only tests: these pass without the backend. When `/api/me` cannot be
 * reached (no backend) the SPA falls back to the anonymous (login) view, which
 * is exactly what we assert here.
 */
test.describe("login view", () => {
  test("renders the login form", async ({ page }) => {
    await page.goto("/v2/");
    await expect(page.getByText("Iniciar sesión")).toBeVisible();
    await expect(page.getByPlaceholder("tu@correo.com")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Entrar|Entrando/ }),
    ).toBeVisible();
  });
});

/**
 * Auth-dependent tests: require the Axum backend on :8090. They self-skip unless
 * E2E_BACKEND=1, so the default suite stays green without a server/DB.
 *
 * The happy-path test additionally needs a known seeded user:
 *   E2E_EMAIL=<email> E2E_TOTP_SECRET=<base32 secret> E2E_BACKEND=1
 */
test.describe("authentication", () => {
  test.skip(
    !process.env.E2E_BACKEND,
    "set E2E_BACKEND=1 (and run the backend) to enable",
  );

  test("invalid credentials show a generic error", async ({ page }) => {
    await page.goto("/v2/");
    await page.getByPlaceholder("tu@correo.com").fill("nobody@example.com");
    await page.locator('input[inputmode="numeric"]').fill("000000");
    await page.getByRole("button", { name: "Entrar" }).click();
    await expect(
      page.getByText(/Correo o código inválido|Error de autenticación/),
    ).toBeVisible();
  });

  test("valid TOTP login lands on the authenticated shell", async ({
    page,
  }) => {
    const email = process.env.E2E_EMAIL;
    const secret = process.env.E2E_TOTP_SECRET;
    test.skip(
      !email || !secret,
      "set E2E_EMAIL and E2E_TOTP_SECRET to run the happy path",
    );

    const code = authenticator.generate(secret!);
    await page.goto("/v2/");
    await page.getByPlaceholder("tu@correo.com").fill(email!);
    await page.locator('input[inputmode="numeric"]').fill(code);
    await page.getByRole("button", { name: "Entrar" }).click();

    // Lands on the shell: logout affordance + companies heading.
    await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();
    await expect(page.getByText("Compañías")).toBeVisible();
  });
});
