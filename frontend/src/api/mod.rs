//! Typed client over the existing JSON API.
//!
//! All requests are same-origin so the HttpOnly session cookie is sent
//! automatically; the client never reads or stores the cookie or the TOTP
//! secret. Non-success responses map to explicit `ApiError` variants so the UI
//! can surface failures instead of silently falling back.
//!
//! Layout: this module holds the auth/session core and the shared request
//! helpers; the per-domain data types live in submodules:
//!   - [`finance`] — accounts, categories, contacts, transactions, recurring
//!     plans, planned entries, forecasts.
//!   - [`operations`] — orders, projects, resources, resource logs, concept
//!     statuses, project concepts, the hourly grid.
//!   - [`misc`] — timeline (tiempo) and CFDIs.
//!   - [`admin`] — companies, users, SAT configs, own account profile.

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

mod admin;
mod finance;
mod misc;
mod operations;

pub use admin::*;
pub use finance::*;
pub use misc::*;
pub use operations::*;

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

// --- shared request helpers ----------------------------------------------

pub(crate) fn transport(e: gloo_net::Error) -> ApiError {
    ApiError::Transport(e.to_string())
}

/// Maps an HTTP status to an error, or `None` when it is a success status.
pub(crate) fn err_for(status: u16) -> Option<ApiError> {
    match status {
        200 | 201 | 204 => None,
        401 => Some(ApiError::Unauthorized),
        403 => Some(ApiError::Forbidden),
        s => Some(ApiError::Status(s)),
    }
}

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
