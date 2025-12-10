// state.rs
// AppState encapsulating Mongo collections plus initialization and helpers.

use anyhow::{Context, Result};
use data_encoding::BASE32_NOPAD;
use futures::stream::TryStreamExt;
use mongodb::{
    Client, Collection, Database,
    bson::{DateTime, doc, oid::ObjectId},
};
use rand::RngCore;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    time::{Duration, SystemTime},
};

use crate::models::{
    Account, AccountType, Category, Company, Contact, ContactType, FlowType, Forecast, PlannedEntry,
    PlannedStatus, RecurringPlan, SeedUser, Session, Transaction, TransactionType, User, UserRole,
};

pub const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24; // 1 day

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
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_company(state: &AppState, id: &ObjectId) -> Result<()> {
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
    let res = state
        .accounts
        .insert_one(Account {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            account_type,
            currency: currency.to_string(),
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
    version: i32,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .recurring_plans
        .insert_one(RecurringPlan {
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
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("recurring plan insert missing _id")
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
    version: i32,
    notes: Option<String>,
) -> Result<()> {
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
                "end_date": end_date,
                "is_active": is_active,
                "version": version,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_recurring_plan(state: &AppState, id: &ObjectId) -> Result<()> {
    state
        .recurring_plans
        .delete_one(doc! { "_id": id })
        .await?;
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
    status: PlannedStatus,
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
            status,
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
    Ok(())
}

pub async fn delete_planned_entry(state: &AppState, id: &ObjectId) -> Result<()> {
    state
        .planned_entries
        .delete_one(doc! { "_id": id })
        .await?;
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

pub async fn get_transaction_by_id(
    state: &AppState,
    id: &ObjectId,
) -> Result<Option<Transaction>> {
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
    let res = state
        .transactions
        .insert_one(Transaction {
            id: None,
            company_id: company_id.clone(),
            date,
            description: description.to_string(),
            transaction_type,
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
    Ok(())
}

pub async fn delete_transaction(state: &AppState, id: &ObjectId) -> Result<()> {
    state
        .transactions
        .delete_one(doc! { "_id": id })
        .await?;
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
