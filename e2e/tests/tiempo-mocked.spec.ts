import { test, expect, type Page } from "@playwright/test";

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

test.describe("tiempo timeline (mocked API)", () => {
  test("renders the infinite-scroll widget and switches granularity", async ({ page }) => {
    await me(page);
    let lastUrl = "";
    await page.route("**/api/tiempo*", (route) => {
      lastUrl = route.request().url();
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/tiempo");

    // The v1 controls and strip are present.
    await expect(page.getByRole("button", { name: "Ir a hoy" })).toBeVisible();
    await expect(page.locator("#timelineStrip > div").first()).toBeVisible();
    // The sticky chart renders, and empty buckets show "Sin items".
    await expect(page.locator("#timelineChart svg").first()).toBeVisible();
    await expect(page.getByText("Sin items").first()).toBeVisible();

    // Switching mode reloads with the chosen granularity.
    await page.getByRole("button", { name: "Semana" }).click();
    await expect.poll(() => lastUrl).toContain("mode=week");
  });
});
