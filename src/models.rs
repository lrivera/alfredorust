// models.rs
// Domain models for auth/multitenancy and finance entities (MongoDB).

use mongodb::bson::{DateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

/// ---------- AUTH / PLATFORM LAYER ----------

/// User roles for authorization (system-level).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Staff,
}

impl UserRole {
    pub fn default_admin() -> Self {
        UserRole::Admin
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Staff => "staff",
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Staff
    }
}

/// Seed user definition as stored in users.json (company referenced by name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedUser {
    pub email: String,
    pub secret: String,
    pub company: String,
    #[serde(default = "UserRole::default_admin")]
    pub role: UserRole,
}

/// Company document stored in MongoDB.
/// This acts as the "tenant" or organization for all finance entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,

    /// Default currency for this company (tenant), e.g. "MXN", "USD".
    #[serde(default)]
    pub default_currency: String,

    /// Whether this company is active.
    #[serde(default = "default_true")]
    pub is_active: bool,

    /// Optional timestamps (you can fill these when inserting/updating).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    /// Optional notes / description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

fn default_true() -> bool {
    true
}

/// User document stored in MongoDB referencing the company by ObjectId.
/// Each user belongs to exactly one company (tenant) in this first version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    pub secret: String,

    /// Tenant the user belongs to.
    #[serde(rename = "company")]
    pub company_id: ObjectId,

    pub role: UserRole,
}

/// Session document stored in MongoDB linking a token to a user and expiry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub token: String,
    pub user_email: String,
    pub expires_at: DateTime,
}

/// ---------- SHARED ENUMS FOR FINANCE DOMAIN ----------

/// Basic income/expense kind used by categories, recurring plans, planned entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlowType {
    Income,
    Expense,
}

impl FlowType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlowType::Income => "income",
            FlowType::Expense => "expense",
        }
    }
}

/// Account type (bank, cash, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Bank,
    Cash,
    CreditCard,
    Investment,
    Other,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Bank => "bank",
            AccountType::Cash => "cash",
            AccountType::CreditCard => "credit_card",
            AccountType::Investment => "investment",
            AccountType::Other => "other",
        }
    }
}

/// Transaction type: income, expense or internal transfer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Income,
    Expense,
    Transfer,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Income => "income",
            TransactionType::Expense => "expense",
            TransactionType::Transfer => "transfer",
        }
    }
}

/// Contact type (customer, supplier, service, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContactType {
    Customer,
    Supplier,
    Service,
    Other,
}

impl ContactType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContactType::Customer => "customer",
            ContactType::Supplier => "supplier",
            ContactType::Service => "service",
            ContactType::Other => "other",
        }
    }
}

/// Status of a planned entry (commitment/budget item).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlannedStatus {
    Planned,
    PartiallyCovered,
    Covered,
    Overdue,
    Cancelled,
}

impl PlannedStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlannedStatus::Planned => "planned",
            PlannedStatus::PartiallyCovered => "partially_covered",
            PlannedStatus::Covered => "covered",
            PlannedStatus::Overdue => "overdue",
            PlannedStatus::Cancelled => "cancelled",
        }
    }
}

/// ---------- FINANCE ENTITIES (SCOPED BY COMPANY/TENANT) ----------

/// Financial account (bank account, cash, credit card, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this account belongs to.
    pub company_id: ObjectId,

    pub name: String,
    pub account_type: AccountType,

    /// ISO-like currency code, e.g. "MXN", "USD".
    pub currency: String,

    #[serde(default = "default_true")]
    pub is_active: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Category for incomes/expenses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this category belongs to.
    pub company_id: ObjectId,

    pub name: String,
    pub flow_type: FlowType,

    /// Optional parent category for hierarchy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Contact: customer, supplier, service (CFE, landlord, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this contact belongs to.
    pub company_id: ObjectId,

    pub name: String,
    pub contact_type: ContactType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// RecurringPlan: template for recurring income/expense,
/// e.g. "Electricity CFE every month on day 10, estimated 2000 MXN".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringPlan {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this plan belongs to.
    pub company_id: ObjectId,

    pub name: String,
    pub flow_type: FlowType,

    pub category_id: ObjectId,
    pub account_expected_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    pub amount_estimated: f64,

    /// Frequency: usually "monthly", "weekly", "yearly".
    pub frequency: String,

    /// Day of month (1–31) if frequency is monthly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_of_month: Option<i32>,

    pub start_date: DateTime,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<DateTime>,

    /// Whether this recurring plan is active.
    #[serde(default = "default_true")]
    pub is_active: bool,

    /// Version number for detecting outdated PlannedEntries.
    #[serde(default = "default_one")]
    pub version: i32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

fn default_one() -> i32 {
    1
}

/// PlannedEntry: concrete commitment/budget item with a due date,
/// used by the "traffic light" (semaphore) and to match with real transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedEntry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this planned entry belongs to.
    pub company_id: ObjectId,

    /// Optional link to the recurring plan that generated this entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurring_plan_id: Option<ObjectId>,

    /// Version of the RecurringPlan at the time this entry was generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurring_plan_version: Option<i32>,

    pub name: String,
    pub flow_type: FlowType,

    pub category_id: ObjectId,
    pub account_expected_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    pub amount_estimated: f64,

    /// Due date of this specific commitment (e.g. 10th of November).
    pub due_date: DateTime,

    pub status: PlannedStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Transaction: real movement (income, expense, transfer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this transaction belongs to.
    pub company_id: ObjectId,

    pub date: DateTime,
    pub description: String,

    pub transaction_type: TransactionType,
    pub category_id: ObjectId,

    /// For expenses or transfers (money goes out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_from_id: Option<ObjectId>,

    /// For incomes or transfers (money goes in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_to_id: Option<ObjectId>,

    pub amount: f64,

    /// Optional link to the planned entry this transaction is covering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_entry_id: Option<ObjectId>,

    #[serde(default = "default_true")]
    pub is_confirmed: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Forecast: optional snapshot of a projection (3, 6, 12 months, scenarios, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Tenant this forecast belongs to.
    pub company_id: ObjectId,

    pub generated_at: DateTime,

    /// Optional user that generated this forecast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by_user_id: Option<ObjectId>,

    pub start_date: DateTime,
    pub end_date: DateTime,
    pub currency: String,

    pub projected_income_total: f64,
    pub projected_expense_total: f64,
    pub projected_net: f64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_balance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_balance: Option<f64>,

    /// Optional JSON/text field to store detailed breakdown
    /// (per month, per category, scenario parameters, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// Optional scenario name, e.g. "base", "reduce_restaurants_20".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}
