import { test, expect } from "@playwright/test";

/**
 * Hermetic companies-admin tests: API mocked with `page.route`, no backend.
 * Covers list (with the active-company "Actual" badge that blocks delete),
 * create, and delete of a non-active company.
 */

const ADMIN_ME = {
  email: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "c1", name: "Acme", slug: "acme", active: true }],
};

const STAFF_ME = { ...ADMIN_ME, email: "staff@example.com", role: "staff" };

function company(over: Record<string, unknown>) {
  return {
    name: "Acme",
    slug: "acme",
    default_currency: "MXN",
    is_active: true,
    notes: null,
    is_current: false,
    ...over,
  };
}

test.describe("admin / companies (mocked API)", () => {
  test("admin lists companies, current one cannot be deleted", async ({ page }) => {
    const companies = [
      company({ id: "c1", name: "Acme", is_current: true }),
      company({ id: "c2", name: "Beta", default_currency: "USD", is_active: false }),
    ];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: companies }));

    await page.goto("/v2/");
    await page.getByRole("link", { name: "Compañías" }).click();

    await expect(page.getByRole("cell", { name: "Beta" })).toBeVisible();
    // Active company shows the "Actual" badge and offers no delete.
    const acmeRow = page.getByRole("row", { name: /Acme/ });
    await expect(acmeRow.getByText("Actual")).toBeVisible();
    await expect(acmeRow.getByRole("button", { name: "Eliminar" })).toHaveCount(0);
    // The non-active company can be deleted.
    await expect(
      page.getByRole("row", { name: /Beta/ }).getByRole("button", { name: "Eliminar" }),
    ).toBeVisible();
  });

  test("admin can create a company", async ({ page }) => {
    const companies = [company({ id: "c1", name: "Acme", is_current: true })];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        const body = JSON.parse(req.postData() || "{}");
        companies.push(company({ id: "c2", name: body.name, slug: body.slug || "x" }));
        return route.fulfill({ status: 201, json: { id: "c2" } });
      }
      return route.fulfill({ json: companies });
    });

    await page.goto("/v2/");
    await page.getByRole("link", { name: "Compañías" }).click();

    await page.getByPlaceholder("Mi Empresa S.A.").fill("Gamma");
    await page.getByPlaceholder("ej. research").fill("gamma");
    await page.getByRole("button", { name: /Crear compañía|Guardando/ }).click();
    await expect(page.getByRole("cell", { name: "Gamma" })).toBeVisible();
  });

  test("invalid slug is rejected client-side", async ({ page }) => {
    const companies = [company({ id: "c1", name: "Acme", is_current: true })];
    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: companies }));

    await page.goto("/v2/companies");
    await page.getByPlaceholder("Mi Empresa S.A.").fill("Delta");
    await page.getByPlaceholder("ej. research").fill("Bad_Slug");
    await page.getByRole("button", { name: /Crear compañía/ }).click();
    await expect(page.getByText(/Slug inválido/)).toBeVisible();
  });

  test("staff does not see the companies nav link", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.goto("/v2/");
    await expect(page.getByRole("link", { name: "Compañías" })).toHaveCount(0);
  });
});
