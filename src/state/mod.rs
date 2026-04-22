// state module: AppState, initialization, and re-exports of submodules.

use anyhow::Result;
use mongodb::{Client, Collection};
use serde::Serialize;
use std::{collections::HashMap, env, sync::Arc};
use tokio::sync::Mutex;

use crate::models::{
    Account, Category, Company, Contact, Forecast, PlannedEntry, RecurringPlan, SatConfig,
    ServiceOrder, Session, Transaction, User, UserCompany,
};
use bson::Document;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum CfdiJobStatus {
    Running,
    Done {
        imported: usize,
        transactions_created: usize,
        transactions_updated: usize,
        transactions_skipped: usize,
        errors: Vec<String>,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CfdiJob {
    pub job_id: String,
    pub company_id: String,
    pub label: String,
    pub chunk_start: String,
    pub started_at: String,
    pub status: CfdiJobStatus,
}

pub type JobStore = Arc<Mutex<HashMap<String, CfdiJob>>>;

mod seed;
mod users;
mod companies;
mod finance;
mod orders;
mod sat_configs;

pub use users::*;
pub use companies::*;
pub use finance::*;
pub use orders::*;
pub use sat_configs::*;

pub const SESSION_TTL_SECONDS: u64 = 60 * 60 * 24; // 1 day
pub const PLANNED_MONTHS_AHEAD: u32 = 24;

#[derive(Clone)]
pub struct AppState {
    pub jobs: JobStore,
    pub users: Collection<User>,
    pub user_companies: Collection<UserCompany>,
    pub companies: Collection<Company>,
    pub sessions: Collection<Session>,
    pub accounts: Collection<Account>,
    pub categories: Collection<Category>,
    pub contacts: Collection<Contact>,
    pub recurring_plans: Collection<RecurringPlan>,
    pub planned_entries: Collection<PlannedEntry>,
    pub transactions: Collection<Transaction>,
    pub forecasts: Collection<Forecast>,
    pub cfdis: Collection<Document>,
    pub sat_configs: Collection<SatConfig>,
    pub orders: Collection<ServiceOrder>,
}

pub async fn init_state() -> Result<AppState> {
    let uri = env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let db_name = env::var("MONGODB_DB").unwrap_or_else(|_| "totp".to_string());

    let client = Client::with_uri_str(uri).await?;
    let db = client.database(&db_name);

    seed::ensure_collections(&db).await?;

    // Only seed when the database is effectively empty (no users).
    if seed::is_database_empty(&db).await? {
        let default_users = seed::load_default_users()?;
        let company_names = seed::derive_company_names(&default_users);
        let company_ids = seed::seed_default_companies(&db, &company_names).await?;
        seed::seed_default_users(&db, &default_users, &company_ids).await?;
        seed::seed_sample_finance(&db, company_ids.values().next().cloned()).await?;
    }

    Ok(AppState {
        jobs: Arc::new(Mutex::new(HashMap::new())),
        users: db.collection::<User>("users"),
        user_companies: db.collection::<UserCompany>("user_companies"),
        companies: db.collection::<Company>("company"),
        sessions: db.collection::<Session>("sessions"),
        accounts: db.collection::<Account>("accounts"),
        categories: db.collection::<Category>("categories"),
        contacts: db.collection::<Contact>("contacts"),
        recurring_plans: db.collection::<RecurringPlan>("recurring_plans"),
        planned_entries: db.collection::<PlannedEntry>("planned_entries"),
        transactions: db.collection::<Transaction>("transactions"),
        forecasts: db.collection::<Forecast>("forecasts"),
        cfdis: db.collection::<Document>("cfdis"),
        sat_configs: db.collection::<SatConfig>("sat_configs"),
        orders: db.collection::<ServiceOrder>("service_orders"),
    })
}
