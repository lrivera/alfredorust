use std::time::SystemTime;

use alfredodev::models::{AccountType, ContactType, FlowType, PlannedStatus, TransactionType};
use alfredodev::state::{
    create_account, create_category, create_contact, create_forecast, create_planned_entry,
    create_recurring_plan, create_transaction, delete_account, delete_category, delete_contact,
    delete_forecast, delete_planned_entry, delete_recurring_plan, delete_transaction,
    get_account_by_id, get_category_by_id, get_contact_by_id, get_forecast_by_id,
    get_planned_entry_by_id, get_transaction_by_id, list_accounts, list_categories,
    list_companies, list_contacts, list_forecasts, list_planned_entries, list_recurring_plans,
    list_transactions,
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

    delete_account(&state, &acc_id).await.unwrap();
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
    )
    .await
    .unwrap();
    assert!(list_contacts(&state).await.unwrap().len() > initial);

    let fetched = get_contact_by_id(&state, &contact_id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test Contact");

    delete_contact(&state, &contact_id).await.unwrap();
    assert!(get_contact_by_id(&state, &contact_id).await.unwrap().is_none());

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
    let cat_id = create_category(
        &state,
        &company_id,
        "RP Cat",
        FlowType::Income,
        None,
        None,
    )
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

    let cat_id = create_category(
        &state,
        &company_id,
        "PE Cat",
        FlowType::Expense,
        None,
        None,
    )
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

    let fetched = get_planned_entry_by_id(&state, &pe_id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test PE");

    delete_planned_entry(&state, &pe_id).await.unwrap();
    assert!(get_planned_entry_by_id(&state, &pe_id).await.unwrap().is_none());

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

    let cat_id = create_category(
        &state,
        &company_id,
        "TX Cat",
        FlowType::Expense,
        None,
        None,
    )
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
        true,
        None,
    )
    .await
    .unwrap();
    assert!(list_transactions(&state).await.unwrap().len() > initial);

    let fetched = get_transaction_by_id(&state, &tx_id).await.unwrap().unwrap();
    assert_eq!(fetched.description, "Test TX");

    delete_transaction(&state, &tx_id).await.unwrap();
    assert!(get_transaction_by_id(&state, &tx_id).await.unwrap().is_none());

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
