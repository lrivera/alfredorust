import { test, expect } from "@playwright/test";

/**
 * Hermetic login-flow tests: the JSON API is mocked at the network layer with
 * `page.route`, so these exercise the real SPA UI (form submit → shell, error
 * handling, logout) without a backend or database. The live-backend happy path
 * lives in login.spec.ts behind E2E_BACKEND.
 */

const ME = {
  username: "demo@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: ["view_projects"],
  companies: [
    { id: "1", name: "Acme", slug: "acme", active: true },
    { id: "2", name: "Globex", slug: "globex", active: false },
  ],
};

test.describe("login flow (mocked API)", () => {
  test("valid login lands on the authenticated shell", async ({ page }) => {
    let loggedIn = false;
    await page.route("**/api/me", (route) =>
      loggedIn
        ? route.fulfill({ json: ME })
        : route.fulfill({ status: 401, body: "unauthorized" }),
    );
    await page.route("**/login", (route) => {
      loggedIn = true;
      return route.fulfill({ json: { ok: true, redirect_url: null } });
    });

    await page.goto("/v2/");
    await page.locator('input[autocomplete="username"]').fill("demo@example.com");
    await page.locator('input[inputmode="numeric"]').fill("123456");
    await page.getByRole("button", { name: "Entrar" }).click();

    await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();
    // The dashboard company-switcher heading (a nav link of the same name now
    // also exists, so target the heading specifically).
    await expect(page.getByRole("heading", { name: "Compañías" })).toBeVisible();
    // Both memberships render; the inactive one is a good unique assertion.
    await expect(page.getByText("Globex")).toBeVisible();
  });

  test("invalid credentials show a generic error", async ({ page }) => {
    await page.route("**/api/me", (route) =>
      route.fulfill({ status: 401, body: "unauthorized" }),
    );
    await page.route("**/login", (route) =>
      route.fulfill({ status: 401, json: { ok: false } }),
    );

    await page.goto("/v2/");
    await page.locator('input[autocomplete="username"]').fill("nobody@example.com");
    await page.locator('input[inputmode="numeric"]').fill("000000");
    await page.getByRole("button", { name: "Entrar" }).click();

    await expect(page.getByText("Correo o código inválido")).toBeVisible();
  });

  test("logout returns to the login view", async ({ page }) => {
    let loggedIn = true;
    await page.route("**/api/me", (route) =>
      loggedIn
        ? route.fulfill({ json: ME })
        : route.fulfill({ status: 401, body: "unauthorized" }),
    );
    await page.route("**/logout", (route) => {
      loggedIn = false;
      return route.fulfill({ status: 200, body: "" });
    });

    await page.goto("/v2/");
    await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();
    await page.getByRole("button", { name: "Salir" }).click();
    await expect(page.getByText("Iniciar sesión")).toBeVisible();
  });
});
