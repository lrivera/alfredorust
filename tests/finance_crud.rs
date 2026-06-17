use std::time::SystemTime;

use alfredodev::models::{AccountType, ContactType, FlowType, PlannedStatus, TransactionType};
use alfredodev::state::{
    create_account, create_category, create_company, create_contact, create_forecast,
    create_or_update_planned_entry_from_cfdi, create_planned_entry, create_recurring_plan,
    create_transaction, delete_account, delete_category, delete_contact, delete_forecast,
    delete_planned_entry, delete_recurring_plan, delete_transaction, get_account_by_id,
    get_category_by_id, get_contact_by_id, get_forecast_by_id, get_planned_entry_by_cfdi_uuid,
    get_planned_entry_by_id, get_transaction_by_id, list_accounts, list_categories, list_companies,
    list_contacts, list_forecasts, list_planned_entries, list_recurring_plans, list_transactions,
    pay_planned_entry,
};

#[path = "common/mod.rs"]
mod common;

use chrono::Utc;
use mongodb::bson::DateTime;

fn now() -> DateTime {
    DateTime::from_system_time(SystemTime::now())
}

#[tokio::test]
async fn accounts_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let initial = list_accounts(&state).await.unwrap().len();
    let acc_id = create_account(
        &state,
        &company_id,
        "Test Account",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    assert!(list_accounts(&state).await.unwrap().len() > initial);

    let fetched = get_account_by_id(&state, &acc_id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test Account");

    delete_account(&state, &acc_id, &company_id).await.unwrap();
    assert!(get_account_by_id(&state, &acc_id).await.unwrap().is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn categories_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let initial = list_categories(&state).await.unwrap().len();
    let cat_id = create_category(
        &state,
        &company_id,
        "Test Category",
        FlowType::Income,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(list_categories(&state).await.unwrap().len() > initial);

    let fetched = get_category_by_id(&state, &cat_id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test Category");

    delete_category(&state, &cat_id).await.unwrap();
    assert!(get_category_by_id(&state, &cat_id).await.unwrap().is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn contacts_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let initial = list_contacts(&state).await.unwrap().len();
    let contact_id = create_contact(
        &state,
        &company_id,
        "Test Contact",
        ContactType::Customer,
        Some("test@example.com".into()),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(list_contacts(&state).await.unwrap().len() > initial);

    let fetched = get_contact_by_id(&state, &contact_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.name, "Test Contact");

    delete_contact(&state, &contact_id).await.unwrap();
    assert!(
        get_contact_by_id(&state, &contact_id)
            .await
            .unwrap()
            .is_none()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn recurring_plans_seed_and_creation_work() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    // ensure seed created plans
    assert!(!list_recurring_plans(&state).await.unwrap().is_empty());

    // create dependencies
    let cat_id = create_category(&state, &company_id, "RP Cat", FlowType::Income, None, None)
        .await
        .unwrap();
    let acc_id = create_account(
        &state,
        &company_id,
        "RP Account",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let initial = list_recurring_plans(&state).await.unwrap().len();
    let plan_id = create_recurring_plan(
        &state,
        &company_id,
        "Test Plan",
        FlowType::Income,
        &cat_id,
        &acc_id,
        None,
        100.0,
        "monthly",
        Some(1),
        now(),
        None,
        true,
        1,
        None,
    )
    .await
    .unwrap();
    assert!(list_recurring_plans(&state).await.unwrap().len() > initial);

    // creating plan should also allow planned entries count to grow
    assert!(!list_planned_entries(&state).await.unwrap().is_empty());

    delete_recurring_plan(&state, &plan_id).await.unwrap();

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn planned_entries_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let cat_id = create_category(&state, &company_id, "PE Cat", FlowType::Expense, None, None)
        .await
        .unwrap();
    let acc_id = create_account(
        &state,
        &company_id,
        "PE Account",
        AccountType::Cash,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let initial = list_planned_entries(&state).await.unwrap().len();
    let pe_id = create_planned_entry(
        &state,
        &company_id,
        None,
        None,
        None,
        "Test PE",
        FlowType::Expense,
        &cat_id,
        &acc_id,
        None,
        50.0,
        DateTime::from_chrono(Utc::now()),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    assert!(list_planned_entries(&state).await.unwrap().len() > initial);

    let fetched = get_planned_entry_by_id(&state, &pe_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.name, "Test PE");

    delete_planned_entry(&state, &pe_id).await.unwrap();
    assert!(
        get_planned_entry_by_id(&state, &pe_id)
            .await
            .unwrap()
            .is_none()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn transactions_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let cat_id = create_category(&state, &company_id, "TX Cat", FlowType::Expense, None, None)
        .await
        .unwrap();
    let acc_from = create_account(
        &state,
        &company_id,
        "TX From",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let initial = list_transactions(&state).await.unwrap().len();
    let tx_id = create_transaction(
        &state,
        &company_id,
        DateTime::from_chrono(Utc::now()),
        "Test TX",
        TransactionType::Expense,
        &cat_id,
        Some(acc_from.clone()),
        None,
        25.0,
        None,
        None,
        true,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(list_transactions(&state).await.unwrap().len() > initial);

    let fetched = get_transaction_by_id(&state, &tx_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.description, "Test TX");

    delete_transaction(&state, &tx_id).await.unwrap();
    assert!(
        get_transaction_by_id(&state, &tx_id)
            .await
            .unwrap()
            .is_none()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn cfdi_planned_entry_upsert_is_idempotent() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let cat_id = create_category(
        &state,
        &company_id,
        "CFDI Planned Cat",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let acc_id = create_account(
        &state,
        &company_id,
        "CFDI Planned Account",
        AccountType::Cash,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let due = DateTime::from_chrono(Utc::now());

    let (first_id, created) = create_or_update_planned_entry_from_cfdi(
        &state,
        &company_id,
        due,
        "Proveedor CFDI - UUID-TEST",
        FlowType::Expense,
        &cat_id,
        &acc_id,
        None,
        150.0,
        "UUID-TEST",
        Some("MXN".into()),
        Some("A-1".into()),
        None,
    )
    .await
    .unwrap();
    assert!(created);

    let (second_id, created) = create_or_update_planned_entry_from_cfdi(
        &state,
        &company_id,
        due,
        "Proveedor CFDI - UUID-TEST actualizado",
        FlowType::Expense,
        &cat_id,
        &acc_id,
        None,
        175.0,
        "UUID-TEST",
        Some("MXN".into()),
        Some("A-2".into()),
        None,
    )
    .await
    .unwrap();
    assert!(!created);
    assert_eq!(first_id, second_id);

    let entry = get_planned_entry_by_cfdi_uuid(&state, &company_id, "UUID-TEST")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.amount_estimated, 175.0);
    assert_eq!(entry.cfdi_folio.as_deref(), Some("A-2"));

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn forecasts_crud_works() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let initial = list_forecasts(&state).await.unwrap().len();
    let fc_id = create_forecast(
        &state,
        &company_id,
        DateTime::from_chrono(Utc::now()),
        None,
        DateTime::from_chrono(Utc::now()),
        DateTime::from_chrono(Utc::now()),
        "MXN",
        1000.0,
        500.0,
        500.0,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(list_forecasts(&state).await.unwrap().len() > initial);

    let fetched = get_forecast_by_id(&state, &fc_id).await.unwrap().unwrap();
    assert_eq!(fetched.currency, "MXN");

    delete_forecast(&state, &fc_id).await.unwrap();
    assert!(get_forecast_by_id(&state, &fc_id).await.unwrap().is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn pay_planned_entry_succeeds_when_category_flow_type_mismatches_entry() {
    // Regression: a planned entry whose category has the wrong flow_type (e.g. after
    // the category was edited) must still be payable — the planned entry is the authority
    // on flow type, not the category.
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    // Category is Income but the planned entry will be Expense — the mismatch
    // that used to cause "category flow_type does not match transaction type".
    let cat_id = create_category(
        &state,
        &company_id,
        "Wrong-flow cat",
        FlowType::Income,
        None,
        None,
    )
    .await
    .unwrap();

    let acc_id = create_account(
        &state,
        &company_id,
        "Test Bank",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let due = DateTime::from_chrono(Utc::now());
    let pe_id = create_planned_entry(
        &state,
        &company_id,
        None,
        None,
        None,
        "Rent",
        FlowType::Expense,
        &cat_id,
        &acc_id,
        None,
        1000.0,
        due,
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();

    let initial_txs = list_transactions(&state).await.unwrap().len();

    pay_planned_entry(&state, &pe_id, &company_id, &acc_id, 1000.0, due, None)
        .await
        .expect("payment must succeed even when category flow_type mismatches entry flow_type");

    assert_eq!(
        list_transactions(&state).await.unwrap().len(),
        initial_txs + 1,
        "exactly one transaction must be created"
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn delete_account_integrity_check_is_company_scoped() {
    let ctx = match common::setup_state().await {
        Some(s) => s,
        None => return,
    };
    let state = ctx.state.clone();

    let company_a = create_company(&state, "Scope A", "scope-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "Scope B", "scope-b", "MXN", true, None)
        .await
        .unwrap();

    let acc_a = create_account(&state, &company_a, "A acc", AccountType::Bank, "MXN", true, None)
        .await
        .unwrap();
    let acc_a2 = create_account(&state, &company_a, "A acc 2", AccountType::Bank, "MXN", true, None)
        .await
        .unwrap();

    // Raw-insert references (the API validates company membership, so an
    // out-of-tenant reference can only exist as orphaned/inconsistent data).
    let raw = state
        .transactions
        .clone_with_type::<mongodb::bson::Document>();
    // (1) a transaction in ANOTHER company that points at the company-A account
    raw.insert_one(mongodb::bson::doc! { "company_id": &company_b, "account_from_id": &acc_a })
        .await
        .unwrap();
    // (2) a transaction in the SAME company that points at acc_a2
    raw.insert_one(mongodb::bson::doc! { "company_id": &company_a, "account_from_id": &acc_a2 })
        .await
        .unwrap();

    // The out-of-tenant reference must NOT block deletion (scoped check).
    delete_account(&state, &acc_a, &company_a)
        .await
        .expect("an out-of-tenant reference must not block account deletion");
    assert!(get_account_by_id(&state, &acc_a).await.unwrap().is_none());

    // The in-company reference must STILL block deletion (integrity preserved).
    assert!(
        delete_account(&state, &acc_a2, &company_a).await.is_err(),
        "an in-company reference must still block account deletion"
    );

    common::teardown(Some(ctx)).await;
}
