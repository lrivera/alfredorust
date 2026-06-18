import { test, expect } from "@playwright/test";

/**
 * Hermetic finance/accounts tests: API mocked with `page.route`, no backend.
 * Exercises the routed shell (sidebar nav), the admin-gated create form, the
 * list, and delete — the reference CRUD pattern for the rest of `finance`.
 */

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

const STAFF_ME = { ...ADMIN_ME, username: "staff@example.com", role: "staff" };

test.describe("finance / accounts (mocked API)", () => {
  test("admin can list, create and delete accounts", async ({ page }) => {
    let accounts = [
      {
        id: "a1",
        name: "Caja",
        company: "Acme",
        account_type: "cash",
        currency: "MXN",
        is_active: true,
      },
    ];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/accounts", (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        const body = JSON.parse(req.postData() || "{}");
        accounts.push({
          id: `a${accounts.length + 1}`,
          name: body.name,
          company: "Acme",
          account_type: body.account_type,
          currency: body.currency || "MXN",
          is_active: true,
        });
        return route.fulfill({ status: 201, json: { id: `a${accounts.length}` } });
      }
      return route.fulfill({ json: accounts });
    });
    await page.route("**/api/admin/accounts/*/delete", (route) => {
      const parts = route.request().url().split("/");
      const id = parts[parts.length - 2];
      accounts = accounts.filter((a) => a.id !== id);
      return route.fulfill({ json: { ok: true } });
    });

    await page.goto("/v2/");
    await page.getByRole("link", { name: "Cuentas" }).click();
    await expect(page.getByText("Caja")).toBeVisible();

    // Create
    await page.getByPlaceholder("Cuenta principal").fill("Banco BBVA");
    await page.getByRole("button", { name: /Crear cuenta|Guardando/ }).click();
    await expect(page.getByText("Banco BBVA")).toBeVisible();

    // Delete the original row
    await page
      .getByRole("row", { name: /Caja/ })
      .getByRole("button", { name: "Eliminar" })
      .click();
    await expect(page.getByText("Caja")).toHaveCount(0);
  });

  test("admin can edit an account", async ({ page }) => {
    let accounts = [
      {
        id: "a1",
        name: "Caja",
        company: "Acme",
        account_type: "cash",
        currency: "MXN",
        is_active: true,
      },
    ];
    const detail = {
      name: "Caja",
      account_type: "cash",
      currency: "MXN",
      is_active: true,
      notes: "nota",
    };

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/accounts", (r) => r.fulfill({ json: accounts }));
    await page.route("**/api/admin/accounts/*/update", (route) => {
      const b = JSON.parse(route.request().postData() || "{}");
      accounts = accounts.map((a) => (a.id === "a1" ? { ...a, name: b.name } : a));
      return route.fulfill({ json: { ok: true } });
    });
    // Detail GET (single trailing segment) — registered last so it wins over
    // the list route for `/accounts/a1`, but not over `/accounts/a1/update`.
    await page.route("**/api/admin/accounts/*", (route) => route.fulfill({ json: detail }));

    await page.goto("/v2/");
    await page.getByRole("link", { name: "Cuentas" }).click();
    await page
      .getByRole("row", { name: /Caja/ })
      .getByRole("button", { name: "Editar" })
      .click();
    await expect(page.getByText("Editar cuenta")).toBeVisible();

    await page.getByPlaceholder("Cuenta principal").fill("Caja Chica");
    await page.getByRole("button", { name: "Guardar cambios" }).click();
    await expect(page.getByRole("cell", { name: "Caja Chica" })).toBeVisible();
  });

  test("staff does not see the accounts nav link", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.goto("/v2/");
    await expect(page.getByRole("link", { name: "Inicio" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Cuentas" })).toHaveCount(0);
  });
});
