#[path = "common/mod.rs"]
mod common;

use alfredodev::state::{
    list_accounts, list_categories, list_contacts, list_forecasts, list_planned_entries,
    list_recurring_plans, list_transactions,
};

#[tokio::test]
async fn seed_populates_finance_collections() {
    let ctx = common::setup_state().await;
    if ctx.is_none() {
        return;
    }
    let ctx = ctx.unwrap();
    let state = ctx.state.clone();

    // Seed should populate all finance collections
    let accounts = list_accounts(&state).await.unwrap();
    let categories = list_categories(&state).await.unwrap();
    let contacts = list_contacts(&state).await.unwrap();
    let plans = list_recurring_plans(&state).await.unwrap();
    let planned_entries = list_planned_entries(&state).await.unwrap();
    let txs = list_transactions(&state).await.unwrap();
    let forecasts = list_forecasts(&state).await.unwrap();

    assert!(!accounts.is_empty(), "accounts seeded");
    assert!(!categories.is_empty(), "categories seeded");
    assert!(!contacts.is_empty(), "contacts seeded");
    assert!(!plans.is_empty(), "recurring plans seeded");
    assert!(!planned_entries.is_empty(), "planned entries seeded");
    assert!(!txs.is_empty(), "transactions seeded");
    assert!(!forecasts.is_empty(), "forecasts seeded");

    common::teardown(Some(ctx)).await;
}
