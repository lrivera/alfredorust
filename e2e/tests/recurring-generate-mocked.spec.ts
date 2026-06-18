import { test, expect } from "@playwright/test";

/**
 * Hermetic test for the recurring-plan "Generar" action (materializes planned
 * entries from a plan). API mocked with `page.route`, no backend.
 */

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "c1", name: "Acme", slug: "acme", active: true }],
};

test.describe("recurring plans / generate (mocked API)", () => {
  test("admin can generate entries from a plan", async ({ page }) => {
    let generateHit = false;
    const plans = [
      {
        id: "rp1",
        name: "Renta",
        flow_type: "expense",
        amount_estimated: 1000,
        frequency: "monthly",
        start_date: "2026-01-01T00:00:00Z",
        is_active: true,
      },
    ];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/categories", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/accounts", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/contacts", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/recurring-plans/*/generate", (route) => {
      generateHit = true;
      return route.fulfill({ json: { ok: true } });
    });
    await page.route("**/api/admin/recurring-plans", (r) => r.fulfill({ json: plans }));

    await page.goto("/v2/recurring-plans");
    await expect(page.getByRole("cell", { name: "Renta" })).toBeVisible();

    await page
      .getByRole("row", { name: /Renta/ })
      .getByRole("button", { name: "Generar" })
      .click();

    await expect(page.getByText("Entradas generadas")).toBeVisible();
    expect(generateHit).toBe(true);
  });
});
