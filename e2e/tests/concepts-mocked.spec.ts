import { test, expect, type Page } from "@playwright/test";

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

const group = (page: Page, label: string) =>
  page.locator(`div:has(> label:text-is("${label}"))`);

async function me(page: Page) {
  await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
}

const STATUS = {
  id: "st1",
  name: "En proceso",
  position: 1,
  color: null,
  is_initial: true,
  is_terminal: false,
  is_cancelled: false,
  is_active: true,
};

test.describe("concept statuses (mocked API)", () => {
  test("create a status", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/concept_statuses", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [STATUS] });
    });

    await page.goto("/v2/concept-statuses");
    await group(page, "Nombre").getByRole("textbox").fill("Calidad");
    await group(page, "Posición").getByRole("textbox").fill("3");
    await page.getByRole("button", { name: /Crear estado|Guardando/ }).click();

    await expect.poll(() => body?.name).toBe("Calidad");
    expect(body.position).toBe(3);
  });
});

test.describe("project detail / concepts (mocked API)", () => {
  test("renders concepts, adds one, advances one", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/concept_statuses", (r) => r.fulfill({ json: [STATUS] }));
    await page.route("**/api/admin/projects/p1", (r) =>
      r.fulfill({
        json: {
          title: "Casa de campo",
          contact_id: null,
          category_id: null,
          description: null,
          priority: "medium",
          total_budget: null,
          scheduled_at: null,
          notes: null,
        },
      }),
    );
    let createBody: any;
    await page.route("**/api/admin/projects/p1/concepts", (route) => {
      if (route.request().method() === "POST") {
        createBody = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({
        json: [
          {
            id: "c1",
            status_id: "st1",
            name: "Cimentación",
            quantity: 1,
            unit: "lote",
            description: null,
            estimated_hours: 40,
            estimated_cost: null,
            position: 0,
          },
        ],
      });
    });
    let advanced = false;
    await page.route("**/api/admin/project_concepts/*/advance", (route) => {
      advanced = true;
      return route.fulfill({ json: { ok: true } });
    });

    await page.goto("/v2/projects/p1");
    await expect(page.getByRole("heading", { name: "Casa de campo" })).toBeVisible();
    await expect(page.getByRole("cell", { name: "Cimentación" })).toBeVisible();

    // Add a concept.
    await group(page, "Concepto").getByRole("textbox").fill("Muros");
    await page.getByRole("button", { name: /Agregar concepto|Guardando/ }).click();
    await expect.poll(() => createBody?.name).toBe("Muros");

    // Advance the existing concept.
    await page
      .getByRole("row", { name: /Cimentación/ })
      .getByRole("button", { name: "Avanzar" })
      .click();
    await expect.poll(() => advanced).toBe(true);
  });
});
