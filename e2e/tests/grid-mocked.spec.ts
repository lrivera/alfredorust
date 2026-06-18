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

function gridJson(canEdit = true) {
  return {
    date: "2026-06-18",
    can_edit: canEdit,
    statuses: [{ id: "st1", name: "En proceso" }],
    rows: [
      {
        concept_id: "c1",
        project_id: "p1",
        project_title: "Casa",
        status_name: "En proceso",
        concept_name: "Cimentación",
        quantity: 1,
        unit: "lote",
        cells: [
          { hour: 8, is_work_hour: true, resources: [{ resource_id: "r1", label: "Grúa", selected: false }] },
        ],
      },
    ],
  };
}

test.describe("resource usages grid (mocked API)", () => {
  test("renders the v1 grid, cycles a cell and saves", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/resource_usages/grid**", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ json: { ok: true } });
      }
      return route.fulfill({ json: gridJson() });
    });

    await page.goto("/v2/resource-usages");

    // v1 markup: project header link, concept name, quantity, the big date.
    await expect(page.getByRole("link", { name: "Casa" }).first()).toBeVisible();
    await expect(page.getByText("Cimentación")).toBeVisible();
    await expect(page.getByText("2026-06-18").first()).toBeVisible();

    // Empty cell shows "+".
    const cellBtn = page.locator("[data-resource-cell-button]").first();
    await expect(cellBtn).toHaveText("+");

    // Short press (down+up, no long-press) cycles to the first resource.
    await cellBtn.dispatchEvent("pointerdown");
    await cellBtn.dispatchEvent("pointerup");
    await expect(cellBtn).toHaveText("Grúa");

    await page.getByRole("button", { name: /Guardar captura|Guardando/ }).click();
    await expect.poll(() => body?.selections).toEqual([
      { concept_id: "c1", hour: 8, resource_id: "r1" },
    ]);
  });

  test("read-only hides save and disables inputs", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/resource_usages/grid**", (route) =>
      route.fulfill({ json: gridJson(false) }),
    );
    await page.goto("/v2/resource-usages");
    await expect(page.getByText("Solo lectura")).toBeVisible();
    await expect(page.getByRole("button", { name: /Guardar captura/ })).toHaveCount(0);
  });
});
