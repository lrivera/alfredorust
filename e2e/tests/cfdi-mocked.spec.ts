import { test, expect, type Page } from "@playwright/test";

const ADMIN_ME = {
  email: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

async function me(page: Page) {
  await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
}

test.describe("cfdi (mocked API)", () => {
  test("lists CFDIs from /api/admin/cfdis/data", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/cfdis/data", (r) =>
      r.fulfill({
        json: {
          company_rfcs: ["XAXX010101000"],
          items: [
            {
              uuid: "u1",
              folio: "A-100",
              tipo: "I",
              fecha: "2026-01-05T00:00:00Z",
              subtotal: 1000,
              iva: 160,
              total: 1160,
              moneda: "MXN",
              forma_pago: "",
              metodo_pago: "",
              emisor_rfc: "",
              emisor_nombre: "ACME",
              receptor_rfc: "",
              receptor_nombre: "Cliente",
              concepto: "",
              es_emitido: true,
            },
          ],
        },
      }),
    );

    await page.goto("/v2/cfdi");
    await expect(page.getByRole("cell", { name: "A-100" })).toBeVisible();
    await expect(page.getByRole("cell", { name: "Emitido" })).toBeVisible();
    await expect(page.getByRole("cell", { name: /1160\.00/ })).toBeVisible();
  });
});
