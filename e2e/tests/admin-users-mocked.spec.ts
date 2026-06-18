import { test, expect } from "@playwright/test";

/**
 * Hermetic users-admin tests: API mocked with `page.route`, no backend. Covers
 * the list (with self-delete suppression), and creating a user with a per-company
 * membership through the compound form.
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

const COMPANIES = [
  {
    id: "c1",
    name: "Acme",
    slug: "acme",
    default_currency: "MXN",
    is_active: true,
    notes: null,
    is_current: true,
  },
];

test.describe("admin / users (mocked API)", () => {
  test("admin lists users; own row has no delete", async ({ page }) => {
    const users = [
      {
        id: "u1",
        username: "admin@example.com",
        role: "admin",
        companies: ["Acme"],
        memberships: [
          { company_id: "c1", company_name: "Acme", role: "admin", permissions: [] },
        ],
      },
      {
        id: "u2",
        username: "worker@example.com",
        role: "staff",
        companies: ["Acme"],
        memberships: [
          { company_id: "c1", company_name: "Acme", role: "staff", permissions: ["view_projects"] },
        ],
      },
    ];

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: COMPANIES }));
    await page.route("**/api/admin/users", (r) => r.fulfill({ json: users }));

    await page.goto("/v2/users");
    await expect(page.getByRole("cell", { name: "worker@example.com" })).toBeVisible();

    // Own user (admin@example.com) cannot be deleted.
    await expect(
      page.getByRole("row", { name: /admin@example.com/ }).getByRole("button", { name: "Eliminar" }),
    ).toHaveCount(0);
    // Another user can be deleted.
    await expect(
      page.getByRole("row", { name: /worker@example.com/ }).getByRole("button", { name: "Eliminar" }),
    ).toBeVisible();
  });

  test("admin can create a user with a company membership", async ({ page }) => {
    const users: any[] = [];
    let lastPost: any = null;

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: COMPANIES }));
    await page.route("**/api/admin/users", (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        lastPost = JSON.parse(req.postData() || "{}");
        users.push({
          id: "u9",
          username: lastPost.username,
          role: "staff",
          companies: ["Acme"],
          memberships: lastPost.memberships,
        });
        return route.fulfill({ status: 201, json: { id: "u9" } });
      }
      return route.fulfill({ json: users });
    });

    await page.goto("/v2/users");
    const form = page.locator("form");
    await form.getByRole("textbox").first().fill("new@example.com");
    await form.getByRole("checkbox", { name: "Acme" }).check();
    await page.getByRole("button", { name: /Crear usuario|Guardando/ }).click();

    await expect(page.getByRole("cell", { name: "new@example.com" })).toBeVisible();
    expect(lastPost.username).toBe("new@example.com");
    expect(lastPost.memberships).toHaveLength(1);
    expect(lastPost.memberships[0].company_id).toBe("c1");
  });

  test("editing a user shows the TOTP QR code", async ({ page }) => {
    const users = [
      {
        id: "u2",
        username: "worker@example.com",
        role: "staff",
        companies: ["Acme"],
        memberships: [
          { company_id: "c1", company_name: "Acme", role: "staff", permissions: [] },
        ],
      },
    ];
    // 1x1 transparent PNG.
    const png = Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
      "base64",
    );

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: COMPANIES }));
    await page.route("**/admin/users/*/qrcode", (r) =>
      r.fulfill({ contentType: "image/png", body: png }),
    );
    await page.route("**/api/admin/users", (r) => r.fulfill({ json: users }));
    // Detail GET wins for the single-segment path (registered last). The detail
    // endpoint also returns the secret for copying.
    await page.route("**/api/admin/users/*", (r) =>
      r.fulfill({ json: { ...users[0], secret: "JBSWY3DPEHPK3PXP" } }),
    );

    await page.goto("/v2/users");
    await page
      .getByRole("row", { name: /worker@example.com/ })
      .getByRole("button", { name: "Editar" })
      .click();
    await expect(page.getByText("Editar usuario")).toBeVisible();

    const qr = page.getByRole("img", { name: "Código QR TOTP" });
    await expect(qr).toBeVisible();
    await expect(qr).toHaveAttribute("src", "/admin/users/u2/qrcode");
    // The secret is shown as copyable text under the QR.
    await expect(page.getByText("JBSWY3DPEHPK3PXP")).toBeVisible();
  });

  test("duplicate username is reported", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/companies", (r) => r.fulfill({ json: COMPANIES }));
    await page.route("**/api/admin/users", (route) => {
      if (route.request().method() === "POST") {
        return route.fulfill({ status: 409, json: { error: "El nombre de usuario ya existe" } });
      }
      return route.fulfill({ json: [] });
    });

    await page.goto("/v2/users");
    await expect(page.getByText("Nombre de usuario")).toBeVisible();
    const form = page.locator("form");
    await form.getByRole("textbox").first().fill("alfredo@example.com");
    await form.getByRole("checkbox", { name: "Acme" }).check();
    await page.getByRole("button", { name: /Crear usuario|Guardando/ }).click();
    await expect(page.getByText("El nombre de usuario ya existe")).toBeVisible();
  });

  test("staff does not see the users nav link", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.goto("/v2/");
    await expect(page.getByRole("link", { name: "Usuarios" })).toHaveCount(0);
  });
});
