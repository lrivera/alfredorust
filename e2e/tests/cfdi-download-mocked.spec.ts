import { test, expect } from "@playwright/test";

/**
 * Hermetic test for the CFDI SAT-download panel: start a download and watch the
 * job list populate (the page polls the jobs endpoint). API mocked, no backend.
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

test.describe("cfdi download (mocked API)", () => {
  test("admin starts a download and sees the job appear", async ({ page }) => {
    let started: any = null;

    await page.route("**/api/me", (r) => r.fulfill({ json: ADMIN_ME }));
    await page.route("**/api/admin/cfdis/data", (r) =>
      r.fulfill({ json: { company_rfcs: [], items: [] } }),
    );
    await page.route("**/api/admin/sat-configs", (r) =>
      r.fulfill({ json: [{ id: "s1", company_id: "c1", rfc: "XAXX010101000", label: "Principal", created_at: "" }] }),
    );
    await page.route("**/api/admin/companies/*/cfdi/download", (route) => {
      started = JSON.parse(route.request().postData() || "{}");
      return route.fulfill({ status: 202, json: { jobs: [{ job_id: "j1", label: "Ene 2026" }] } });
    });
    // Jobs list: empty until a download has been started, then one done job.
    await page.route("**/api/admin/companies/*/cfdi/jobs", (route) =>
      route.fulfill({
        json: started
          ? [
              {
                job_id: "j1",
                label: "Ene 2026",
                chunk_start: "2026-01-01T00:00:00",
                started_at: "2026-06-18",
                status: {
                  status: "done",
                  imported: 3,
                  transactions_created: 2,
                  transactions_updated: 1,
                  transactions_skipped: 0,
                  errors: [],
                },
              },
            ]
          : [],
      }),
    );

    await page.goto("/v2/cfdi");
    await expect(page.getByText("Descargar del SAT")).toBeVisible();

    await page.getByRole("button", { name: /Descargar CFDIs|Descargando/ }).click();

    await expect(page.getByText("Ene 2026")).toBeVisible();
    await expect(page.getByText("✓ Listo")).toBeVisible();
    expect(started.download_type).toBe("both");
    expect(started.sat_config_id).toBe("s1");
  });

  test("download panel hidden for staff", async ({ page }) => {
    await page.route("**/api/me", (r) => r.fulfill({ json: STAFF_ME }));
    await page.route("**/api/admin/cfdis/data", (r) =>
      r.fulfill({ json: { company_rfcs: [], items: [] } }),
    );
    await page.goto("/v2/cfdi");
    await expect(page.getByRole("heading", { name: "CFDIs" })).toBeVisible();
    await expect(page.getByText("Descargar del SAT")).toHaveCount(0);
  });
});
