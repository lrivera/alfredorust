import { test, expect, type Page } from "@playwright/test";

/**
 * The finance screens (movimientos, CFDIs) render KPI + monthly line + donut
 * charts ported from v1. These assert the charts appear from the loaded data.
 */

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

async function me(page: Page) {
  await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
}

test.describe("finance charts (mocked API)", () => {
  test("transactions show KPIs, line chart, donut and category bars", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/categories", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/accounts", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/planned-entries", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/transactions/data", (r) =>
      r.fulfill({
        json: [
          { id: "t1", date: "2026-01-10T00:00:00Z", description: "Venta", tx_type: "income", amount: 1000, category: "Ventas", account_from: "", account_to: "", is_confirmed: true },
          { id: "t2", date: "2026-02-15T00:00:00Z", description: "Renta", tx_type: "expense", amount: 400, category: "Renta", account_from: "", account_to: "", is_confirmed: true },
        ],
      }),
    );

    await page.goto("/v2/transactions");
    await expect(page.getByText("Ingresos", { exact: true })).toBeVisible();
    await expect(page.getByText("Por categoría", { exact: true })).toBeVisible();
    // KPI value + at least the two chart SVGs (line + donut).
    await expect(page.getByText("1000.00").first()).toBeVisible();
    expect(await page.locator("svg").count()).toBeGreaterThanOrEqual(2);
  });

  test("cfdis show KPIs and charts", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/sat-configs", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/cfdis/data", (r) =>
      r.fulfill({
        json: {
          company_rfcs: ["XAXX010101000"],
          items: [
            { uuid: "u1", folio: "A-100", tipo: "I", fecha: "2026-01-05T00:00:00Z", total: 1160, moneda: "MXN", emisor_nombre: "ACME", receptor_nombre: "Cliente", es_emitido: true },
            { uuid: "u2", folio: "B-200", tipo: "I", fecha: "2026-02-05T00:00:00Z", total: 500, moneda: "MXN", emisor_nombre: "Prov", receptor_nombre: "ACME", es_emitido: false },
          ],
        },
      }),
    );

    await page.goto("/v2/cfdi");
    await expect(page.getByText("Emitidos", { exact: true })).toBeVisible();
    await expect(page.getByText("Recibidos", { exact: true })).toBeVisible();
    expect(await page.locator("svg").count()).toBeGreaterThanOrEqual(2);
  });
});
