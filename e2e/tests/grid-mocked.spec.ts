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

test.describe("resource usages grid (mocked API)", () => {
  test("toggle a cell and save the grid", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/resource_usages/grid**", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ json: { ok: true } });
      }
      return route.fulfill({
        json: {
          date: "2026-06-18",
          can_edit: true,
          statuses: [{ id: "st1", name: "En proceso" }],
          rows: [
            {
              concept_id: "c1",
              project_title: "Casa",
              status_name: "En proceso",
              concept_name: "Cimentación",
              quantity: 1,
              unit: "lote",
              cells: [
                {
                  hour: 8,
                  is_work_hour: true,
                  resources: [{ resource_id: "r1", label: "Grúa", selected: false }],
                },
              ],
            },
          ],
        },
      });
    });

    await page.goto("/v2/resource-usages");
    await expect(page.getByText("Cimentación")).toBeVisible();

    await page.getByRole("checkbox", { name: "Grúa" }).check();
    await page.getByRole("button", { name: /Guardar grid|Guardando/ }).click();

    await expect.poll(() => body?.selections).toEqual([
      { concept_id: "c1", hour: 8, resource_id: "r1" },
    ]);
  });
});
