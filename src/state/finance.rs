use anyhow::{Context, Result, bail};
use chrono::{DateTime as ChronoDateTime, Datelike, Months, TimeZone, Timelike, Utc};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::time::SystemTime;

use crate::models::{
    Account, AccountType, Category, Contact, ContactType, FlowType, Forecast, PlannedEntry,
    PlannedStatus, RecurringPlan, Transaction, TransactionType,
};

use super::{companies::company_default_currency, AppState, PLANNED_MONTHS_AHEAD};

pub async fn list_accounts(state: &AppState) -> Result<Vec<Account>> {
    let mut cursor = state.accounts.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(account) = cursor.try_next().await? {
        items.push(account);
    }
    Ok(items)
}

pub async fn get_account_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Account>> {
    state
        .accounts
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_account(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    account_type: AccountType,
    currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<ObjectId> {
    let currency = if currency.trim().is_empty() {
        company_default_currency(state, company_id).await?
    } else {
        currency.to_string()
    };

    let res = state
        .accounts
        .insert_one(Account {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            account_type,
            currency,
            is_active,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("account insert missing _id")
}

pub async fn update_account(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    account_type: AccountType,
    currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<()> {
    let currency = if currency.trim().is_empty() {
        company_default_currency(state, company_id).await?
    } else {
        currency.to_string()
    };

    state
        .accounts
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "name": name,
                "account_type": account_type.as_str(),
                "currency": currency,
                "is_active": is_active,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_account(state: &AppState, id: &ObjectId) -> Result<()> {
    let has_transactions = state
        .transactions
        .find_one(doc! { "$or": [
            { "account_from_id": id },
            { "account_to_id": id }
        ]})
        .await?
        .is_some();
    let has_plans = state
        .recurring_plans
        .find_one(doc! { "account_expected_id": id })
        .await?
        .is_some();
    let has_planned_entries = state
        .planned_entries
        .find_one(doc! { "account_expected_id": id })
        .await?
        .is_some();

    if has_transactions || has_plans || has_planned_entries {
        bail!("account has related records; deactivate instead of deleting");
    }

    state.accounts.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn list_categories(state: &AppState) -> Result<Vec<Category>> {
    let mut cursor = state.categories.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(category) = cursor.try_next().await? {
        items.push(category);
    }
    Ok(items)
}

pub async fn get_category_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Category>> {
    state
        .categories
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_category(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    flow_type: FlowType,
    parent_id: Option<ObjectId>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .categories
        .insert_one(Category {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            flow_type,
            parent_id,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("category insert missing _id")
}

pub async fn update_category(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    flow_type: FlowType,
    parent_id: Option<ObjectId>,
    notes: Option<String>,
) -> Result<()> {
    state
        .categories
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "name": name,
                "flow_type": flow_type.as_str(),
                "parent_id": parent_id,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_category(state: &AppState, id: &ObjectId) -> Result<()> {
    state.categories.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn list_contacts(state: &AppState) -> Result<Vec<Contact>> {
    let mut cursor = state.contacts.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(contact) = cursor.try_next().await? {
        items.push(contact);
    }
    Ok(items)
}

pub async fn get_contact_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Contact>> {
    state
        .contacts
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_contact(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    contact_type: ContactType,
    email: Option<String>,
    phone: Option<String>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .contacts
        .insert_one(Contact {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            contact_type,
            email,
            phone,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("contact insert missing _id")
}

pub async fn update_contact(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    contact_type: ContactType,
    email: Option<String>,
    phone: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    state
        .contacts
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "name": name,
                "contact_type": contact_type.as_str(),
                "email": email,
                "phone": phone,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_contact(state: &AppState, id: &ObjectId) -> Result<()> {
    state.contacts.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn list_recurring_plans(state: &AppState) -> Result<Vec<RecurringPlan>> {
    let mut cursor = state.recurring_plans.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(plan) = cursor.try_next().await? {
        items.push(plan);
    }
    Ok(items)
}

pub async fn get_recurring_plan_by_id(
    state: &AppState,
    id: &ObjectId,
) -> Result<Option<RecurringPlan>> {
    state
        .recurring_plans
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_recurring_plan(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    flow_type: FlowType,
    category_id: &ObjectId,
    account_expected_id: &ObjectId,
    contact_id: Option<ObjectId>,
    amount_estimated: f64,
    frequency: &str,
    day_of_month: Option<i32>,
    start_date: DateTime,
    end_date: Option<DateTime>,
    is_active: bool,
    _version: i32,
    notes: Option<String>,
) -> Result<ObjectId> {
    let version = 1;
    let now = DateTime::from_system_time(SystemTime::now());

    let mut plan = RecurringPlan {
        id: None,
        company_id: company_id.clone(),
        name: name.to_string(),
        flow_type,
        category_id: category_id.clone(),
        account_expected_id: account_expected_id.clone(),
        contact_id,
        amount_estimated,
        frequency: frequency.to_string(),
        day_of_month,
        start_date,
        end_date,
        is_active,
        version,
        created_at: Some(now),
        updated_at: None,
        notes,
    };

    let res = state.recurring_plans.insert_one(plan.clone()).await?;
    let id = res
        .inserted_id
        .as_object_id()
        .context("recurring plan insert missing _id")?;

    plan.id = Some(id.clone());
    generate_planned_entries_for_plan(state, &plan, PLANNED_MONTHS_AHEAD).await?;

    Ok(id)
}

pub async fn update_recurring_plan(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    flow_type: FlowType,
    category_id: &ObjectId,
    account_expected_id: &ObjectId,
    contact_id: Option<ObjectId>,
    amount_estimated: f64,
    frequency: &str,
    day_of_month: Option<i32>,
    start_date: DateTime,
    end_date: Option<DateTime>,
    is_active: bool,
    _version: i32,
    notes: Option<String>,
) -> Result<()> {
    let existing = state
        .recurring_plans
        .find_one(doc! { "_id": id })
        .await?
        .context("recurring plan not found")?;

    let mut new_version = existing.version;
    let significant_change = existing.name != name
        || existing.flow_type != flow_type
        || existing.category_id != *category_id
        || existing.account_expected_id != *account_expected_id
        || existing.contact_id != contact_id
        || (existing.amount_estimated - amount_estimated).abs() > f64::EPSILON
        || existing.frequency != frequency
        || existing.day_of_month != day_of_month
        || existing.start_date != start_date
        || existing.end_date != end_date
        || existing.is_active != is_active;

    if significant_change {
        new_version += 1;
    }

    let final_end_date = if !is_active {
        Some(DateTime::from_system_time(SystemTime::now()))
    } else {
        end_date
    };

    state
        .recurring_plans
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "name": name,
                "flow_type": flow_type.as_str(),
                "category_id": category_id,
                "account_expected_id": account_expected_id,
                "contact_id": contact_id,
                "amount_estimated": amount_estimated,
                "frequency": frequency,
                "day_of_month": day_of_month,
                "start_date": start_date,
                "end_date": final_end_date,
                "is_active": is_active,
                "version": new_version,
                "notes": notes.clone(),
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;

    let updated_plan = RecurringPlan {
        id: Some(id.clone()),
        company_id: company_id.clone(),
        name: name.to_string(),
        flow_type,
        category_id: category_id.clone(),
        account_expected_id: account_expected_id.clone(),
        contact_id,
        amount_estimated,
        frequency: frequency.to_string(),
        day_of_month,
        start_date,
        end_date: final_end_date,
        is_active,
        version: new_version,
        created_at: existing.created_at,
        updated_at: Some(DateTime::from_system_time(SystemTime::now())),
        notes,
    };

    if is_active {
        regenerate_planned_entries(state, &updated_plan).await?;
    } else if let Some(plan_id) = updated_plan.id.as_ref() {
        delete_future_open_entries(state, plan_id).await?;
    }

    Ok(())
}

pub async fn delete_recurring_plan(state: &AppState, id: &ObjectId) -> Result<()> {
    let now = DateTime::from_system_time(SystemTime::now());
    state
        .recurring_plans
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "is_active": false,
                "end_date": now,
                "updated_at": now,
            }},
        )
        .await?;
    delete_future_open_entries(state, id).await?;
    Ok(())
}

pub async fn list_planned_entries(state: &AppState) -> Result<Vec<PlannedEntry>> {
    let mut cursor = state.planned_entries.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(entry) = cursor.try_next().await? {
        items.push(entry);
    }
    Ok(items)
}

pub async fn get_planned_entry_by_id(
    state: &AppState,
    id: &ObjectId,
) -> Result<Option<PlannedEntry>> {
    state
        .planned_entries
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_planned_entry(
    state: &AppState,
    company_id: &ObjectId,
    recurring_plan_id: Option<ObjectId>,
    recurring_plan_version: Option<i32>,
    name: &str,
    flow_type: FlowType,
    category_id: &ObjectId,
    account_expected_id: &ObjectId,
    contact_id: Option<ObjectId>,
    amount_estimated: f64,
    due_date: DateTime,
    _status: PlannedStatus,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .planned_entries
        .insert_one(PlannedEntry {
            id: None,
            company_id: company_id.clone(),
            recurring_plan_id,
            recurring_plan_version,
            name: name.to_string(),
            flow_type,
            category_id: category_id.clone(),
            account_expected_id: account_expected_id.clone(),
            contact_id,
            amount_estimated,
            due_date,
            status: PlannedStatus::Planned,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("planned entry insert missing _id")
}

pub async fn update_planned_entry(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    recurring_plan_id: Option<ObjectId>,
    recurring_plan_version: Option<i32>,
    name: &str,
    flow_type: FlowType,
    category_id: &ObjectId,
    account_expected_id: &ObjectId,
    contact_id: Option<ObjectId>,
    amount_estimated: f64,
    due_date: DateTime,
    status: PlannedStatus,
    notes: Option<String>,
) -> Result<()> {
    state
        .planned_entries
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "recurring_plan_id": recurring_plan_id,
                "recurring_plan_version": recurring_plan_version,
                "name": name,
                "flow_type": flow_type.as_str(),
                "category_id": category_id,
                "account_expected_id": account_expected_id,
                "contact_id": contact_id,
                "amount_estimated": amount_estimated,
                "due_date": due_date,
                "status": status.as_str(),
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    let _ = recalculate_planned_entry_status(state, id).await;
    Ok(())
}

pub async fn delete_planned_entry(state: &AppState, id: &ObjectId) -> Result<()> {
    state.planned_entries.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn list_transactions(state: &AppState) -> Result<Vec<Transaction>> {
    let mut cursor = state.transactions.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(transaction) = cursor.try_next().await? {
        items.push(transaction);
    }
    Ok(items)
}

pub async fn get_transaction_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Transaction>> {
    state
        .transactions
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_transaction(
    state: &AppState,
    company_id: &ObjectId,
    date: DateTime,
    description: &str,
    transaction_type: TransactionType,
    category_id: &ObjectId,
    account_from_id: Option<ObjectId>,
    account_to_id: Option<ObjectId>,
    amount: f64,
    planned_entry_id: Option<ObjectId>,
    is_confirmed: bool,
    notes: Option<String>,
) -> Result<ObjectId> {
    validate_transaction_links(
        state,
        company_id,
        &transaction_type,
        &category_id,
        account_from_id.as_ref(),
        account_to_id.as_ref(),
        planned_entry_id.as_ref(),
    )
    .await?;

    let res = state
        .transactions
        .insert_one(Transaction {
            id: None,
            company_id: company_id.clone(),
            date,
            description: description.to_string(),
            transaction_type: transaction_type.clone(),
            category_id: category_id.clone(),
            account_from_id,
            account_to_id,
            amount,
            planned_entry_id,
            is_confirmed,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;

    if let Some(pe_id) = planned_entry_id {
        let _ = recalculate_planned_entry_status(state, &pe_id).await;
    }

    res.inserted_id
        .as_object_id()
        .context("transaction insert missing _id")
}

pub async fn update_transaction(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    date: DateTime,
    description: &str,
    transaction_type: TransactionType,
    category_id: &ObjectId,
    account_from_id: Option<ObjectId>,
    account_to_id: Option<ObjectId>,
    amount: f64,
    planned_entry_id: Option<ObjectId>,
    is_confirmed: bool,
    notes: Option<String>,
) -> Result<()> {
    let existing = state
        .transactions
        .find_one(doc! { "_id": id })
        .await?
        .context("transaction not found")?;

    validate_transaction_links(
        state,
        company_id,
        &transaction_type,
        &category_id,
        account_from_id.as_ref(),
        account_to_id.as_ref(),
        planned_entry_id.as_ref(),
    )
    .await?;

    state
        .transactions
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "date": date,
                "description": description,
                "transaction_type": transaction_type.as_str(),
                "category_id": category_id,
                "account_from_id": account_from_id,
                "account_to_id": account_to_id,
                "amount": amount,
                "planned_entry_id": planned_entry_id,
                "is_confirmed": is_confirmed,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;

    if existing.planned_entry_id != planned_entry_id {
        if let Some(old) = existing.planned_entry_id {
            let _ = recalculate_planned_entry_status(state, &old).await;
        }
    }
    if let Some(new_pe) = planned_entry_id {
        let _ = recalculate_planned_entry_status(state, &new_pe).await;
    }

    Ok(())
}

pub async fn delete_transaction(state: &AppState, id: &ObjectId) -> Result<()> {
    let existing = state.transactions.find_one(doc! { "_id": id }).await?;

    state.transactions.delete_one(doc! { "_id": id }).await?;

    if let Some(tx) = existing {
        if let Some(pe_id) = tx.planned_entry_id {
            let _ = recalculate_planned_entry_status(state, &pe_id).await;
        }
    }

    Ok(())
}

pub async fn list_forecasts(state: &AppState) -> Result<Vec<Forecast>> {
    let mut cursor = state.forecasts.find(doc! {}).await?;
    let mut items = Vec::new();
    while let Some(forecast) = cursor.try_next().await? {
        items.push(forecast);
    }
    Ok(items)
}

pub async fn get_forecast_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Forecast>> {
    state
        .forecasts
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_forecast(
    state: &AppState,
    company_id: &ObjectId,
    generated_at: DateTime,
    generated_by_user_id: Option<ObjectId>,
    start_date: DateTime,
    end_date: DateTime,
    currency: &str,
    projected_income_total: f64,
    projected_expense_total: f64,
    projected_net: f64,
    initial_balance: Option<f64>,
    final_balance: Option<f64>,
    details: Option<String>,
    scenario_name: Option<String>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .forecasts
        .insert_one(Forecast {
            id: None,
            company_id: company_id.clone(),
            generated_at,
            generated_by_user_id,
            start_date,
            end_date,
            currency: currency.to_string(),
            projected_income_total,
            projected_expense_total,
            projected_net,
            initial_balance,
            final_balance,
            details,
            scenario_name,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("forecast insert missing _id")
}

pub async fn update_forecast(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    generated_at: DateTime,
    generated_by_user_id: Option<ObjectId>,
    start_date: DateTime,
    end_date: DateTime,
    currency: &str,
    projected_income_total: f64,
    projected_expense_total: f64,
    projected_net: f64,
    initial_balance: Option<f64>,
    final_balance: Option<f64>,
    details: Option<String>,
    scenario_name: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    state
        .forecasts
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "company_id": company_id,
                "generated_at": generated_at,
                "generated_by_user_id": generated_by_user_id,
                "start_date": start_date,
                "end_date": end_date,
                "currency": currency,
                "projected_income_total": projected_income_total,
                "projected_expense_total": projected_expense_total,
                "projected_net": projected_net,
                "initial_balance": initial_balance,
                "final_balance": final_balance,
                "details": details,
                "scenario_name": scenario_name,
                "notes": notes,
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_forecast(state: &AppState, id: &ObjectId) -> Result<()> {
    state.forecasts.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

async fn validate_transaction_links(
    state: &AppState,
    company_id: &ObjectId,
    transaction_type: &TransactionType,
    category_id: &ObjectId,
    account_from_id: Option<&ObjectId>,
    account_to_id: Option<&ObjectId>,
    planned_entry_id: Option<&ObjectId>,
) -> Result<()> {
    match transaction_type {
        TransactionType::Income => {
            if account_to_id.is_none() {
                bail!("income transaction requires account_to_id");
            }
            if account_from_id.is_some() {
                bail!("income should not set account_from_id");
            }
        }
        TransactionType::Expense => {
            if account_from_id.is_none() {
                bail!("expense transaction requires account_from_id");
            }
            if account_to_id.is_some() {
                bail!("expense should not set account_to_id");
            }
        }
        TransactionType::Transfer => {
            let from = account_from_id.context("transfer needs account_from_id")?;
            let to = account_to_id.context("transfer needs account_to_id")?;
            if from == to {
                bail!("transfer accounts must differ");
            }
        }
    }

    if let Some(acc) = account_from_id {
        ensure_account_active_in_company(state, acc, company_id).await?;
    }
    if let Some(acc) = account_to_id {
        ensure_account_active_in_company(state, acc, company_id).await?;
    }

    ensure_category_matches_flow(state, category_id, company_id, transaction_type).await?;

    if let Some(pe_id) = planned_entry_id {
        ensure_planned_entry_alignment(state, pe_id, company_id, transaction_type).await?;
    }

    Ok(())
}

async fn ensure_account_active_in_company(
    state: &AppState,
    account_id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    let account = state
        .accounts
        .find_one(doc! { "_id": account_id })
        .await?
        .context("account not found")?;

    if &account.company_id != company_id {
        bail!("account belongs to another company");
    }
    if !account.is_active {
        bail!("account is inactive");
    }
    Ok(())
}

async fn ensure_category_matches_flow(
    state: &AppState,
    category_id: &ObjectId,
    company_id: &ObjectId,
    transaction_type: &TransactionType,
) -> Result<()> {
    let category = state
        .categories
        .find_one(doc! { "_id": category_id })
        .await?
        .context("category not found")?;

    if &category.company_id != company_id {
        bail!("category belongs to another company");
    }

    let expected_flow = match *transaction_type {
        TransactionType::Income => FlowType::Income,
        TransactionType::Expense => FlowType::Expense,
        TransactionType::Transfer => return Ok(()),
    };

    if category.flow_type != expected_flow {
        bail!("category flow_type does not match transaction type");
    }

    Ok(())
}

async fn ensure_planned_entry_alignment(
    state: &AppState,
    planned_entry_id: &ObjectId,
    company_id: &ObjectId,
    transaction_type: &TransactionType,
) -> Result<()> {
    let pe = state
        .planned_entries
        .find_one(doc! { "_id": planned_entry_id })
        .await?
        .context("planned entry not found")?;

    if &pe.company_id != company_id {
        bail!("planned entry belongs to another company");
    }

    if matches!(pe.status, PlannedStatus::Cancelled) {
        bail!("planned entry is cancelled");
    }

    match (transaction_type.clone(), pe.flow_type) {
        (TransactionType::Income, FlowType::Income)
        | (TransactionType::Expense, FlowType::Expense) => {}
        _ => bail!("planned entry flow_type mismatches transaction type"),
    }

    Ok(())
}

async fn recalculate_planned_entry_status(
    state: &AppState,
    planned_entry_id: &ObjectId,
) -> Result<()> {
    let pe = match state
        .planned_entries
        .find_one(doc! { "_id": planned_entry_id })
        .await?
    {
        Some(pe) => pe,
        None => return Ok(()),
    };

    if matches!(pe.status, PlannedStatus::Cancelled) {
        return Ok(());
    }

    let mut total = 0_f64;
    let mut cursor = state
        .transactions
        .find(doc! { "planned_entry_id": planned_entry_id })
        .await?;
    while let Some(tx) = cursor.try_next().await? {
        total += tx.amount;
    }

    let mut status = if total <= 0.0 {
        PlannedStatus::Planned
    } else if total < pe.amount_estimated {
        PlannedStatus::PartiallyCovered
    } else {
        PlannedStatus::Covered
    };

    let now = DateTime::from_system_time(SystemTime::now());
    if matches!(
        status,
        PlannedStatus::Planned | PlannedStatus::PartiallyCovered
    ) && pe.due_date < now
    {
        status = PlannedStatus::Overdue;
    }

    if status != pe.status {
        state
            .planned_entries
            .update_one(
                doc! { "_id": planned_entry_id },
                doc! { "$set": {
                    "status": status.as_str(),
                    "updated_at": DateTime::from_system_time(SystemTime::now()),
                } },
            )
            .await?;
    }

    Ok(())
}

pub async fn regenerate_planned_entries(state: &AppState, plan: &RecurringPlan) -> Result<()> {
    if plan.id.is_none() || !plan.is_active {
        return Ok(());
    }

    let plan_id = plan.id.as_ref().unwrap();
    delete_future_open_entries(state, plan_id).await?;
    generate_planned_entries_for_plan(state, plan, PLANNED_MONTHS_AHEAD).await
}

pub async fn regenerate_planned_entries_for_plan_id(
    state: &AppState,
    plan_id: &ObjectId,
) -> Result<()> {
    let plan = state
        .recurring_plans
        .find_one(doc! { "_id": plan_id })
        .await?
        .context("recurring plan not found")?;

    if !plan.is_active {
        bail!("recurring plan is inactive");
    }

    regenerate_planned_entries(state, &plan).await
}

async fn delete_future_open_entries(state: &AppState, plan_id: &ObjectId) -> Result<()> {
    let now = DateTime::from_system_time(SystemTime::now());
    state
        .planned_entries
        .delete_many(doc! {
            "recurring_plan_id": plan_id,
            "status": { "$in": [PlannedStatus::Planned.as_str(), PlannedStatus::PartiallyCovered.as_str()] },
            "due_date": { "$gte": now },
        })
        .await?;
    Ok(())
}

async fn generate_planned_entries_for_plan(
    state: &AppState,
    plan: &RecurringPlan,
    months_ahead: u32,
) -> Result<()> {
    if !plan.is_active {
        return Ok(());
    }
    let Some(plan_id) = plan.id.as_ref() else {
        return Ok(());
    };

    let now_ref = Utc::now();
    let due_dates = upcoming_due_dates(plan, months_ahead, now_ref);

    for due in due_dates {
        let _ = state
            .planned_entries
            .insert_one(PlannedEntry {
                id: None,
                company_id: plan.company_id.clone(),
                recurring_plan_id: Some(plan_id.clone()),
                recurring_plan_version: Some(plan.version),
                name: format!("{} {}", plan.name, due.to_chrono().date_naive()),
                flow_type: plan.flow_type.clone(),
                category_id: plan.category_id.clone(),
                account_expected_id: plan.account_expected_id.clone(),
                contact_id: plan.contact_id.clone(),
                amount_estimated: plan.amount_estimated,
                due_date: due,
                status: PlannedStatus::Planned,
                created_at: Some(DateTime::from_system_time(SystemTime::now())),
                updated_at: None,
                notes: plan.notes.clone(),
            })
            .await?;
    }
    Ok(())
}

fn upcoming_due_dates(
    plan: &RecurringPlan,
    months_ahead: u32,
    now_ref: ChronoDateTime<Utc>,
) -> Vec<DateTime> {
    let start = plan.start_date.to_chrono();
    let mut dates = Vec::new();
    let end_limit = plan.end_date.map(|d| d.to_chrono());

    match plan.frequency.to_lowercase().as_str() {
        "monthly" => {
            let anchor = align_to_day(start, plan.day_of_month);
            let base = if now_ref.date_naive() > anchor.date_naive() {
                align_to_day(now_ref, plan.day_of_month)
            } else {
                anchor
            };

            for i in 0..months_ahead {
                let candidate = base
                    .checked_add_months(Months::new(i.into()))
                    .unwrap_or(base);
                if candidate < start {
                    continue;
                }
                if let Some(end) = end_limit {
                    if candidate > end {
                        break;
                    }
                }
                dates.push(DateTime::from_chrono(candidate));
            }
        }
        "weekly" => {
            let step = chrono::Duration::days(7);
            let mut current = start;
            while current + step <= now_ref {
                current = current + step;
            }
            for _ in 0..months_ahead {
                if let Some(end) = end_limit {
                    if current > end {
                        break;
                    }
                }
                if current >= start {
                    dates.push(DateTime::from_chrono(current));
                }
                current = current + step;
            }
        }
        "biweekly" => {
            let step = chrono::Duration::days(14);
            let mut current = start;
            while current + step <= now_ref {
                current = current + step;
            }
            for _ in 0..months_ahead {
                if let Some(end) = end_limit {
                    if current > end {
                        break;
                    }
                }
                if current >= start {
                    dates.push(DateTime::from_chrono(current));
                }
                current = current + step;
            }
        }
        _ => {
            let step = chrono::Duration::days(30);
            let mut current = if now_ref > start { now_ref } else { start };
            for _ in 0..months_ahead {
                if current >= start {
                    if let Some(end) = end_limit {
                        if current > end {
                            break;
                        }
                    }
                    dates.push(DateTime::from_chrono(current));
                }
                current = current + step;
            }
        }
    }

    dates
}

fn align_to_day(dt: ChronoDateTime<Utc>, day: Option<i32>) -> ChronoDateTime<Utc> {
    let chosen_day = day.unwrap_or(dt.day() as i32);
    let clamped = clamp_day(dt.year(), dt.month(), chosen_day);
    Utc.with_ymd_and_hms(
        dt.year(),
        dt.month(),
        clamped,
        dt.hour(),
        dt.minute(),
        dt.second(),
    )
    .single()
    .unwrap_or(dt)
}

fn clamp_day(year: i32, month: u32, day: i32) -> u32 {
    if day < 1 {
        return 1;
    }
    let day_u32 = day as u32;
    chrono::NaiveDate::from_ymd_opt(year, month, day_u32)
        .map(|d| d.day())
        .unwrap_or_else(|| {
            let next_month = if month == 12 { 1 } else { month + 1 };
            let next_year = if month == 12 { year + 1 } else { year };
            let last_day = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
                .unwrap()
                .pred_opt()
                .unwrap()
                .day();
            last_day
        })
}

