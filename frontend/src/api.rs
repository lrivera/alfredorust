//! Typed client over the existing JSON API.
//!
//! All requests are same-origin so the HttpOnly session cookie is sent
//! automatically; the client never reads or stores the cookie or the TOTP
//! secret. Non-success responses map to explicit `ApiError` variants so the UI
//! can surface failures instead of silently falling back.

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A company the user belongs to. Mirrors the backend `CompanySummary`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub active: bool,
}

/// Bootstrap payload from `GET /api/me`. Mirrors the backend `MeResponse`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Me {
    pub email: String,
    pub company: String,
    pub company_slug: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub companies: Vec<Company>,
}

impl Me {
    /// UX-only permission check; the server remains the authorization boundary.
    pub fn can(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    /// Missing or expired session — the SPA must route to login.
    Unauthorized,
    /// Authenticated but not allowed — show an explicit not-allowed state.
    Forbidden,
    /// Any other non-success status.
    Status(u16),
    /// Transport/parse failure.
    Transport(String),
}

#[derive(Serialize)]
struct LoginBody<'a> {
    email: &'a str,
    code: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoginOk {
    pub ok: bool,
    #[serde(default)]
    pub redirect_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    fn me_with(perms: &[&str]) -> Me {
        Me {
            email: "u@example.com".into(),
            company: "Acme".into(),
            company_slug: "acme".into(),
            role: "admin".into(),
            permissions: perms.iter().map(|p| p.to_string()).collect(),
            companies: vec![],
        }
    }

    #[wasm_bindgen_test]
    fn can_is_true_for_granted_permission() {
        let me = me_with(&["view_projects", "view_timeline"]);
        assert!(me.can("view_projects"));
        assert!(me.can("view_timeline"));
    }

    #[wasm_bindgen_test]
    fn can_is_false_for_missing_permission() {
        let me = me_with(&["view_projects"]);
        assert!(!me.can("edit_resource_usage_today"));
        assert!(!me_with(&[]).can("view_projects"));
    }
}

/// Bootstrap the current session. `401` means anonymous.
pub async fn get_me() -> Result<Me, ApiError> {
    let resp = Request::get("/api/me")
        .send()
        .await
        .map_err(|e| ApiError::Transport(e.to_string()))?;
    match resp.status() {
        200 => resp
            .json::<Me>()
            .await
            .map_err(|e| ApiError::Transport(e.to_string())),
        401 => Err(ApiError::Unauthorized),
        403 => Err(ApiError::Forbidden),
        s => Err(ApiError::Status(s)),
    }
}

/// Passwordless TOTP login. The server sets the session cookie on success.
pub async fn login(email: &str, code: &str) -> Result<LoginOk, ApiError> {
    let resp = Request::post("/login")
        .json(&LoginBody { email, code })
        .map_err(|e| ApiError::Transport(e.to_string()))?
        .send()
        .await
        .map_err(|e| ApiError::Transport(e.to_string()))?;
    match resp.status() {
        200 => resp
            .json::<LoginOk>()
            .await
            .map_err(|e| ApiError::Transport(e.to_string())),
        401 => Err(ApiError::Unauthorized),
        s => Err(ApiError::Status(s)),
    }
}

/// End the session server-side.
pub async fn logout() -> Result<(), ApiError> {
    Request::post("/logout")
        .send()
        .await
        .map_err(|e| ApiError::Transport(e.to_string()))?;
    Ok(())
}

// --- shared helpers -------------------------------------------------------

fn transport(e: gloo_net::Error) -> ApiError {
    ApiError::Transport(e.to_string())
}

/// Maps an HTTP status to an error, or `None` when it is a success status.
fn err_for(status: u16) -> Option<ApiError> {
    match status {
        200 | 201 | 204 => None,
        401 => Some(ApiError::Unauthorized),
        403 => Some(ApiError::Forbidden),
        s => Some(ApiError::Status(s)),
    }
}

// --- accounts (finance) ---------------------------------------------------

/// A finance account row. Mirrors the backend `AccountRow`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub company: String,
    pub account_type: String,
    pub currency: String,
    pub is_active: bool,
}

/// Create/update payload. Mirrors `AccountCreatePayload`/`AccountUpdatePayload`.
#[derive(Clone, Debug, Serialize)]
pub struct AccountPayload {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
}

pub async fn list_accounts() -> Result<Vec<Account>, ApiError> {
    let resp = Request::get("/api/admin/accounts")
        .send()
        .await
        .map_err(transport)?;
    if let Some(e) = err_for(resp.status()) {
        return Err(e);
    }
    resp.json().await.map_err(transport)
}

pub async fn create_account(payload: &AccountPayload) -> Result<(), ApiError> {
    let resp = Request::post("/api/admin/accounts")
        .json(payload)
        .map_err(transport)?
        .send()
        .await
        .map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

pub async fn update_account(id: &str, payload: &AccountPayload) -> Result<(), ApiError> {
    let resp = Request::post(&format!("/api/admin/accounts/{id}/update"))
        .json(payload)
        .map_err(transport)?
        .send()
        .await
        .map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

pub async fn delete_account(id: &str) -> Result<(), ApiError> {
    let resp = Request::post(&format!("/api/admin/accounts/{id}/delete"))
        .send()
        .await
        .map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

// --- generic JSON helpers (used by the finance pages) ---------------------

/// GET a JSON resource. Unknown fields in the response are ignored, so SPA
/// structs can declare just the fields they render.
pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, ApiError> {
    let resp = Request::get(url).send().await.map_err(transport)?;
    if let Some(e) = err_for(resp.status()) {
        return Err(e);
    }
    resp.json().await.map_err(transport)
}

/// POST a JSON body, expecting a success status (ignores the response body).
pub async fn post_json<B: Serialize>(url: &str, body: &B) -> Result<(), ApiError> {
    let resp = Request::post(url)
        .json(body)
        .map_err(transport)?
        .send()
        .await
        .map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

/// POST with no body (e.g. delete/generate endpoints).
pub async fn post_empty(url: &str) -> Result<(), ApiError> {
    let resp = Request::post(url).send().await.map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

// --- categories -----------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub flow_type: String,
    #[serde(default)]
    pub parent: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CategoryPayload {
    pub name: String,
    pub flow_type: String,
    pub parent_id: Option<String>,
    pub notes: Option<String>,
}

// --- contacts -------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContactPayload {
    pub name: String,
    pub contact_type: String,
    pub rfc: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

// --- forecasts ------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Forecast {
    pub id: String,
    pub currency: String,
    pub projected_net: f64,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub scenario_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForecastPayload {
    pub generated_at: String,
    pub start_date: String,
    pub end_date: String,
    pub currency: String,
    pub projected_income_total: f64,
    pub projected_expense_total: f64,
    pub projected_net: f64,
    pub initial_balance: Option<f64>,
    pub final_balance: Option<f64>,
    pub details: Option<String>,
    pub scenario_name: Option<String>,
    pub notes: Option<String>,
}

// --- recurring plans ------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RecurringPlan {
    pub id: String,
    pub name: String,
    pub flow_type: String,
    pub amount_estimated: f64,
    pub frequency: String,
    pub start_date: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecurringPlanPayload {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub frequency: String,
    pub day_of_month: Option<i32>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
    pub version: i32,
    pub notes: Option<String>,
}

// --- planned entries ------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlannedEntry {
    pub id: String,
    pub name: String,
    pub flow_type: String,
    pub amount_estimated: f64,
    pub due_date: String,
    pub status: String,
    #[serde(default)]
    pub status_label: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlannedEntryPayload {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub due_date: String,
    pub status: String,
    pub project_id: Option<String>,
    pub notes: Option<String>,
}

/// Payload for paying a single planned entry. Mirrors `PlannedEntryPayPayload`.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedEntryPayPayload {
    pub paid_at: String,
    pub amount: f64,
    pub account_id: String,
    pub project_id: Option<String>,
    pub notes: Option<String>,
}

/// Payload for paying several planned entries at once. Mirrors
/// `PlannedEntryBulkPayPayload`.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedEntryBulkPayPayload {
    pub entry_ids: Vec<String>,
    pub paid_at: String,
    pub account_id: String,
    pub project_id: Option<String>,
    pub notes: Option<String>,
}

/// A project, for option pickers. `/api/admin/projects` returns `title`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub title: String,
}

// --- transactions ---------------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Transaction {
    pub id: String,
    pub date: String,
    pub description: String,
    pub tx_type: String,
    pub amount: f64,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub account_from: String,
    #[serde(default)]
    pub account_to: String,
    pub is_confirmed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TransactionPayload {
    pub date: String,
    pub description: String,
    pub transaction_type: String,
    pub category_id: String,
    pub account_from_id: Option<String>,
    pub account_to_id: Option<String>,
    pub amount: f64,
    pub planned_entry_id: Option<String>,
    pub is_confirmed: bool,
    pub notes: Option<String>,
}

// --- detail structs (used to repopulate edit forms losslessly) ------------
// Lean: only the fields the edit forms write back. Unknown fields are ignored.

#[derive(Clone, Debug, Deserialize)]
pub struct AccountDetail {
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub is_active: bool,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CategoryDetail {
    pub name: String,
    pub flow_type: String,
    pub parent_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ContactDetail {
    pub name: String,
    pub contact_type: String,
    pub rfc: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ForecastDetail {
    pub generated_at: String,
    pub start_date: String,
    pub end_date: String,
    pub currency: String,
    pub projected_income_total: f64,
    pub projected_expense_total: f64,
    pub initial_balance: Option<f64>,
    pub final_balance: Option<f64>,
    pub details: Option<String>,
    pub scenario_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RecurringPlanDetail {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub frequency: String,
    pub day_of_month: Option<i32>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
    pub version: i32,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlannedEntryDetail {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub due_date: String,
    pub status: String,
    pub project_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TransactionDetail {
    pub date: String,
    pub description: String,
    pub transaction_type: String,
    pub category_id: String,
    pub account_from_id: Option<String>,
    pub account_to_id: Option<String>,
    pub amount: f64,
    pub planned_entry_id: Option<String>,
    pub is_confirmed: bool,
    pub notes: Option<String>,
}

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
