import { test, expect, type Page } from "@playwright/test";

/**
 * Hermetic tests for the orders and projects screens (API mocked). Cover create
 * (incl. order line items), the Completar (orders) and Avanzar (projects)
 * actions.
 */

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

async function base(page: Page) {
  await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
  await page.route("**/api/admin/categories", (r) => r.fulfill({ json: [] }));
  await page.route("**/api/admin/accounts", (r) => r.fulfill({ json: [] }));
  await page.route("**/api/admin/contacts", (r) => r.fulfill({ json: [] }));
}

test.describe("orders (mocked API)", () => {
  test("create with a line item", async ({ page }) => {
    await base(page);
    let body: any;
    await page.route("**/api/admin/orders", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/orders");
    await group(page, "Título").getByRole("textbox").fill("Servicio A");
    await group(page, "Monto total").getByRole("textbox").fill("500");
    await page.getByRole("button", { name: "+ Agregar línea" }).click();
    await page.getByPlaceholder("Descripción").fill("Mano de obra");
    await page.getByPlaceholder("Cant.").fill("2");
    await page.getByPlaceholder("P. unit.").fill("250");
    await page.getByRole("button", { name: /Crear orden|Guardando/ }).click();

    await expect.poll(() => body?.title).toBe("Servicio A");
    expect(body.amount).toBe(500);
    expect(body.items).toEqual([{ description: "Mano de obra", quantity: 2, unit_price: 250 }]);
  });

  test("complete action", async ({ page }) => {
    await base(page);
    let completed = false;
    await page.route("**/api/admin/orders/*/complete", (route) => {
      completed = true;
      return route.fulfill({ json: { ok: true } });
    });
    await page.route("**/api/admin/orders", (route) =>
      route.fulfill({
        json: [
          { id: "o1", title: "Orden 1", status: "pending", status_label: "Pendiente", amount: 100 },
        ],
      }),
    );

    await page.goto("/v2/orders");
    await page.getByRole("row", { name: /Orden 1/ }).getByRole("button", { name: "Completar" }).click();
    await expect.poll(() => completed).toBe(true);
  });
});

test.describe("projects (mocked API)", () => {
  test("create with priority + budget", async ({ page }) => {
    await base(page);
    let body: any;
    await page.route("**/api/admin/projects", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/projects");
    await group(page, "Título").getByRole("textbox").fill("Proyecto X");
    await group(page, "Prioridad").getByRole("combobox").selectOption("high");
    await group(page, "Presupuesto (opcional)").getByRole("textbox").fill("10000");
    await page.getByRole("button", { name: /Crear proyecto|Guardando/ }).click();

    await expect.poll(() => body?.title).toBe("Proyecto X");
    expect(body.priority).toBe("high");
    expect(body.total_budget).toBe(10000);
  });

  test("advance action", async ({ page }) => {
    await base(page);
    let advanced = false;
    await page.route("**/api/admin/projects/*/advance", (route) => {
      advanced = true;
      return route.fulfill({ json: { ok: true } });
    });
    await page.route("**/api/admin/projects", (route) =>
      route.fulfill({
        json: [
          {
            id: "pr1",
            title: "Proj1",
            status: "pedidos",
            status_label: "Pedidos",
            priority: "medium",
            priority_label: "Media",
          },
        ],
      }),
    );

    await page.goto("/v2/projects");
    await page.getByRole("row", { name: /Proj1/ }).getByRole("button", { name: "Avanzar" }).click();
    await expect.poll(() => advanced).toBe(true);
  });
});
