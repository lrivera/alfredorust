// routes/login.rs
// POST /login { "email": "...", "code": "123456" } -> { "ok": true|false }

use axum::{extract::{Json, State}, response::IntoResponse};
use serde::Deserialize;
use std::sync::Arc;

use crate::state::{AppState, find_user};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub code: String,
}

/// Verifies the current TOTP code with a small skew (±1 step) defined in TOTP::new().
pub async fn login(State(st): State<Arc<AppState>>, Json(body): Json<LoginRequest>) -> impl IntoResponse {
    if let Some(user) = find_user(&st, &body.email) {
        match build_totp(&user.company, &user.email, &user.secret) {
            Ok(totp) => {
                let ok = totp.check_current(&body.code).unwrap_or(false);
                if ok {
                    (
                        axum::http::StatusCode::OK,
                        Json(serde_json::json!({"ok": true})),
                    )
                } else {
                    (
                        axum::http::StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"ok": false})),
                    )
                }
            }
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        }
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"user not found"})),
        )
    }
}
