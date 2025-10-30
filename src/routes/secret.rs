// routes/secret.rs
// GET /secret?bytes=20[&email=...] -> returns a fresh Base32 secret; DOES NOT persist.

use axum::{Json, extract::Query, response::IntoResponse};
use serde::Deserialize;

use crate::session::SessionUser;
use crate::totp::{DEFAULT_SECRET_BYTES, MIN_SECRET_BYTES, generate_base32_secret_n};

#[derive(Deserialize)]
pub struct SecretQuery {
    pub email: Option<String>, // optional, only echoed back
    pub s: Option<usize>,      // alias for bytes
    pub bytes: Option<usize>,  // preferred param
}

/// Only generates and returns a Base32 secret (NOPAD). No I/O or persistence.
pub async fn secret_generate(
    SessionUser { .. }: SessionUser,
    Query(q): Query<SecretQuery>,
) -> impl IntoResponse {
    let mut n = q.bytes.or(q.s).unwrap_or(DEFAULT_SECRET_BYTES);
    if n < MIN_SECRET_BYTES {
        n = MIN_SECRET_BYTES;
    }
    let secret = generate_base32_secret_n(n);

    let mut body = serde_json::Map::new();
    body.insert("secret".to_string(), serde_json::Value::String(secret));
    body.insert(
        "bytes".to_string(),
        serde_json::Value::Number((n as u64).into()),
    );
    if let Some(email) = q.email {
        body.insert("email".to_string(), serde_json::Value::String(email));
    }
    (
        axum::http::StatusCode::OK,
        Json(serde_json::Value::Object(body)),
    )
}
