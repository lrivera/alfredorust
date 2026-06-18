import { test, expect } from "@playwright/test";

/**
 * Hermetic finance tests (API mocked). Covers the routed nav for all finance
 * sections and a full CRUD on categories — the generic get_json/post_json/
 * post_empty path shared by every finance page.
 */

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

const FINANCE_LINKS = [
  "Cuentas",
  "Categorías",
  "Contactos",
  "Movimientos",
  "Planes recurrentes",
  "Entradas planificadas",
  "Pronósticos",
];

test.describe("finance (mocked API)", () => {
  test("admin sees all finance nav links", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.goto("/v2/");
    for (const name of FINANCE_LINKS) {
      await expect(page.getByRole("link", { name })).toBeVisible();
    }
  });

  test("admin can list, create and delete categories", async ({ page }) => {
    let cats = [{ id: "c1", name: "Servicios", flow_type: "expense", parent: "" }];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/categories", (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        const b = JSON.parse(req.postData() || "{}");
        cats.push({
          id: `c${cats.length + 1}`,
          name: b.name,
          flow_type: b.flow_type,
          parent: "",
        });
        return route.fulfill({ status: 201, json: { id: `c${cats.length}` } });
      }
      return route.fulfill({ json: cats });
    });
    await page.route("**/api/admin/categories/*/delete", (route) => {
      const parts = route.request().url().split("/");
      const id = parts[parts.length - 2];
      cats = cats.filter((c) => c.id !== id);
      return route.fulfill({ json: { ok: true } });
    });

    await page.goto("/v2/");
    await page.getByRole("link", { name: "Categorías" }).click();
    await expect(page.getByRole("cell", { name: "Servicios" })).toBeVisible();

    // Create
    await page.getByPlaceholder("Servicios").fill("Renta");
    await page.getByRole("button", { name: /Crear categoría|Guardando/ }).click();
    await expect(page.getByRole("cell", { name: "Renta" })).toBeVisible();

    // Delete the seeded row
    await page
      .getByRole("row", { name: /Servicios/ })
      .getByRole("button", { name: "Eliminar" })
      .click();
    await expect(page.getByRole("cell", { name: "Servicios" })).toHaveCount(0);
  });
});
