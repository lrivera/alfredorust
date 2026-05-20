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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum UserPermission {
    ViewProjects,
    ViewProjectMoney,
    EditResourceUsageToday,
    ViewResourceUsageHistory,
    ViewTimeline,
}

impl UserPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserPermission::ViewProjects => "view_projects",
            UserPermission::ViewProjectMoney => "view_project_money",
            UserPermission::EditResourceUsageToday => "edit_resource_usage_today",
            UserPermission::ViewResourceUsageHistory => "view_resource_usage_history",
            UserPermission::ViewTimeline => "view_timeline",
        }
    }
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
    /// Primary company by name (backwards compatibility).
    pub company: String,
    /// Additional companies by name (including the primary if you want).
    #[serde(default)]
    pub companies: Vec<String>,
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

    /// URL-friendly identifier; must be unique.
    #[serde(default)]
    pub slug: String,

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

fn default_zero() -> f64 {
    0.0
}

fn default_mxn() -> String {
    "MXN".to_string()
}

/// User document stored in MongoDB referencing the company by ObjectId.
/// Each user belongs to exactly one company (tenant) in this first version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    pub secret: String,

    /// Primary tenant the user belongs to (for compatibility).
    #[serde(rename = "company", skip_serializing_if = "Option::is_none")]
    pub company_id: Option<ObjectId>,

    /// All companies the user can access.
    #[serde(rename = "companies", default)]
    pub company_ids: Vec<ObjectId>,
}

/// User-company membership with per-company role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCompany {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub company_id: ObjectId,
    pub role: UserRole,

    #[serde(default)]
    pub permissions: Vec<UserPermission>,
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
    pub rfc: Option<String>,

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

    /// Optional link to the service order that generated this entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_order_id: Option<ObjectId>,

    /// Optional project this commitment belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ObjectId>,

    /// Optional estimated commitment that this real commitment helps cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_planned_entry_id: Option<ObjectId>,

    pub name: String,
    pub flow_type: FlowType,

    pub category_id: ObjectId,
    pub account_expected_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    pub amount_estimated: f64,

    /// Snapshot of amount_estimated before the first real payment aligned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_amount_estimated: Option<f64>,

    /// Due date of this specific commitment (e.g. 10th of November).
    pub due_date: DateTime,

    /// Snapshot of due_date before the first real payment aligned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_due_date: Option<DateTime>,

    pub status: PlannedStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// UUID of the CFDI/factura that originated this commitment, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfdi_uuid: Option<String>,

    /// Currency code from the CFDI (e.g. "MXN", "USD").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Serie-Folio of the CFDI (e.g. "REGT-474850").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfdi_folio: Option<String>,
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

    /// Optional project this real movement belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ObjectId>,

    #[serde(default = "default_true")]
    pub is_confirmed: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,

    /// Contact (client/supplier) linked to this transaction, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    /// UUID of the CFDI/factura that originated this transaction, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfdi_uuid: Option<String>,

    /// Currency code from the CFDI (e.g. "MXN", "USD").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Serie-Folio of the CFDI (e.g. "REGT-474850").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfdi_folio: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// ---------- SERVICE ORDERS ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    InProgress,
    Completed,
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Confirmed => "confirmed",
            OrderStatus::InProgress => "in_progress",
            OrderStatus::Completed => "completed",
            OrderStatus::Cancelled => "cancelled",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "Pendiente",
            OrderStatus::Confirmed => "Confirmada",
            OrderStatus::InProgress => "En proceso",
            OrderStatus::Completed => "Completada",
            OrderStatus::Cancelled => "Cancelada",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
}

impl OrderItem {
    pub fn subtotal(&self) -> f64 {
        self.quantity * self.unit_price
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceOrder {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_entry_id: Option<ObjectId>,

    pub title: String,
    pub status: OrderStatus,
    pub amount: f64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<ObjectId>,

    #[serde(default)]
    pub items: Vec<OrderItem>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
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

/// ---------- SAT ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatConfig {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub company_id: ObjectId,
    pub rfc: String,
    pub cer_path: String,
    pub key_path: String,
    pub key_password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at: DateTime,
}

/// ---------- PROJECTS ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Pedidos,
    Analisis,
    Cotizado,
    OrdenDeCompraMandada,
    Ingenieria,
    Produccion,
    Calidad,
    EsperandoEntrega,
    Entregado,
    Cancelado,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectStatus::Pedidos => "pedidos",
            ProjectStatus::Analisis => "analisis",
            ProjectStatus::Cotizado => "cotizado",
            ProjectStatus::OrdenDeCompraMandada => "orden_de_compra_mandada",
            ProjectStatus::Ingenieria => "ingenieria",
            ProjectStatus::Produccion => "produccion",
            ProjectStatus::Calidad => "calidad",
            ProjectStatus::EsperandoEntrega => "esperando_entrega",
            ProjectStatus::Entregado => "entregado",
            ProjectStatus::Cancelado => "cancelado",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            ProjectStatus::Pedidos => "Pedidos",
            ProjectStatus::Analisis => "Análisis",
            ProjectStatus::Cotizado => "Cotizado",
            ProjectStatus::OrdenDeCompraMandada => "Orden de Compra Mandada",
            ProjectStatus::Ingenieria => "Ingeniería",
            ProjectStatus::Produccion => "Producción",
            ProjectStatus::Calidad => "Calidad",
            ProjectStatus::EsperandoEntrega => "Esperando Entrega",
            ProjectStatus::Entregado => "Entregado",
            ProjectStatus::Cancelado => "Cancelado",
        }
    }
    pub fn next(&self) -> Option<Self> {
        match self {
            ProjectStatus::Pedidos => Some(ProjectStatus::Analisis),
            ProjectStatus::Analisis => Some(ProjectStatus::Cotizado),
            ProjectStatus::Cotizado => Some(ProjectStatus::OrdenDeCompraMandada),
            ProjectStatus::OrdenDeCompraMandada => Some(ProjectStatus::Ingenieria),
            ProjectStatus::Ingenieria => Some(ProjectStatus::Produccion),
            ProjectStatus::Produccion => Some(ProjectStatus::Calidad),
            ProjectStatus::Calidad => Some(ProjectStatus::EsperandoEntrega),
            ProjectStatus::EsperandoEntrega => Some(ProjectStatus::Entregado),
            ProjectStatus::Entregado => None,
            ProjectStatus::Cancelado => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProjectPriority {
    Low,
    #[default]
    Medium,
    High,
    Urgent,
}

impl ProjectPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectPriority::Low => "low",
            ProjectPriority::Medium => "medium",
            ProjectPriority::High => "high",
            ProjectPriority::Urgent => "urgent",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            ProjectPriority::Low => "Baja",
            ProjectPriority::Medium => "Media",
            ProjectPriority::High => "Alta",
            ProjectPriority::Urgent => "Urgente",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<ObjectId>,

    pub title: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub status: ProjectStatus,

    #[serde(default)]
    pub priority: ProjectPriority,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_budget: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

/// ---------- CONFIGURABLE CONCEPT STATUSES ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptStatus {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,
    pub name: String,
    pub position: i32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(default)]
    pub is_initial: bool,

    #[serde(default)]
    pub is_terminal: bool,

    #[serde(default)]
    pub is_cancelled: bool,

    #[serde(default = "default_true")]
    pub is_active: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

/// A line item/part inside a project, e.g. "Engrane A, 5 piezas".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConcept {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,
    pub project_id: ObjectId,
    pub status_id: ObjectId,
    pub name: String,

    #[serde(default = "default_one_f64")]
    pub quantity: f64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_hours: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default)]
    pub position: i32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

fn default_one_f64() -> f64 {
    1.0
}

/// ---------- RESOURCES ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Machinery,
    Vehicle,
    Equipment,
    Other,
}

impl ResourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Machinery => "machinery",
            ResourceType::Vehicle => "vehicle",
            ResourceType::Equipment => "equipment",
            ResourceType::Other => "other",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::Machinery => "Maquinaria",
            ResourceType::Vehicle => "Vehículo",
            ResourceType::Equipment => "Equipo",
            ResourceType::Other => "Otro",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,

    pub name: String,

    pub resource_type: ResourceType,

    #[serde(default = "default_true")]
    pub is_active: bool,

    #[serde(default = "default_zero")]
    pub hourly_cost: f64,

    #[serde(default = "default_mxn")]
    pub currency: String,

    /// If empty, this resource is available for every concept status.
    #[serde(default)]
    pub allowed_status_ids: Vec<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

/// ---------- RESOURCE LOGS ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLog {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<ObjectId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,

    pub started_at: DateTime,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_hours: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
}

/// Real time window where one resource was used. Cost is snapshotted here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,
    pub resource_id: ObjectId,
    pub resource_name_snapshot: String,
    pub started_at: DateTime,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_hours: Option<f64>,

    #[serde(default = "default_zero")]
    pub hourly_cost_snapshot: f64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<f64>,

    #[serde(default = "default_mxn")]
    pub currency: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

/// Cost/time allocation from one resource usage to one project concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageAllocation {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub company_id: ObjectId,
    pub usage_id: ObjectId,
    pub project_id: ObjectId,
    pub concept_id: ObjectId,
    pub allocation_ratio: f64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocated_hours: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocated_cost: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_role_defaults_to_admin_and_maps_strings() {
        assert_eq!(UserRole::default(), UserRole::Staff);
        assert!(UserRole::Admin.is_admin());
        assert!(!UserRole::Staff.is_admin());
        assert_eq!(UserRole::Admin.as_str(), "admin");
        assert_eq!(UserRole::Staff.as_str(), "staff");
    }

    #[test]
    fn order_item_subtotal_multiplies_quantity_and_price() {
        let item = OrderItem {
            description: "Part".into(),
            quantity: 3.0,
            unit_price: 12.5,
        };

        assert_eq!(item.subtotal(), 37.5);
    }

    #[test]
    fn project_status_advances_until_terminal_states() {
        assert_eq!(ProjectStatus::Pedidos.next(), Some(ProjectStatus::Analisis));
        assert_eq!(
            ProjectStatus::EsperandoEntrega.next(),
            Some(ProjectStatus::Entregado)
        );
        assert_eq!(ProjectStatus::Entregado.next(), None);
        assert_eq!(ProjectStatus::Cancelado.next(), None);
    }

    #[test]
    fn project_priority_and_resource_type_expose_form_values() {
        assert_eq!(ProjectPriority::Urgent.as_str(), "urgent");
        assert_eq!(ProjectPriority::Urgent.label(), "Urgente");
        assert_eq!(ResourceType::Vehicle.as_str(), "vehicle");
        assert_eq!(ResourceType::Vehicle.label(), "Vehículo");
    }
}
