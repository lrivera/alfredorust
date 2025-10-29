// routes/setup.rs
// GET /setup?email=... -> returns the otpauth:// URL for enrollment.

use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
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
pub async fn setup(session: SessionUser, Query(q): Query<EmailQuery>) -> Response {
    if let Err(resp) = session.ensure_email(&q.email) {
        return resp;
    }

    let current = session.user();

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
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
