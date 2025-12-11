// state.rs
// AppState encapsulating Mongo collections plus initialization and helpers.

use anyhow::{Context, Result, bail};
use chrono::{DateTime as ChronoDateTime, Datelike, Months, TimeZone, Timelike, Utc};
use data_encoding::BASE32_NOPAD;
use futures::stream::TryStreamExt;
use mongodb::{
    Client, Collection, Database,
    bson::{DateTime, doc, oid::ObjectId},
};
use rand::RngCore;
use serde::de::DeserializeOwned;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    time::{Duration, SystemTime},
};

use crate::models::{
    Account, AccountType, Category, Company, Contact, ContactType, FlowType, Forecast,
    PlannedEntry, PlannedStatus, RecurringPlan, SeedUser, Session, Transaction, TransactionType,
    User, UserRole,
};

pub const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24; // 1 day
const PLANNED_MONTHS_AHEAD: u32 = 24;

#[derive(Clone)]
pub struct AppState {
    pub users: Collection<User>,
    pub companies: Collection<Company>,
    pub sessions: Collection<Session>,
    pub accounts: Collection<Account>,
    pub categories: Collection<Category>,
    pub contacts: Collection<Contact>,
    pub recurring_plans: Collection<RecurringPlan>,
    pub planned_entries: Collection<PlannedEntry>,
    pub transactions: Collection<Transaction>,
    pub forecasts: Collection<Forecast>,
}

#[derive(Clone)]
pub struct UserWithCompany {
    pub id: ObjectId,
    pub email: String,
    pub secret: String,
    pub company_id: ObjectId,
    pub company_name: String,
    pub role: UserRole,
}

pub async fn init_state() -> Result<AppState> {
    let uri = env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let db_name = env::var("MONGODB_DB").unwrap_or_else(|_| "totp".to_string());

    let client = Client::with_uri_str(uri).await?;
    let db = client.database(&db_name);

    ensure_collections(&db).await?;

    let default_users = load_default_users()?;
    let company_names = derive_company_names(&default_users);
    let company_ids = seed_default_companies(&db, &company_names).await?;
    seed_default_users(&db, &default_users, &company_ids).await?;
    seed_sample_finance(&db, company_ids.values().next().cloned()).await?;

    Ok(AppState {
        users: db.collection::<User>("users"),
        companies: db.collection::<Company>("company"),
        sessions: db.collection::<Session>("sessions"),
        accounts: db.collection::<Account>("accounts"),
        categories: db.collection::<Category>("categories"),
        contacts: db.collection::<Contact>("contacts"),
        recurring_plans: db.collection::<RecurringPlan>("recurring_plans"),
        planned_entries: db.collection::<PlannedEntry>("planned_entries"),
        transactions: db.collection::<Transaction>("transactions"),
        forecasts: db.collection::<Forecast>("forecasts"),
    })
}

fn load_default_users() -> Result<Vec<SeedUser>> {
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "./data/users.json".to_string());
    let users_json = fs::read_to_string(&users_file)?;
    let users = serde_json::from_str::<Vec<SeedUser>>(&users_json)?;
    Ok(users)
}

fn load_json_array<T: DeserializeOwned>(env_key: &str, default_path: &str) -> Result<Vec<T>> {
    let path = env::var(env_key).unwrap_or_else(|_| default_path.to_string());
    if let Ok(contents) = fs::read_to_string(&path) {
        let parsed = serde_json::from_str::<Vec<T>>(&contents)?;
        Ok(parsed)
    } else {
        Ok(Vec::new())
    }
}

fn derive_company_names(users: &[SeedUser]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut companies = Vec::new();
    for user in users {
        if seen.insert(user.company.clone()) {
            companies.push(user.company.clone());
        }
    }
    companies
}

async fn ensure_collections(db: &Database) -> Result<()> {
    let existing = db.list_collection_names().await?;
    if !existing.iter().any(|name| name == "users") {
        db.create_collection("users").await?;
    }
    if !existing.iter().any(|name| name == "company") {
        db.create_collection("company").await?;
    }
    if !existing.iter().any(|name| name == "sessions") {
        db.create_collection("sessions").await?;
    }
    if !existing.iter().any(|name| name == "accounts") {
        db.create_collection("accounts").await?;
    }
    if !existing.iter().any(|name| name == "categories") {
        db.create_collection("categories").await?;
    }
    if !existing.iter().any(|name| name == "contacts") {
        db.create_collection("contacts").await?;
    }
    if !existing.iter().any(|name| name == "recurring_plans") {
        db.create_collection("recurring_plans").await?;
    }
    if !existing.iter().any(|name| name == "planned_entries") {
        db.create_collection("planned_entries").await?;
    }
    if !existing.iter().any(|name| name == "transactions") {
        db.create_collection("transactions").await?;
    }
    if !existing.iter().any(|name| name == "forecasts") {
        db.create_collection("forecasts").await?;
    }
    Ok(())
}

async fn seed_default_companies(
    db: &Database,
    names: &[String],
) -> Result<HashMap<String, ObjectId>> {
    let coll = db.collection::<Company>("company");
    let mut map = HashMap::new();

    for name in names {
        if let Some(existing) = coll.find_one(doc! { "name": name.clone() }).await? {
            if let Some(id) = existing.id {
                map.insert(name.clone(), id);
                continue;
            }
        }

        let inserted = coll
            .insert_one(Company {
                id: None,
                name: name.clone(),
                default_currency: String::new(),
                is_active: true,
                created_at: None,
                updated_at: None,
                notes: None,
            })
            .await?;
        let id = inserted
            .inserted_id
            .as_object_id()
            .context("company insert did not return an ObjectId")?;
        map.insert(name.clone(), id);
    }

    Ok(map)
}

async fn seed_default_users(
    db: &Database,
    defaults: &[SeedUser],
    company_ids: &HashMap<String, ObjectId>,
) -> Result<()> {
    let coll = db.collection::<mongodb::bson::Document>("users");
    for user in defaults {
        let company_id = company_ids
            .get(&user.company)
            .cloned()
            .context(format!("missing company id for {}", user.company))?;

        coll.update_one(
            doc! { "email": &user.email },
            doc! {
                "$set": {
                    "secret": &user.secret,
                    "company": company_id,
                    "role": user.role.as_str(),
                },
                "$setOnInsert": {
                    "email": &user.email,
                }
            },
        )
        .upsert(true)
        .await?;
    }
    Ok(())
}

async fn seed_sample_finance(db: &Database, company_id: Option<ObjectId>) -> Result<()> {
    let company_id = if let Some(id) = company_id {
        id
    } else {
        return Ok(());
    };

    let accounts_coll = db.collection::<Account>("accounts");
    if !is_collection_empty(&accounts_coll).await? {
        return Ok(());
    }

    let categories_coll = db.collection::<Category>("categories");
    let contacts_coll = db.collection::<Contact>("contacts");
    let plans_coll = db.collection::<RecurringPlan>("recurring_plans");
    let planned_coll = db.collection::<PlannedEntry>("planned_entries");
    let tx_coll = db.collection::<Transaction>("transactions");
    let forecast_coll = db.collection::<Forecast>("forecasts");

    let accounts_seed: Vec<Account> = load_json_array("ACCOUNTS_FILE", "./data/accounts.json")?;
    let categories_seed: Vec<Category> =
        load_json_array("CATEGORIES_FILE", "./data/categories.json")?;
    let contacts_seed: Vec<Contact> = load_json_array("CONTACTS_FILE", "./data/contacts.json")?;
    let plans_seed: Vec<RecurringPlan> =
        load_json_array("RECURRING_PLANS_FILE", "./data/recurring_plans.json")?;
    let planned_seed: Vec<PlannedEntry> =
        load_json_array("PLANNED_ENTRIES_FILE", "./data/planned_entries.json")?;
    let tx_seed: Vec<Transaction> =
        load_json_array("TRANSACTIONS_FILE", "./data/transactions.json")?;
    let forecasts_seed: Vec<Forecast> = load_json_array("FORECASTS_FILE", "./data/forecasts.json")?;

    let mut account_map = HashMap::new();
    for acc in accounts_seed {
        let old_id = acc.id.unwrap_or_else(ObjectId::new);
        let res = accounts_coll
            .insert_one(Account {
                id: None,
                company_id: company_id.clone(),
                name: acc.name,
                account_type: acc.account_type,
                currency: acc.currency,
                is_active: acc.is_active,
                created_at: acc.created_at,
                updated_at: acc.updated_at,
                notes: acc.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("account insert missing _id")?;
        account_map.insert(old_id, new_id);
    }

    let mut category_map = HashMap::new();
    for cat in categories_seed {
        let old_id = cat.id.unwrap_or_else(ObjectId::new);
        let parent_old = cat.parent_id;
        let parent_new = parent_old.and_then(|p| category_map.get(&p).cloned());
        let res = categories_coll
            .insert_one(Category {
                id: None,
                company_id: company_id.clone(),
                name: cat.name,
                flow_type: cat.flow_type,
                parent_id: parent_new,
                created_at: cat.created_at,
                updated_at: cat.updated_at,
                notes: cat.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("category insert missing _id")?;
        category_map.insert(old_id, new_id);
    }

    let mut contact_map = HashMap::new();
    for contact in contacts_seed {
        let old_id = contact.id.unwrap_or_else(ObjectId::new);
        let res = contacts_coll
            .insert_one(Contact {
                id: None,
                company_id: company_id.clone(),
                name: contact.name,
                contact_type: contact.contact_type,
                email: contact.email,
                phone: contact.phone,
                created_at: contact.created_at,
                updated_at: contact.updated_at,
                notes: contact.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("contact insert missing _id")?;
        contact_map.insert(old_id, new_id);
    }

    let mut plan_map = HashMap::new();
    for plan in plans_seed {
        let old_id = plan.id.unwrap_or_else(ObjectId::new);
        let category_id = remap_id(&category_map, &plan.category_id)?;
        let account_expected_id = remap_id(&account_map, &plan.account_expected_id)?;
        let contact_id = plan.contact_id.and_then(|c| contact_map.get(&c).cloned());

        let res = plans_coll
            .insert_one(RecurringPlan {
                id: None,
                company_id: company_id.clone(),
                name: plan.name,
                flow_type: plan.flow_type,
                category_id,
                account_expected_id,
                contact_id,
                amount_estimated: plan.amount_estimated,
                frequency: plan.frequency,
                day_of_month: plan.day_of_month,
                start_date: plan.start_date,
                end_date: plan.end_date,
                is_active: plan.is_active,
                version: plan.version,
                created_at: plan.created_at,
                updated_at: plan.updated_at,
                notes: plan.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("recurring plan insert missing _id")?;
        plan_map.insert(old_id, new_id);
    }

    let mut planned_map = HashMap::new();
    for pe in planned_seed {
        let old_id = pe.id.unwrap_or_else(ObjectId::new);
        let category_id = remap_id(&category_map, &pe.category_id)?;
        let account_expected_id = remap_id(&account_map, &pe.account_expected_id)?;
        let contact_id = pe.contact_id.and_then(|c| contact_map.get(&c).cloned());
        let recurring_plan_id = pe.recurring_plan_id.and_then(|p| plan_map.get(&p).cloned());

        let res = planned_coll
            .insert_one(PlannedEntry {
                id: None,
                company_id: company_id.clone(),
                recurring_plan_id,
                recurring_plan_version: pe.recurring_plan_version,
                name: pe.name,
                flow_type: pe.flow_type,
                category_id,
                account_expected_id,
                contact_id,
                amount_estimated: pe.amount_estimated,
                due_date: pe.due_date,
                status: pe.status,
                created_at: pe.created_at,
                updated_at: pe.updated_at,
                notes: pe.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("planned entry insert missing _id")?;
        planned_map.insert(old_id, new_id);
    }

    for tx in tx_seed {
        let category_id = remap_id(&category_map, &tx.category_id)?;
        let account_from_id = tx
            .account_from_id
            .and_then(|a| account_map.get(&a).cloned());
        let account_to_id = tx.account_to_id.and_then(|a| account_map.get(&a).cloned());
        let planned_entry_id = tx
            .planned_entry_id
            .and_then(|p| planned_map.get(&p).cloned());

        let _ = tx_coll
            .insert_one(Transaction {
                id: None,
                company_id: company_id.clone(),
                date: tx.date,
                description: tx.description,
                transaction_type: tx.transaction_type,
                category_id,
                account_from_id,
                account_to_id,
                amount: tx.amount,
                planned_entry_id,
                is_confirmed: tx.is_confirmed,
                created_at: tx.created_at,
                updated_at: tx.updated_at,
                notes: tx.notes,
            })
            .await?;
    }

    for fc in forecasts_seed {
        let _ = forecast_coll
            .insert_one(Forecast {
                id: None,
                company_id: company_id.clone(),
                generated_at: fc.generated_at,
                generated_by_user_id: fc.generated_by_user_id,
                start_date: fc.start_date,
                end_date: fc.end_date,
                currency: fc.currency,
                projected_income_total: fc.projected_income_total,
                projected_expense_total: fc.projected_expense_total,
                projected_net: fc.projected_net,
                initial_balance: fc.initial_balance,
                final_balance: fc.final_balance,
                details: fc.details,
                scenario_name: fc.scenario_name,
                notes: fc.notes,
            })
            .await?;
    }

    Ok(())
}

async fn is_collection_empty<T: Send + Sync>(coll: &Collection<T>) -> Result<bool> {
    let count = coll.estimated_document_count().await?;
    Ok(count == 0)
}

fn remap_id(map: &HashMap<ObjectId, ObjectId>, original: &ObjectId) -> Result<ObjectId> {
    map.get(original)
        .cloned()
        .context("missing referenced id during seeding")
}

pub async fn find_user(state: &AppState, email: &str) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "email": email }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_session(state: &AppState, email: &str) -> Result<String> {
    // Optionally drop previous sessions for this user (best-effort)
    let _ = state
        .sessions
        .delete_many(doc! { "user_email": email.to_string() })
        .await;

    let mut token_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let token = BASE32_NOPAD.encode(&token_bytes);

    let expires_at =
        DateTime::from_system_time(SystemTime::now() + Duration::from_secs(SESSION_TTL_SECONDS));

    state
        .sessions
        .insert_one(Session {
            id: None,
            token: token.clone(),
            user_email: email.to_string(),
            expires_at,
        })
        .await?;

    Ok(token)
}

pub async fn find_user_by_session(
    state: &AppState,
    token: &str,
) -> Result<Option<UserWithCompany>> {
    if let Some(session) = state.sessions.find_one(doc! { "token": token }).await? {
        let expires_at = session.expires_at.to_system_time();
        if expires_at <= SystemTime::now() {
            // Remove expired session, ignore result
            if let Some(id) = session.id {
                let _ = state.sessions.delete_one(doc! { "_id": id }).await;
            } else {
                let _ = state
                    .sessions
                    .delete_one(doc! { "token": &session.token })
                    .await;
            }
            return Ok(None);
        }
        find_user(state, &session.user_email).await
    } else {
        Ok(None)
    }
}

pub async fn list_users(state: &AppState) -> Result<Vec<UserWithCompany>> {
    let mut cursor = state.users.find(doc! {}).await?;
    let mut users = Vec::new();
    while let Some(user) = cursor.try_next().await? {
        users.push(build_user_with_company(state, user).await?);
    }
    Ok(users)
}

pub async fn get_user_by_id(state: &AppState, id: &ObjectId) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "_id": id }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_user(
    state: &AppState,
    email: &str,
    secret: &str,
    company_id: &ObjectId,
    role: UserRole,
) -> Result<ObjectId> {
    let res = state
        .users
        .insert_one(User {
            id: None,
            email: email.to_string(),
            secret: secret.to_string(),
            company_id: company_id.clone(),
            role,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("user insert missing _id")
}

pub async fn update_user(
    state: &AppState,
    id: &ObjectId,
    email: &str,
    secret: &str,
    company_id: &ObjectId,
    role: UserRole,
) -> Result<()> {
    state
        .users
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {"email": email, "secret": secret, "company": company_id, "role": role.as_str()} },
        )
        .await?;
    Ok(())
}

pub async fn delete_user(state: &AppState, id: &ObjectId) -> Result<()> {
    state.users.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn list_companies(state: &AppState) -> Result<Vec<Company>> {
    let mut cursor = state.companies.find(doc! {}).await?;
    let mut companies = Vec::new();
    while let Some(company) = cursor.try_next().await? {
        companies.push(company);
    }
    Ok(companies)
}

pub async fn get_company_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Company>> {
    state
        .companies
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_company(
    state: &AppState,
    name: &str,
    default_currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .companies
        .insert_one(Company {
            id: None,
            name: name.to_string(),
            default_currency: default_currency.to_string(),
            is_active,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("company insert missing _id")
}

pub async fn update_company(
    state: &AppState,
    id: &ObjectId,
    name: &str,
    default_currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<()> {
    state
        .companies
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "name": name,
                "default_currency": default_currency,
                "is_active": is_active,
                "notes": notes.clone(),
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_company(state: &AppState, id: &ObjectId) -> Result<()> {
    // If the company has dependent financial data, mark it inactive instead of deleting.
    let has_dependents = state
        .accounts
        .find_one(doc! { "company_id": id })
        .await?
        .is_some()
        || state
            .categories
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .contacts
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .recurring_plans
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .planned_entries
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .transactions
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .forecasts
            .find_one(doc! { "company_id": id })
            .await?
            .is_some();

    if has_dependents {
        state
            .companies
            .update_one(
                doc! { "_id": id },
                doc! { "$set": {
                    "is_active": false,
                    "updated_at": DateTime::from_system_time(SystemTime::now())
                } },
            )
            .await?;
        return Ok(());
    }

    state.companies.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

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
    state.recurring_plans.delete_one(doc! { "_id": id }).await?;
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

async fn build_user_with_company(state: &AppState, user: User) -> Result<UserWithCompany> {
    let id = user.id.context("user missing _id")?;
    let company = state
        .companies
        .find_one(doc! { "_id": &user.company_id })
        .await?
        .context("user references missing company")?;
    Ok(UserWithCompany {
        id,
        email: user.email,
        secret: user.secret,
        company_id: user.company_id,
        company_name: company.name,
        role: user.role,
    })
}

pub async fn delete_session(state: &AppState, token: &str) -> Result<()> {
    let _ = state.sessions.delete_one(doc! { "token": token }).await?;
    Ok(())
}

async fn company_default_currency(state: &AppState, company_id: &ObjectId) -> Result<String> {
    let company = state
        .companies
        .find_one(doc! { "_id": company_id })
        .await?
        .context("company not found for currency fallback")?;
    Ok(if company.default_currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        company.default_currency
    })
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

    let now = Utc::now();
    let due_dates = upcoming_due_dates(plan, months_ahead, now);

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
    now: ChronoDateTime<Utc>,
) -> Vec<DateTime> {
    let start = plan.start_date.to_chrono();
    let mut dates = Vec::new();
    let end_limit = plan.end_date.map(|d| d.to_chrono());

    match plan.frequency.to_lowercase().as_str() {
        "monthly" => {
            let anchor = align_to_day(start, plan.day_of_month);
            let base = if now.date_naive() > anchor.date_naive() {
                align_to_day(now, plan.day_of_month)
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
            while current < now {
                current = current + step;
            }
            for _ in 0..months_ahead {
                if let Some(end) = end_limit {
                    if current > end {
                        break;
                    }
                }
                dates.push(DateTime::from_chrono(current));
                current = current + step;
            }
        }
        "biweekly" => {
            let step = chrono::Duration::days(14);
            let mut current = start;
            while current < now {
                current = current + step;
            }
            for _ in 0..months_ahead {
                if let Some(end) = end_limit {
                    if current > end {
                        break;
                    }
                }
                dates.push(DateTime::from_chrono(current));
                current = current + step;
            }
        }
        _ => {
            let step = chrono::Duration::days(30);
            let mut current = if now > start { now } else { start };
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
