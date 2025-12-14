// routes/logout.rs
// POST /logout -> clears session cookie and removes the session entry.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::routes::login::compute_cookie_domain;
use crate::session::SessionUser;
use crate::state::{AppState, delete_session};

pub async fn logout(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    session: SessionUser,
) -> Response {
    let delete_result = delete_session(&st, session.token()).await;

    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let domain = compute_cookie_domain(host);
    let host_cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        crate::session::SESSION_COOKIE_NAME,
    );
    let domain_cookie = domain.map(|d| {
        format!(
            "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Domain={}",
            crate::session::SESSION_COOKIE_NAME,
            d
        )
    });

    match delete_result {
        Ok(_) => {
            let mut response =
                (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
            if let Ok(header_value) = HeaderValue::from_str(&host_cookie) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
            if let Some(dc) = domain_cookie {
                if let Ok(header_value) = HeaderValue::from_str(&dc) {
                    response.headers_mut().append(SET_COOKIE, header_value);
                }
            }
            response
        }
        Err(e) => {
            let mut response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("session error: {e}") })),
            )
                .into_response();
            if let Ok(header_value) = HeaderValue::from_str(&host_cookie) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
            if let Some(dc) = domain_cookie {
                if let Ok(header_value) = HeaderValue::from_str(&dc) {
                    response.headers_mut().append(SET_COOKIE, header_value);
                }
            }
            response
        }
    }
}
