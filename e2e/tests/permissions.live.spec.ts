import { test, expect } from "@playwright/test";
import { authenticator } from "otplib";

/**
 * Live (E2E_SMOKE): exercises the real staff-permission gating end to end.
 *  1. Admin (shared session) creates a throwaway company "test2".
 *  2. Admin creates a STAFF user on test2 with a known TOTP secret and a
 *     partial permission set (view_projects + view_timeline only).
 *  3. That staff user logs in (real TOTP) on the test2 tenant and we assert the
 *     SPA shows exactly what those permissions unlock and hides the rest.
 *  4. Cleanup: delete the staff user and the company.
 *
 * The known secret is a throwaway — once the user is deleted it's worthless.
 * The exhaustive per-permission matrix lives in permissions-mocked.spec.ts;
 * this proves the wiring against the real backend with a real login.
 */

// app.alfredorivera.dev -> alfredorivera.dev ; build the test2 tenant host.
const root = (process.env.SPCLI_BASE_URL ?? "https://app.alfredorivera.dev").replace(
  /^https?:\/\/[^.]+\./,
  "",
);
const test2Host = `https://test2.${root}`;

const STAFF_EMAIL = "test2-staff@example.com";
const STAFF_SECRET = "JBSWY3DPEHPK3PXP"; // throwaway base32; deleted with the user

test.describe("staff permissions (live · test tenant · self-cleaning)", () => {
  test("a staff user sees only what its permissions allow", async ({ request, browser }) => {
    // 1. Find or create company test2 (admin is admin of the test tenant).
    const list = await request.get("/api/admin/companies");
    expect(list.ok(), await list.text()).toBeTruthy();
    let test2Id: string | undefined = (await list.json()).find(
      (c: any) => c.slug === "test2",
    )?.id;
    if (!test2Id) {
      const created = await request.post("/api/admin/companies", {
        data: { name: "test2", slug: "test2", default_currency: "MXN", is_active: true },
      });
      expect(created.status(), await created.text()).toBe(201);
      test2Id = (await created.json()).id;
    }

    // 2. Create (or reset) the staff user on the test2 tenant, where the admin
    //    is now admin. Permissions: view_projects + view_timeline only.
    const membership = {
      company_id: test2Id,
      role: "staff",
      permissions: ["view_projects", "view_timeline"],
    };
    const usersRes = await request.get(`${test2Host}/api/admin/users`);
    expect(usersRes.ok(), await usersRes.text()).toBeTruthy();
    let userId: string | undefined = (await usersRes.json()).find(
      (u: any) => u.email === STAFF_EMAIL,
    )?.id;
    if (userId) {
      const upd = await request.post(`${test2Host}/api/admin/users/${userId}/update`, {
        data: { email: STAFF_EMAIL, secret: STAFF_SECRET, memberships: [membership] },
      });
      expect(upd.ok(), await upd.text()).toBeTruthy();
    } else {
      const create = await request.post(`${test2Host}/api/admin/users`, {
        data: { email: STAFF_EMAIL, secret: STAFF_SECRET, memberships: [membership] },
      });
      expect(create.status(), await create.text()).toBe(201);
      userId = (await create.json()).id;
    }
    expect(userId).toBeTruthy();

    // 3. Log in as the staff user in a clean context on the test2 host.
    const ctx = await browser.newContext();
    try {
      const page = await ctx.newPage();
      await page.goto(`${test2Host}/v2/`);
      await page.getByPlaceholder("tu@correo.com").fill(STAFF_EMAIL);
      await page
        .locator('input[inputmode="numeric"]')
        .fill(authenticator.generate(STAFF_SECRET));
      await page.getByRole("button", { name: "Entrar" }).click();
      await expect(page.getByRole("button", { name: "Salir" })).toBeVisible();

      const link = (name: string) =>
        page.locator("aside").getByRole("link", { name, exact: true });

      // Granted -> visible.
      await expect(link("Proyectos")).toBeVisible();
      await expect(link("Tiempo")).toBeVisible();
      // Not granted -> hidden.
      await expect(link("Uso de recursos (grid)")).toHaveCount(0);
      // Admin-only sections are never shown to staff.
      await expect(link("Cuentas")).toHaveCount(0);
      await expect(link("Compañías")).toHaveCount(0);
      await expect(link("Usuarios")).toHaveCount(0);
      await expect(link("CFDIs")).toHaveCount(0);
    } finally {
      await ctx.close();
    }

    // 4. Cleanup: delete the staff user, then the company.
    const delUser = await request.post(`${test2Host}/api/admin/users/${userId}/delete`);
    expect(delUser.ok(), await delUser.text()).toBeTruthy();
    const delCo = await request.post(`/api/admin/companies/${test2Id}/delete`);
    expect(delCo.ok(), await delCo.text()).toBeTruthy();
  });
});
