// routes/logout.rs
// POST /logout -> clears session cookie and removes the session entry.

use axum::{
    extract::{State},
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::session::SessionUser;
use crate::state::{delete_session, AppState};

pub async fn logout(
    State(st): State<Arc<AppState>>,
    SessionUser { token, .. }: SessionUser,
) -> Response {
    let delete_result = delete_session(&st, &token).await;

    let cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        crate::session::SESSION_COOKIE_NAME
    );

    match delete_result {
        Ok(_) => {
            let mut response =
                (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
            if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
            response
        }
        Err(e) => {
            let mut response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("session error: {e}") })),
            )
                .into_response();
            if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
            response
        }
    }
}
