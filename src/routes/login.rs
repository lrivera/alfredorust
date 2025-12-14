// routes/login.rs
// POST /login { "email": "...", "code": "123456" } -> { "ok": true|false }

use axum::{
    extract::{Json, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::session::SESSION_COOKIE_NAME;
use crate::state::{AppState, SESSION_TTL_SECONDS, create_session, find_user};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub code: String,
}

/// Verifies the current TOTP code with a small skew (±1 step) defined in TOTP::new().
pub async fn login(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    match find_user(&st, &body.email).await {
        Ok(Some(user)) => match build_totp(&user.company_name, &user.email, &user.secret) {
            Ok(totp) => {
                let ok = totp.check_current(&body.code).unwrap_or(false);
                if ok {
                    match create_session(&st, &user.email).await {
                        Ok(token) => {
                            let mut response =
                                (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
                                    .into_response();
                            let host = headers
                                .get("host")
                                .and_then(|h| h.to_str().ok())
                                .unwrap_or("localhost");
                            // host-only cookie
                            let cookie_host_only = format!(
                                "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                                SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS
                            );
                            if let Ok(header_value) = HeaderValue::from_str(&cookie_host_only) {
                                response.headers_mut().append(SET_COOKIE, header_value);
                            }
                            // domain cookie (subdomain sharing)
                            if let Some(domain) = compute_cookie_domain(host) {
                                let cookie_domain = format!(
                                    "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Domain={}",
                                    SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS, domain
                                );
                                if let Ok(header_value) = HeaderValue::from_str(&cookie_domain) {
                                    response.headers_mut().append(SET_COOKIE, header_value);
                                }
                            }
                            // cookie for slug host (e.g., company.localhost)
                            let host_base = host.split(':').next().unwrap_or(host);
                            if !user.company_slug.is_empty() {
                                let slug_host = format!("{}.{}", user.company_slug, host_base);
                                let cookie_slug = format!(
                                    "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Domain={}",
                                    SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS, slug_host
                                );
                                if let Ok(header_value) = HeaderValue::from_str(&cookie_slug) {
                                    response.headers_mut().append(SET_COOKIE, header_value);
                                }
                            }
                            response
                        }
                        Err(e) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "error": format!("session error: {e}") })),
                        )
                            .into_response(),
                    }
                } else {
                    (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({ "ok": false })),
                    )
                        .into_response()
                }
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("db error: {e}") })),
        )
            .into_response(),
    }
}
pub fn compute_cookie_domain(host: &str) -> Option<String> {
    let base = host.split(':').next().unwrap_or(host);
    // Accept dot for sharing across subdomains; allow localhost
    if base == "localhost" || base.parse::<std::net::IpAddr>().is_ok() || base.contains('.') {
        Some(format!(".{}", base.trim_start_matches('.')))
    } else {
        None
    }
}
