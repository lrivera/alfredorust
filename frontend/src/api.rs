//! Typed client over the existing JSON API.
//!
//! All requests are same-origin so the HttpOnly session cookie is sent
//! automatically; the client never reads or stores the cookie or the TOTP
//! secret. Non-success responses map to explicit `ApiError` variants so the UI
//! can surface failures instead of silently falling back.

use gloo_net::http::Request;
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
