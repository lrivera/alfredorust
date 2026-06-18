import { test, expect } from "@playwright/test";

/**
 * Live smoke (E2E_SMOKE=1) on the **test** tenant: creates a throwaway company
 * "test2", exercises the danger-zone bulk-delete endpoints on it, then deletes
 * the company. Self-cleaning so it can re-run; reuses test2 if it already
 * exists. Uses the shared session via the API (no UI), so it's robust.
 */
test.describe("company danger zone (live · test tenant · self-cleaning)", () => {
  test("create test2, clear its data, then delete it", async ({ request }) => {
    // Find or create the throwaway company.
    const list = await request.get("/api/admin/companies");
    expect(list.ok(), await list.text()).toBeTruthy();
    const companies = await list.json();
    let id: string | undefined = companies.find((c: any) => c.slug === "test2")?.id;

    if (!id) {
      const created = await request.post("/api/admin/companies", {
        data: { name: "test2", slug: "test2", default_currency: "MXN", is_active: true },
      });
      expect(created.status(), await created.text()).toBe(201);
      id = (await created.json()).id;
    }
    expect(id).toBeTruthy();

    // Danger zone: delete all CFDIs (returns the count).
    const cfdis = await request.post(`/api/admin/companies/${id}/cfdis/delete_all`);
    expect(cfdis.ok(), await cfdis.text()).toBeTruthy();
    const cfdisBody = await cfdis.json();
    expect(cfdisBody.ok).toBe(true);
    expect(typeof cfdisBody.deleted).toBe("number");

    // Danger zone: delete all transactions.
    const txs = await request.post(`/api/admin/companies/${id}/transactions/delete_all`);
    expect(txs.ok(), await txs.text()).toBeTruthy();
    const txsBody = await txs.json();
    expect(txsBody.ok).toBe(true);
    expect(typeof txsBody.deleted).toBe("number");

    // Clean up: delete test2 (it isn't the session's active company).
    const del = await request.post(`/api/admin/companies/${id}/delete`);
    expect(del.ok(), await del.text()).toBeTruthy();
    expect((await del.json()).ok).toBe(true);
  });
});
