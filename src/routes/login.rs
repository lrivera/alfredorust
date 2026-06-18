// routes/login.rs
// POST /login { "email": "...", "code": "123456" } -> { "ok": true|false }

use axum::{
    extract::{Json, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::{env, net::IpAddr, sync::Arc};

use crate::session::SESSION_COOKIE_NAME;
use crate::state::{AppState, SESSION_TTL_SECONDS, create_session, find_user};
use crate::totp::build_totp;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    /// Login identifier. `email` is accepted as an alias during the rename
    /// transition (older cached clients may still send it).
    #[serde(alias = "email")]
    pub username: String,
    pub code: String,
}

/// Verifies the current TOTP code with a small skew (±1 step) defined in TOTP::new().
#[utoipa::path(
    post,
    path = "/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn login(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    match find_user(&st, &body.username).await {
        Ok(Some(user)) => match build_totp(&user.company_name, &user.username, &user.secret) {
            Ok(totp) => {
                let ok = totp.check_current(&body.code).unwrap_or(false);
                if ok {
                    match create_session(&st, &user.username).await {
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
                            set_cookies_for_host(&mut response, &token, host, &user.company_slug);
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
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "ok": false })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "login failed" })),
        )
            .into_response(),
    }
}
fn set_cookies_for_host(response: &mut Response, token: &str, host: &str, slug: &str) {
    let host_base = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim_start_matches('.');

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
    // If BASE_DOMAIN is set, always use it as the root for tenant subdomain routing.
    if let Ok(base_domain) = env::var("BASE_DOMAIN") {
        if !base_domain.is_empty() {
            return Some(base_domain);
        }
    }
    if base.eq_ignore_ascii_case("localhost") {
        return Some("localhost".to_string());
    }
    if base.parse::<IpAddr>().is_ok() {
        return None;
    }
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() >= 2
        && parts
            .last()
            .is_some_and(|last| last.eq_ignore_ascii_case("local"))
    {
        // Drop only the first label: foo.miapp.local -> miapp.local
        return Some(if parts.len() >= 3 {
            parts[1..].join(".")
        } else {
            base.to_string()
        });
    }
    None
}

pub fn compute_cookie_domain(host: &str) -> Option<String> {
    let base = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim_start_matches('.');
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
    let scheme = if port.is_empty() { "https" } else { "http" };
    Some(format!("{}://{}{}", scheme, target_host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::session::test_env_lock()
    }

    #[test]
    fn cookie_domain_handles_common_hosts() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("BASE_DOMAIN");
        }

        assert_eq!(
            compute_cookie_domain("acme.miapp.local:8090"),
            Some("miapp.local".into())
        );
        assert_eq!(
            compute_cookie_domain("localhost:8090"),
            Some("localhost".into())
        );
        assert_eq!(compute_cookie_domain("127.0.0.1:8090"), None);
    }

    #[test]
    fn base_domain_overrides_cookie_domain() {
        let _guard = env_lock();
        unsafe {
            env::set_var("BASE_DOMAIN", "example.test");
        }

        assert_eq!(
            compute_cookie_domain("anything.local:8090"),
            Some("example.test".into())
        );

        unsafe {
            env::remove_var("BASE_DOMAIN");
        }
    }

    #[test]
    fn redirect_url_targets_company_subdomain() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("BASE_DOMAIN");
        }

        assert_eq!(
            compute_redirect_url("miapp.local:8090", "acme"),
            Some("http://acme.miapp.local:8090".into())
        );
        assert_eq!(compute_redirect_url("acme.miapp.local:8090", "acme"), None);
        assert_eq!(compute_redirect_url("miapp.local:8090", ""), None);
    }
}
