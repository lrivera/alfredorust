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

function gridJson(over: Record<string, unknown> = {}) {
  return {
    date: "2026-06-18",
    can_edit: true,
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
          {
            hour: 8,
            is_work_hour: true,
            resources: [{ resource_id: "r1", label: "Grúa", selected: false }],
          },
        ],
      },
    ],
    ...over,
  };
}

test.describe("resource usages grid (mocked API)", () => {
  test("project header, open a cell, toggle a resource and save", async ({ page }) => {
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
    // Project grouping header links to the project.
    await expect(page.getByRole("link", { name: "Casa" })).toHaveAttribute(
      "href",
      "/v2/projects/p1",
    );
    await expect(page.getByText("Cimentación")).toBeVisible();
    // Concept row shows quantity + unit.
    await expect(page.getByText("1.00 lote")).toBeVisible();

    // The cell is a dropdown summarizing selections ("+" when empty); open it.
    await page.getByText("+", { exact: true }).click();
    await page.getByRole("checkbox", { name: "Grúa" }).check();
    await page.getByRole("button", { name: /Guardar captura|Guardando/ }).click();

    await expect.poll(() => body?.selections).toEqual([
      { concept_id: "c1", hour: 8, resource_id: "r1" },
    ]);
  });

  test("read-only date shows badge and disables resources", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/resource_usages/grid**", (route) =>
      route.fulfill({ json: gridJson({ can_edit: false }) }),
    );

    await page.goto("/v2/resource-usages");
    await expect(page.getByText("Solo lectura")).toBeVisible();
    await expect(page.getByRole("button", { name: /Guardar/ })).toHaveCount(0);

    await page.getByText("+", { exact: true }).click();
    await expect(page.getByRole("checkbox", { name: "Grúa" })).toBeDisabled();
  });
});
