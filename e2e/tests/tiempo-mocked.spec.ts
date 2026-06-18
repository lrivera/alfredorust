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

test.describe("tiempo timeline (mocked API)", () => {
  test("renders bucket cards with real/plan rows and drill-down", async ({ page }) => {
    await me(page);
    await page.route("**/api/tiempo*", (r) =>
      r.fulfill({
        json: [
          {
            start: "2026-01-01T00:00:00Z",
            end: "2026-02-01T00:00:00Z",
            real_income: 1000,
            real_expense: 400,
            planned_income: 1200,
            planned_expense: 300,
            net_real: 600,
            net_planned: 900,
            cumulative_real: 600,
            cumulative_planned: 900,
            transactions: [
              { id: "t1", description: "Anticipo", amount: 1000, date: "2026-01-10T00:00:00Z", type: "income" },
            ],
            planned_entries: [
              {
                id: "p1",
                name: "Renta",
                amount_estimated: 300,
                due_date: "2026-01-05T00:00:00Z",
                flow_type: "expense",
                status: "planned",
              },
            ],
          },
          {
            start: "2026-02-01T00:00:00Z",
            end: "2026-03-01T00:00:00Z",
            real_income: 0,
            real_expense: 200,
            planned_income: 0,
            planned_expense: 0,
            net_real: -200,
            net_planned: 0,
            cumulative_real: 400,
            cumulative_planned: 900,
            transactions: [],
            planned_entries: [],
          },
        ],
      }),
    );

    await page.goto("/v2/tiempo");

    // Period label and currency-formatted, sign-aware amounts.
    await expect(page.getByText(/2026-01-01 → 2026-02-01/)).toBeVisible();
    await expect(page.getByText("$1,000", { exact: true })).toBeVisible();
    await expect(page.getByText("-$400", { exact: true })).toBeVisible();

    // Drill-down lists the underlying transaction and planned entry.
    await page.getByText(/1 movimientos · 1 planificadas/).click();
    await expect(page.getByText("Anticipo")).toBeVisible();
    await expect(page.getByText(/Renta/)).toBeVisible();
  });

  test("granularity toggle reloads with the chosen mode", async ({ page }) => {
    await me(page);
    let lastUrl = "";
    await page.route("**/api/tiempo*", (route) => {
      lastUrl = route.request().url();
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/tiempo");
    await page.getByRole("button", { name: "Semana" }).click();
    await expect.poll(() => lastUrl).toContain("mode=week");
  });
});
