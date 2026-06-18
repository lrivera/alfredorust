//! Operations domain types: orders, projects (+ concepts and concept statuses),
//! resources, resource logs, and the hourly resource-usage grid. Pure data —
//! the pages drive them through the generic `get_json`/`post_json` helpers.

use serde::{Deserialize, Serialize};

// --- orders (service orders) ----------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OrderItem {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Order {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub status_label: String,
    pub amount: f64,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub contact_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrderPayload {
    pub title: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub account_id: Option<String>,
    pub status: String,
    pub amount: f64,
    pub scheduled_at: Option<String>,
    pub items: Vec<OrderItem>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OrderDetail {
    pub title: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub account_id: Option<String>,
    pub status: String,
    pub amount: f64,
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub items: Vec<OrderItem>,
    pub notes: Option<String>,
}

// --- projects -------------------------------------------------------------

/// A project for option pickers. `/api/admin/projects` returns `title`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectRow {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: String,
    #[serde(default)]
    pub status_label: String,
    pub priority: String,
    #[serde(default)]
    pub priority_label: String,
    #[serde(default)]
    pub total_budget: Option<f64>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub contact_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectPayload {
    pub title: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub description: Option<String>,
    pub priority: String,
    pub total_budget: Option<f64>,
    pub scheduled_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectDetail {
    pub title: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub description: Option<String>,
    pub priority: String,
    pub total_budget: Option<f64>,
    pub scheduled_at: Option<String>,
    pub notes: Option<String>,
}

// --- project concepts -----------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProjectConcept {
    pub id: String,
    pub status_id: String,
    pub name: String,
    pub quantity: f64,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub estimated_hours: Option<f64>,
    #[serde(default)]
    pub estimated_cost: Option<f64>,
    pub position: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectConceptPayload {
    pub status_id: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub estimated_hours: Option<f64>,
    pub estimated_cost: Option<f64>,
    pub notes: Option<String>,
    pub position: i32,
}

// --- concept statuses -----------------------------------------------------

/// Minimal status for option pickers (resources, concepts).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConceptStatus {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConceptStatusFull {
    pub id: String,
    pub name: String,
    pub position: i32,
    #[serde(default)]
    pub color: Option<String>,
    pub is_initial: bool,
    pub is_terminal: bool,
    pub is_cancelled: bool,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConceptStatusPayload {
    pub name: String,
    pub position: i32,
    pub color: Option<String>,
    pub is_initial: bool,
    pub is_terminal: bool,
    pub is_cancelled: bool,
    pub is_active: bool,
}

// --- resources ------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub resource_type: String,
    #[serde(default)]
    pub resource_type_label: String,
    pub is_active: bool,
    pub hourly_cost: f64,
    pub currency: String,
    #[serde(default)]
    pub allowed_status_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourcePayload {
    pub name: String,
    pub resource_type: String,
    pub is_active: bool,
    pub hourly_cost: f64,
    pub currency: Option<String>,
    pub allowed_status_ids: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResourceDetail {
    pub name: String,
    pub resource_type: String,
    pub is_active: bool,
    pub hourly_cost: f64,
    pub currency: String,
    #[serde(default)]
    pub allowed_status_ids: Vec<String>,
    pub notes: Option<String>,
}

// --- resource logs --------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResourceLog {
    pub id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub resource_name: Option<String>,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub duration_hours: Option<f64>,
    #[serde(default)]
    pub operator_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceLogPayload {
    pub project_id: Option<String>,
    pub phase: Option<String>,
    pub resource_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceLogEndPayload {
    pub ended_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResourceLogDetail {
    pub project_id: Option<String>,
    pub phase: Option<String>,
    pub resource_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
}

// --- resource usages hourly grid ------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridResource {
    pub resource_id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridCell {
    pub hour: i32,
    pub is_work_hour: bool,
    pub resources: Vec<GridResource>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridRow {
    pub concept_id: String,
    #[serde(default)]
    pub project_title: String,
    #[serde(default)]
    pub status_name: String,
    pub concept_name: String,
    pub quantity: f64,
    #[serde(default)]
    pub unit: String,
    pub cells: Vec<GridCell>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridStatus {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GridView {
    pub date: String,
    pub can_edit: bool,
    #[serde(default)]
    pub statuses: Vec<GridStatus>,
    pub rows: Vec<GridRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GridSelection {
    pub concept_id: String,
    pub hour: i32,
    pub resource_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GridSavePayload {
    pub date: String,
    pub status_id: Option<String>,
    pub selections: Vec<GridSelection>,
}
