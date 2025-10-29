// routes/setup.rs
// GET /setup?email=... -> returns the otpauth:// URL for enrollment.

use axum::{
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::session::SessionUser;
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: String,
}

/// Returns a JSON with { email, company, otpauth_url } to enroll in authenticator apps.
pub async fn setup(SessionUser { user: current, .. }: SessionUser, Query(q): Query<EmailQuery>) -> impl IntoResponse {
    if current.email != q.email {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "mismatched session" })),
        );
    }

    match build_totp(&current.company_name, &current.email, &current.secret) {
        Ok(totp) => {
            let url = totp.get_url(); // v5: no args, already contains issuer/account
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "email": current.email,
                    "company": current.company_name,
                    "otpauth_url": url
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}
