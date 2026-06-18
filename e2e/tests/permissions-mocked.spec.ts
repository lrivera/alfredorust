import { test, expect, type Page } from "@playwright/test";

/**
 * Hermetic checks that the SPA gates visibility by staff permissions, mirroring
 * v1: `view_timeline` -> Tiempo, `view_projects` -> Proyectos, resource-usage
 * perms -> the grid, `view_project_money` -> the budget column. Admins see all.
 */

function meWith(role: string, permissions: string[]) {
  return {
    username: role === "admin" ? "admin@example.com" : "staff@example.com",
    company: "Acme",
    company_slug: "acme",
    role,
    permissions,
    companies: [{ id: "c1", name: "Acme", slug: "acme", active: true }],
  };
}

async function gotoAs(page: Page, role: string, permissions: string[]) {
  await page.route("**/api/me", (r) => r.fulfill({ json: meWith(role, permissions) }));
  await page.goto("/v2/");
}

const sidebarLink = (page: Page, name: string) =>
  page.locator("aside").getByRole("link", { name, exact: true });

test.describe("permission-gated navigation (mocked API)", () => {
  test("staff with no permissions sees only base items", async ({ page }) => {
    await gotoAs(page, "staff", []);
    await expect(sidebarLink(page, "Inicio")).toBeVisible();
    await expect(sidebarLink(page, "Mi cuenta")).toBeVisible();
    await expect(sidebarLink(page, "Tiempo")).toHaveCount(0);
    await expect(sidebarLink(page, "Proyectos")).toHaveCount(0);
    await expect(sidebarLink(page, "Uso de recursos (grid)")).toHaveCount(0);
    // No admin sections.
    await expect(sidebarLink(page, "Cuentas")).toHaveCount(0);
    await expect(sidebarLink(page, "Compañías")).toHaveCount(0);
    await expect(sidebarLink(page, "Usuarios")).toHaveCount(0);
  });

  test("view_timeline reveals Tiempo only", async ({ page }) => {
    await gotoAs(page, "staff", ["view_timeline"]);
    await expect(sidebarLink(page, "Tiempo")).toBeVisible();
    await expect(sidebarLink(page, "Proyectos")).toHaveCount(0);
    await expect(sidebarLink(page, "Uso de recursos (grid)")).toHaveCount(0);
  });

  test("view_projects reveals Proyectos only", async ({ page }) => {
    await gotoAs(page, "staff", ["view_projects"]);
    await expect(sidebarLink(page, "Proyectos")).toBeVisible();
    await expect(sidebarLink(page, "Tiempo")).toHaveCount(0);
    await expect(sidebarLink(page, "Uso de recursos (grid)")).toHaveCount(0);
  });

  test("resource-usage permission reveals the grid", async ({ page }) => {
    await gotoAs(page, "staff", ["view_resource_usage_history"]);
    await expect(sidebarLink(page, "Uso de recursos (grid)")).toBeVisible();
    await expect(sidebarLink(page, "Proyectos")).toHaveCount(0);
    // edit-today permission unlocks the same link.
    await gotoAs(page, "staff", ["edit_resource_usage_today"]);
    await expect(sidebarLink(page, "Uso de recursos (grid)")).toBeVisible();
  });

  test("admin sees the full menu", async ({ page }) => {
    await gotoAs(page, "admin", []);
    for (const name of ["Tiempo", "Proyectos", "Cuentas", "Compañías", "Usuarios", "CFDIs"]) {
      await expect(sidebarLink(page, name)).toBeVisible();
    }
  });

  test("view_project_money toggles the budget column", async ({ page }) => {
    const projects = [
      {
        id: "p1",
        title: "Obra",
        status: "active",
        status_label: "Activo",
        priority: "high",
        priority_label: "Alta",
        total_budget: null,
        scheduled_at: null,
        contact_id: null,
      },
    ];
    await page.route("**/api/admin/categories", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/contacts", (r) => r.fulfill({ json: [] }));
    await page.route("**/api/admin/projects", (r) => r.fulfill({ json: projects }));

    // Staff with view_projects but NOT view_project_money: no budget column.
    await page.route("**/api/me", (r) => r.fulfill({ json: meWith("staff", ["view_projects"]) }));
    await page.goto("/v2/projects");
    await expect(page.getByRole("cell", { name: "Obra" })).toBeVisible();
    await expect(page.getByRole("columnheader", { name: "Presupuesto" })).toHaveCount(0);

    // Add view_project_money: the budget column appears.
    await page.unroute("**/api/me");
    await page.route("**/api/me", (r) =>
      r.fulfill({ json: meWith("staff", ["view_projects", "view_project_money"]) }),
    );
    await page.goto("/v2/projects");
    await expect(page.getByRole("columnheader", { name: "Presupuesto" })).toBeVisible();
  });
});
