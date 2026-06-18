import { test, expect, type Page } from "@playwright/test";

/**
 * The topbar tenant switcher: a <select> of the user's companies that navigates
 * to the chosen tenant's subdomain. We assert rendering (options + the active
 * company preselected) hermetically; the actual cross-subdomain navigation is
 * out of scope for a localhost mock.
 */

const meWith = (companies: Array<{ id: string; name: string; slug: string; active: boolean }>) => ({
  username: "demo@example.com",
  company: companies.find((c) => c.active)?.name ?? companies[0].name,
  company_slug: companies.find((c) => c.active)?.slug ?? companies[0].slug,
  role: "admin",
  permissions: [],
  companies,
});

async function bootstrap(page: Page, me: object) {
  await page.route("**/api/me", (r) => r.fulfill({ json: me }));
  await page.goto("/v2/");
  await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();
}

test.describe("company switcher (mocked API)", () => {
  test("lists memberships with the active company preselected", async ({ page }) => {
    await bootstrap(
      page,
      meWith([
        { id: "1", name: "Acme", slug: "acme", active: true },
        { id: "2", name: "Globex", slug: "globex", active: false },
      ]),
    );

    const switcher = page.getByRole("combobox", { name: "Cambiar de compañía" });
    await expect(switcher).toBeVisible();
    // Active tenant preselected, both options present.
    await expect(switcher).toHaveValue("acme");
    await expect(switcher.locator("option")).toHaveText(["Acme", "Globex"]);
  });

  test("is hidden when the user belongs to a single company", async ({ page }) => {
    await bootstrap(page, meWith([{ id: "1", name: "Acme", slug: "acme", active: true }]));
    await expect(page.getByRole("combobox", { name: "Cambiar de compañía" })).toHaveCount(0);
  });
});
