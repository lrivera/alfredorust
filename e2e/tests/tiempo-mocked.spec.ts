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
  test("renders buckets from /api/tiempo", async ({ page }) => {
    await me(page);
    await page.route("**/api/tiempo*", (r) =>
      r.fulfill({
        json: [
          {
            start: "2026-01-01T00:00:00Z",
            end: "2026-01-31T00:00:00Z",
            real_income: 1000,
            real_expense: 400,
            net_real: 600,
            net_planned: 500,
            cumulative_real: 600,
            cumulative_planned: 500,
          },
        ],
      }),
    );

    await page.goto("/v2/tiempo");
    await expect(page.getByRole("cell", { name: /2026-01-01/ })).toBeVisible();
    await expect(page.getByRole("cell", { name: "1000.00" })).toBeVisible();
    await expect(page.getByRole("cell", { name: "600.00" }).first()).toBeVisible();
  });
});
