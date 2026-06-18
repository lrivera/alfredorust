import { test, expect } from "@playwright/test";

/**
 * Hermetic SAT-config tests: API mocked with `page.route`, no backend. Covers
 * the list, the multipart certificate upload (.cer + .key + password), and
 * delete. The upload route just asserts it was hit and echoes a new row.
 */

const ADMIN_ME = {
  username: "admin@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "admin",
  permissions: [],
  companies: [{ id: "c1", name: "Acme", slug: "acme", active: true }],
};

const STAFF_ME = { ...ADMIN_ME, username: "staff@example.com", role: "staff" };

function satConfig(over: Record<string, unknown>) {
  return {
    id: "s1",
    company_id: "c1",
    rfc: "XAXX010101000",
    label: "Principal",
    created_at: "2026-01-02T00:00:00Z",
    ...over,
  };
}

test.describe("admin / sat configs (mocked API)", () => {
  test("admin lists and deletes a SAT config", async ({ page }) => {
    let configs = [satConfig({ id: "s1", rfc: "XAXX010101000" })];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/sat-configs", (r) => r.fulfill({ json: configs }));
    await page.route("**/api/admin/sat-configs/*/delete", (route) => {
      const parts = route.request().url().split("/");
      const id = parts[parts.length - 2];
      configs = configs.filter((c) => c.id !== id);
      return route.fulfill({ json: { ok: true } });
    });

    await page.goto("/v2/sat-configs");
    await expect(page.getByRole("cell", { name: "XAXX010101000" })).toBeVisible();

    await page
      .getByRole("row", { name: /XAXX010101000/ })
      .getByRole("button", { name: "Eliminar" })
      .click();
    await expect(page.getByText("XAXX010101000")).toHaveCount(0);
  });

  test("admin can upload a new SAT config (multipart)", async ({ page }) => {
    const configs: any[] = [];
    let uploadHit = false;

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/sat-configs/upload", (route) => {
      uploadHit = true;
      configs.push(satConfig({ id: "s2", rfc: "AAA010101AAA", label: "Nueva" }));
      return route.fulfill({ status: 201, json: { id: "s2" } });
    });
    await page.route("**/api/admin/sat-configs", (r) => r.fulfill({ json: configs }));

    await page.goto("/v2/sat-configs");

    await page.getByPlaceholder("XAXX010101000").fill("AAA010101AAA");
    await page
      .locator('input[accept=".cer"]')
      .setInputFiles({ name: "cert.cer", mimeType: "application/octet-stream", buffer: Buffer.from("cer-bytes") });
    await page
      .locator('input[accept=".key"]')
      .setInputFiles({ name: "private.key", mimeType: "application/octet-stream", buffer: Buffer.from("key-bytes") });
    await page.locator('input[type="password"]').fill("s3cret");
    await page.getByRole("button", { name: /Guardar configuración|Subiendo/ }).click();

    await expect(page.getByRole("cell", { name: "AAA010101AAA" })).toBeVisible();
    expect(uploadHit).toBe(true);
  });

  test("missing files blocks upload with a message", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/sat-configs", (r) => r.fulfill({ json: [] }));

    await page.goto("/v2/sat-configs");
    await page.getByPlaceholder("XAXX010101000").fill("AAA010101AAA");
    await page.locator('input[type="password"]').fill("s3cret");
    await page.getByRole("button", { name: /Guardar configuración/ }).click();
    await expect(page.getByText(/Selecciona los archivos/)).toBeVisible();
  });

  test("staff does not see the SAT-config nav link", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.goto("/v2/");
    await expect(page.getByRole("link", { name: "Config. SAT" })).toHaveCount(0);
  });
});
