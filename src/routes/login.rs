// routes/login.rs
// POST /login { "email": "...", "code": "123456" } -> { "ok": true|false }

use axum::{
    extract::{Json, State},
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::session::SESSION_COOKIE_NAME;
use crate::state::{create_session, find_user, AppState, SESSION_TTL_SECONDS};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub code: String,
}

/// Verifies the current TOTP code with a small skew (±1 step) defined in TOTP::new().
pub async fn login(
    State(st): State<Arc<AppState>>,
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
                            let cookie = format!(
                                "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                                SESSION_COOKIE_NAME, token, SESSION_TTL_SECONDS
                            );
                            if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                                response.headers_mut().append(SET_COOKIE, header_value);
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
