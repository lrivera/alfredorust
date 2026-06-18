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
    pub username: String,
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

    /// Id of the tenant the session is scoped to (the `active` company), falling
    /// back to the first membership.
    pub fn active_company_id(&self) -> Option<String> {
        self.companies
            .iter()
            .find(|c| c.active)
            .or_else(|| self.companies.first())
            .map(|c| c.id.clone())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    /// Missing or expired session — the SPA must route to login.
    Unauthorized,
    /// Authenticated but not allowed — show an explicit not-allowed state.
    Forbidden,
    /// Any other non-success status, with the server's `{"error": ...}` message
    /// when it sent one (so the UI can show the real reason).
    Status { code: u16, message: Option<String> },
    /// Transport/parse failure.
    Transport(String),
}

/// Decide what to show the user for an error: the server's own message when it
/// sent one, friendly text for auth/network, otherwise the per-call `fallback`.
/// Centralizes the "which error do I show?" logic so screens don't each guess.
pub fn humanize(err: &ApiError, fallback: &str) -> String {
    match err {
        ApiError::Forbidden => "No tienes permiso para esta acción".to_string(),
        ApiError::Unauthorized => "Tu sesión expiró. Vuelve a iniciar sesión.".to_string(),
        ApiError::Transport(_) => "Error de conexión. Intenta de nuevo.".to_string(),
        ApiError::Status { message: Some(m), .. } if !m.trim().is_empty() => m.clone(),
        ApiError::Status { .. } => fallback.to_string(),
    }
}

/// Build an `ApiError` from a non-success response, reading the `{"error": ...}`
/// body so the message isn't lost.
pub(crate) async fn err_with_body(resp: gloo_net::http::Response) -> ApiError {
    match resp.status() {
        401 => ApiError::Unauthorized,
        403 => ApiError::Forbidden,
        code => {
            let message = resp
                .text()
                .await
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .filter(|s| !s.is_empty());
            ApiError::Status { code, message }
        }
    }
}

#[derive(Serialize)]
struct LoginBody<'a> {
    username: &'a str,
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
        code => Err(ApiError::Status { code, message: None }),
    }
}

/// Passwordless TOTP login. The server sets the session cookie on success.
pub async fn login(username: &str, code: &str) -> Result<LoginOk, ApiError> {
    let resp = Request::post("/login")
        .json(&LoginBody { username, code })
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
        code => Err(ApiError::Status { code, message: None }),
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

fn is_success(status: u16) -> bool {
    matches!(status, 200 | 201 | 202 | 204)
}

/// GET a JSON resource. Unknown fields in the response are ignored, so SPA
/// structs can declare just the fields they render.
pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, ApiError> {
    let resp = Request::get(url).send().await.map_err(transport)?;
    if !is_success(resp.status()) {
        return Err(err_with_body(resp).await);
    }
    resp.json().await.map_err(transport)
}

/// POST a JSON body. On failure carries the server's `{error}` message.
pub async fn post_json<B: Serialize>(url: &str, body: &B) -> Result<(), ApiError> {
    let resp = Request::post(url)
        .json(body)
        .map_err(transport)?
        .send()
        .await
        .map_err(transport)?;
    if is_success(resp.status()) {
        Ok(())
    } else {
        Err(err_with_body(resp).await)
    }
}

/// POST with no body (e.g. delete/generate endpoints). On failure carries the
/// server's `{error}` message.
pub async fn post_empty(url: &str) -> Result<(), ApiError> {
    let resp = Request::post(url).send().await.map_err(transport)?;
    if is_success(resp.status()) {
        Ok(())
    } else {
        Err(err_with_body(resp).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    fn me_with(perms: &[&str]) -> Me {
        Me {
            username: "u@example.com".into(),
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

    #[wasm_bindgen_test]
    fn humanize_prefers_server_message_then_friendly_then_fallback() {
        // Server's own message wins.
        assert_eq!(
            humanize(
                &ApiError::Status { code: 400, message: Some("recurring plan is inactive".into()) },
                "fallback"
            ),
            "recurring plan is inactive"
        );
        // No body → per-call fallback.
        assert_eq!(
            humanize(&ApiError::Status { code: 500, message: None }, "No se pudo guardar"),
            "No se pudo guardar"
        );
        // Auth/transport get friendly text regardless of fallback.
        assert_eq!(humanize(&ApiError::Forbidden, "x"), "No tienes permiso para esta acción");
        assert!(humanize(&ApiError::Unauthorized, "x").contains("sesión"));
        assert!(humanize(&ApiError::Transport("boom".into()), "x").contains("conexión"));
        // Empty server message falls back too.
        assert_eq!(
            humanize(&ApiError::Status { code: 400, message: Some("  ".into()) }, "fb"),
            "fb"
        );
    }
}
