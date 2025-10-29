// routes/setup.rs
// GET /setup?email=... -> returns the otpauth:// URL for enrollment.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::state::{find_user, AppState};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: String,
}

/// Returns a JSON with { email, company, otpauth_url } to enroll in authenticator apps.
pub async fn setup(
    State(st): State<Arc<AppState>>,
    Query(q): Query<EmailQuery>,
) -> impl IntoResponse {
    match find_user(&st, &q.email).await {
        Ok(Some(user)) => match build_totp(&user.company_name, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url(); // v5: no args, already contains issuer/account
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "email": user.email,
                        "company": user.company_name,
                        "otpauth_url": url
                    })),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "user not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("db error: {e}") })),
        ),
    }
}
