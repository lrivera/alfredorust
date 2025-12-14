// routes/login.rs
// POST /login { "email": "...", "code": "123456" } -> { "ok": true|false }

use axum::{
    extract::{Json, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::{net::IpAddr, sync::Arc};

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
                            let redirect_url = compute_redirect_url(
                                headers
                                    .get("host")
                                    .and_then(|h| h.to_str().ok())
                                    .unwrap_or("localhost"),
                                &user.company_slug,
                            );
                            let mut response = (
                                StatusCode::OK,
                                Json(serde_json::json!({
                                    "ok": true,
                                    "redirect_url": redirect_url
                                })),
                            )
                                .into_response();
                            let host = headers
                                .get("host")
                                .and_then(|h| h.to_str().ok())
                                .unwrap_or("localhost");
                            set_cookies_for_host(
                                &mut response,
                                &token,
                                host,
                                &user.company_slug,
                            );
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
fn set_cookies_for_host(response: &mut Response, token: &str, host: &str, slug: &str) {
    let host_base = host.split(':').next().unwrap_or(host).trim_start_matches('.');

    // Host-only cookie (current host)
    if let Ok(header_value) = HeaderValue::from_str(&format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS
    )) {
        response.headers_mut().append(SET_COOKIE, header_value);
    }

    // Domain cookies for the current base host (with and without leading dot)
    for domain in [format!(".{}", host_base), host_base.to_string()] {
        if let Ok(header_value) = HeaderValue::from_str(&format!(
            "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Domain={}",
            SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS, domain
        )) {
            response.headers_mut().append(SET_COOKIE, header_value);
        }
    }

    if let Some(root) = compute_root_domain(host_base) {
        let root_no_dot = root.trim_start_matches('.');
        // Root domain cookie (shared across subdominios) with and without leading dot
        for domain in [format!(".{}", root_no_dot), root_no_dot.to_string()] {
            if let Ok(header_value) = HeaderValue::from_str(&format!(
                "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Domain={}",
                SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS, domain
            )) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
        }

        // Slug-specific cookie in both dotted and non-dotted variants
        if !slug.is_empty() {
            let slug_host = format!("{}.{}", slug, root_no_dot);
            for domain in [slug_host.clone(), format!(".{}", slug_host)] {
                if let Ok(header_value) = HeaderValue::from_str(&format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}; Domain={}",
                    SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS, domain
                )) {
                    response.headers_mut().append(SET_COOKIE, header_value);
                }
            }
        }
    }
}

fn compute_root_domain(base: &str) -> Option<String> {
    if base.eq_ignore_ascii_case("localhost") {
        return Some("localhost".to_string());
    }
    if base.parse::<IpAddr>().is_ok() {
        return None;
    }
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() >= 3 {
        // Drop only the first label: foo.miapp.local -> miapp.local
        return Some(parts[1..].join("."));
    }
    if parts.len() == 2 {
        return Some(base.to_string());
    }
    None
}

pub fn compute_cookie_domain(host: &str) -> Option<String> {
    let base = host.split(':').next().unwrap_or(host).trim_start_matches('.');
    compute_root_domain(base)
}

fn compute_redirect_url(host: &str, slug: &str) -> Option<String> {
    if slug.is_empty() {
        return None;
    }
    let (host_no_port, port_part) = host
        .split_once(':')
        .map(|(h, p)| (h, Some(p)))
        .unwrap_or((host, None));
    let base = host_no_port.trim_start_matches('.');
    let root = compute_root_domain(base)?;
    let target_host = format!("{}.{}", slug, root);
    if host_no_port.eq_ignore_ascii_case(&target_host) {
        return None;
    }
    let port = port_part.map(|p| format!(":{}", p)).unwrap_or_default();
    Some(format!("http://{}{}", target_host, port))
}
