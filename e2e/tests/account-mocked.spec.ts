import { test, expect } from "@playwright/test";

/**
 * Hermetic own-account tests: API mocked with `page.route`, no backend. The
 * account page is available to every role; it prefills email + TOTP secret and
 * saves both back.
 */

const STAFF_ME = {
  email: "staff@example.com",
  company: "Acme",
  company_slug: "acme",
  role: "staff",
  permissions: [],
  companies: [{ id: "c1", name: "Acme", slug: "acme", active: true }],
};

test.describe("account (mocked API)", () => {
  test("staff can open and save their own profile", async ({ page }) => {
    let saved: any = null;

    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.route("**/api/account", (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        saved = JSON.parse(req.postData() || "{}");
        return route.fulfill({ json: { ok: true } });
      }
      return route.fulfill({ json: { id: "u1", email: "staff@example.com" } });
    });

    await page.goto("/v2/");
    // The "Mi cuenta" link is visible to non-admins too.
    await page.getByRole("link", { name: "Mi cuenta" }).click();

    const emailInput = page.locator('input[type="email"]');
    await expect(emailInput).toHaveValue("staff@example.com");

    await emailInput.fill("staff2@example.com");
    await page.getByRole("button", { name: /Guardar cambios|Guardando/ }).click();

    await expect(page.getByText("Tu información se guardó correctamente")).toBeVisible();
    expect(saved.email).toBe("staff2@example.com");
    // Secret left blank -> backend keeps the existing one (never exposed here).
    expect(saved.secret).toBe("");
  });

  test("empty email blocks the save", async ({ page }) => {
    let posted = false;
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.route("**/api/account", (route) => {
      if (route.request().method() === "POST") {
        posted = true;
        return route.fulfill({ json: { ok: true } });
      }
      return route.fulfill({ json: { id: "u1", email: "staff@example.com" } });
    });

    await page.goto("/v2/account");
    const emailInput = page.locator('input[type="email"]');
    await emailInput.fill("");
    await page.getByRole("button", { name: /Guardar cambios/ }).click();

    // Native `required` validation keeps the empty field invalid and blocks the
    // submit — no save fires and no success message appears.
    await expect(emailInput).toHaveJSProperty("validity.valid", false);
    await expect(page.getByText("Tu información se guardó correctamente")).toHaveCount(0);
    expect(posted).toBe(false);
  });
});
