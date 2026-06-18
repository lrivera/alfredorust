import { test, expect, type Page } from "@playwright/test";

/** Hermetic tests for the resources and resource-logs screens (API mocked). */

const ADMIN_ME = {
  email: "admin@example.com",
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

test.describe("resources (mocked API)", () => {
  test("create with type, cost and allowed status", async ({ page }) => {
    await me(page);
    await page.route("**/api/admin/concept_statuses", (r) =>
      r.fulfill({ json: [{ id: "s1", name: "En proceso" }] }),
    );
    let body: any;
    await page.route("**/api/admin/resources", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/resources");
    await group(page, "Nombre").getByRole("textbox").fill("Grúa");
    await group(page, "Tipo").getByRole("combobox").selectOption("vehicle");
    await group(page, "Costo por hora").getByRole("textbox").fill("120");
    await page.getByRole("checkbox", { name: "En proceso" }).check();
    await page.getByRole("button", { name: /Crear recurso|Guardando/ }).click();

    await expect.poll(() => body?.resource_type).toBe("vehicle");
    expect(body.hourly_cost).toBe(120);
    expect(body.allowed_status_ids).toEqual(["s1"]);
  });
});

test.describe("resource logs (mocked API)", () => {
  async function options(page: Page) {
    await page.route("**/api/admin/projects", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/resources", (r) => r.fulfill({ json: [] }));
  }

  test("create stamps an RFC3339 start", async ({ page }) => {
    await me(page);
    await options(page);
    let body: any;
    await page.route("**/api/admin/resource_logs", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/resource-logs");
    await group(page, "Inicio").locator("input").fill("2026-01-01T10:00");
    await group(page, "Operador").getByRole("textbox").fill("Juan");
    await page.getByRole("button", { name: /Crear registro|Guardando/ }).click();

    await expect.poll(() => body?.started_at).toBe("2026-01-01T10:00:00Z");
    expect(body.operator_name).toBe("Juan");
  });

  test("end action stamps an open log", async ({ page }) => {
    await me(page);
    await options(page);
    let ended = false;
    await page.route("**/api/admin/resource_logs/*/end", (route) => {
      ended = true;
      return route.fulfill({ json: { ok: true } });
    });
    await page.route("**/api/admin/resource_logs", (route) =>
      route.fulfill({
        json: [
          {
            id: "l1",
            phase: "Fase A",
            resource_name: "Grúa",
            started_at: "2026-01-01T10:00:00Z",
            ended_at: null,
            duration_hours: null,
            operator_name: "Juan",
          },
        ],
      }),
    );

    await page.goto("/v2/resource-logs");
    await page.getByRole("row", { name: /Grúa/ }).getByRole("button", { name: "Terminar" }).click();
    await expect.poll(() => ended).toBe(true);
  });
});
