// routes/setup.rs
// GET /setup?email=... -> returns the otpauth:// URL for enrollment.

use axum::{extract::{Query, State}, response::IntoResponse, Json};
use serde::Deserialize;

use std::sync::Arc;

use crate::state::{AppState, find_user};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: String,
}

/// Returns a JSON with { email, company, otpauth_url } to enroll in authenticator apps.
pub async fn setup(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> impl IntoResponse {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&user.company, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url(); // v5: no args, already contains issuer/account
                (
                    axum::http::StatusCode::OK,
                    Json(serde_json::json!({
                        "email": user.email,
                        "company": user.company,
                        "otpauth_url": url
                    })),
                )
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
