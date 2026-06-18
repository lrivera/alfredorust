import { test, expect, type Page } from "@playwright/test";

/**
 * One test per new finance UI field/flow added in the v1-parity pass. Each test
 * mocks the API, fills the new control(s), submits, and asserts the captured
 * request body carries the new value(s). Hermetic — no backend.
 */

const ADMIN_ME = {
  email: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "1", name: "Acme", slug: "acme", active: true }],
};

// A field group renders <div><label>X</label><input|select/></div>.
const group = (page: Page, label: string) =>
  page.locator(`div:has(> label:text-is("${label}"))`);

async function me(page: Page) {
  await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
}

// Common option sources for relational pages.
async function options(page: Page) {
  await page.route("**/api/admin/categories", (route) => {
    if (route.request().method() === "GET")
      return route.fulfill({ json: [{ id: "c1", name: "Cat1", flow_type: "expense", parent: "" }] });
    return route.fallback();
  });
  await page.route("**/api/admin/accounts", (route) => {
    if (route.request().method() === "GET")
      return route.fulfill({
        json: [{ id: "a1", name: "Banco", company: "Acme", account_type: "bank", currency: "MXN", is_active: true }],
      });
    return route.fallback();
  });
  await page.route("**/api/admin/contacts", (route) => {
    if (route.request().method() === "GET")
      return route.fulfill({ json: [{ id: "ct1", name: "Cliente1", kind: "customer", email: "" }] });
    return route.fallback();
  });
  await page.route("**/api/admin/projects", (route) =>
    route.fulfill({ json: [{ id: "pr1", title: "Proyecto 1" }] }),
  );
  await page.route("**/api/admin/planned-entries", (route) => {
    if (route.request().method() === "GET")
      return route.fulfill({
        json: [
          {
            id: "pe1",
            name: "Compromiso 1",
            flow_type: "expense",
            amount_estimated: 0,
            due_date: "2026-01-01T00:00:00Z",
            status: "planned",
            status_label: "Planificado",
          },
        ],
      });
    return route.fallback();
  });
}

test.describe("finance new fields (mocked API)", () => {
  test("accounts: form sends notes + is_active", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/accounts", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/accounts");
    await page.getByPlaceholder("Cuenta principal").fill("Caja");
    await group(page, "Notas").getByRole("textbox").fill("efectivo");
    await page.getByRole("checkbox", { name: "Activa" }).uncheck();
    await page.getByRole("button", { name: /Crear cuenta|Guardando/ }).click();

    await expect.poll(() => body?.notes).toBe("efectivo");
    expect(body.is_active).toBe(false);
  });

  test("categories: form sends parent_id", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/categories", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [{ id: "c1", name: "Cat1", flow_type: "expense", parent: "" }] });
    });

    await page.goto("/v2/categories");
    await page.getByPlaceholder("Servicios").fill("Sub");
    await group(page, "Categoría padre (opcional)").getByRole("combobox").selectOption("c1");
    await page.getByRole("button", { name: /Crear categoría|Guardando/ }).click();

    await expect.poll(() => body?.parent_id).toBe("c1");
  });

  test("contacts: form sends rfc, phone, notes", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/contacts", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/contacts");
    await page.getByPlaceholder("ACME S.A.").fill("Proveedor");
    await group(page, "RFC").getByRole("textbox").fill("XAXX010101000");
    await group(page, "Teléfono").getByRole("textbox").fill("555-1234");
    await group(page, "Notas").getByRole("textbox").fill("nota");
    await page.getByRole("button", { name: /Crear contacto|Guardando/ }).click();

    await expect.poll(() => body?.rfc).toBe("XAXX010101000");
    expect(body.phone).toBe("555-1234");
    expect(body.notes).toBe("nota");
  });

  test("forecasts: form sends balances + details", async ({ page }) => {
    await me(page);
    let body: any;
    await page.route("**/api/admin/forecasts", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/forecasts");
    await group(page, "Desde").getByRole("textbox").fill("2026-01-01");
    await group(page, "Hasta").getByRole("textbox").fill("2026-12-31");
    await group(page, "Saldo inicial (opcional)").getByRole("textbox").fill("1000");
    await group(page, "Saldo final (opcional)").getByRole("textbox").fill("1500");
    await group(page, "Detalles (opcional)").getByRole("textbox").fill("plan base");
    await page.getByRole("button", { name: /Crear pronóstico|Guardando/ }).click();

    await expect.poll(() => body?.initial_balance).toBe(1000);
    expect(body.final_balance).toBe(1500);
    expect(body.details).toBe("plan base");
  });

  test("recurring plans: form sends contact, end_date, day_of_month, notes, is_active", async ({
    page,
  }) => {
    await me(page);
    await options(page);
    let body: any;
    await page.route("**/api/admin/recurring-plans", (route) => {
      if (route.request().method() === "POST") {
        body = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/recurring-plans");
    await group(page, "Nombre").getByRole("textbox").fill("Plan");
    await group(page, "Categoría").getByRole("combobox").selectOption("c1");
    await group(page, "Cuenta esperada").getByRole("combobox").selectOption("a1");
    await group(page, "Inicio").getByRole("textbox").fill("2026-01-01");
    await group(page, "Término (opcional)").getByRole("textbox").fill("2026-06-01");
    await group(page, "Día del mes (opcional)").getByRole("textbox").fill("15");
    await group(page, "Contacto (opcional)").getByRole("combobox").selectOption("ct1");
    await group(page, "Notas").getByRole("textbox").fill("mensual");
    await page.getByRole("checkbox", { name: "Activo" }).uncheck();
    await page.getByRole("button", { name: /Crear plan|Guardando/ }).click();

    await expect.poll(() => body?.contact_id).toBe("ct1");
    expect(body.day_of_month).toBe(15);
    expect(body.end_date).toContain("2026-06-01");
    expect(body.notes).toBe("mensual");
    expect(body.is_active).toBe(false);
  });

  test("transactions: form sends notes + is_confirmed", async ({ page }) => {
    await me(page);
    await options(page);
    let body: any;
    await page.route("**/api/admin/transactions", (route) => {
      body = JSON.parse(route.request().postData() || "{}");
      return route.fulfill({ status: 201, json: { id: "x" } });
    });
    await page.route("**/api/admin/transactions/data", (route) => route.fulfill({ json: [] }));

    await page.goto("/v2/transactions");
    await group(page, "Fecha").getByRole("textbox").fill("2026-01-01");
    await group(page, "Descripción").getByRole("textbox").fill("Pago");
    await group(page, "Categoría").getByRole("combobox").selectOption("c1");
    await group(page, "Compromiso ligado (opcional)").getByRole("combobox").selectOption("pe1");
    await group(page, "Notas").getByRole("textbox").fill("ref-1");
    await page.getByRole("checkbox", { name: "Confirmado" }).uncheck();
    await page.getByRole("button", { name: /Crear movimiento|Guardando/ }).click();

    await expect.poll(() => body?.notes).toBe("ref-1");
    expect(body.is_confirmed).toBe(false);
    expect(body.planned_entry_id).toBe("pe1");
  });

  test("planned entries: bulk pay selected rows", async ({ page }) => {
    await me(page);
    await options(page);
    let bulkBody: any;
    await page.route("**/api/admin/planned-entries/bulk-pay", (route) => {
      bulkBody = JSON.parse(route.request().postData() || "{}");
      return route.fulfill({ json: { ok: true } });
    });
    // Override the list with two payable rows (wins over options()).
    await page.route("**/api/admin/planned-entries", (route) => {
      if (route.request().method() === "GET")
        return route.fulfill({
          json: [
            { id: "p1", name: "Renta", flow_type: "expense", amount_estimated: 100, due_date: "2026-01-01T00:00:00Z", status: "planned", status_label: "Planificado" },
            { id: "p2", name: "Luz", flow_type: "expense", amount_estimated: 50, due_date: "2026-01-02T00:00:00Z", status: "planned", status_label: "Planificado" },
          ],
        });
      return route.fallback();
    });

    await page.goto("/v2/planned-entries");
    // Select both rows.
    await page.getByRole("row", { name: /Renta/ }).getByRole("checkbox").check();
    await page.getByRole("row", { name: /Luz/ }).getByRole("checkbox").check();
    await page.getByRole("button", { name: "Pagar seleccionadas" }).click();
    await group(page, "Fecha de pago").getByRole("textbox").fill("2026-01-20");
    await group(page, "Cuenta").getByRole("combobox").selectOption("a1");
    await page.getByRole("button", { name: /Confirmar pago|Pagando/ }).click();

    await expect.poll(() => bulkBody?.entry_ids).toEqual(["p1", "p2"]);
    expect(bulkBody.account_id).toBe("a1");
    expect(bulkBody.paid_at).toContain("2026-01-20");
  });

  test("planned entries: contact + notes, and the Pagar flow", async ({ page }) => {
    await me(page);
    await options(page);
    let createBody: any;
    let payBody: any;
    await page.route("**/api/admin/planned-entries/*/pay", (route) => {
      payBody = JSON.parse(route.request().postData() || "{}");
      return route.fulfill({ json: { ok: true } });
    });
    await page.route("**/api/admin/planned-entries", (route) => {
      if (route.request().method() === "POST") {
        createBody = JSON.parse(route.request().postData() || "{}");
        return route.fulfill({ status: 201, json: { id: "x" } });
      }
      return route.fulfill({
        json: [
          {
            id: "p1",
            name: "Renta",
            flow_type: "expense",
            amount_estimated: 100,
            due_date: "2026-01-01T00:00:00Z",
            status: "planned",
            status_label: "Planificado",
          },
        ],
      });
    });

    await page.goto("/v2/planned-entries");

    // New create fields.
    await group(page, "Nombre").getByRole("textbox").fill("Servicio");
    await group(page, "Categoría").getByRole("combobox").selectOption("c1");
    await group(page, "Cuenta esperada").getByRole("combobox").selectOption("a1");
    await group(page, "Vencimiento").getByRole("textbox").fill("2026-02-01");
    await group(page, "Contacto (opcional)").getByRole("combobox").selectOption("ct1");
    await group(page, "Proyecto (opcional)").getByRole("combobox").selectOption("pr1");
    await group(page, "Notas").getByRole("textbox").fill("contrato");
    await page.getByRole("button", { name: /Crear entrada|Guardando/ }).click();
    await expect.poll(() => createBody?.contact_id).toBe("ct1");
    expect(createBody.project_id).toBe("pr1");
    expect(createBody.notes).toBe("contrato");

    // Pay flow on the seeded row.
    await page.getByRole("row", { name: /Renta/ }).getByRole("button", { name: "Pagar" }).click();
    await group(page, "Fecha de pago").getByRole("textbox").fill("2026-01-15");
    await group(page, "Cuenta").getByRole("combobox").selectOption("a1");
    await group(page, "Monto real pagado").getByRole("textbox").fill("90");
    await page.getByRole("button", { name: /Confirmar pago|Pagando/ }).click();

    await expect.poll(() => payBody?.account_id).toBe("a1");
    expect(payBody.amount).toBe(90);
    expect(payBody.paid_at).toContain("2026-01-15");
  });
});
