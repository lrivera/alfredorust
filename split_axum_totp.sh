#!/usr/bin/env bash
set -euo pipefail

# Create directories
mkdir -p src/routes
touch .env || true

# -------- src/models.rs --------
cat > src/models.rs <<'RS'
// models.rs
// Domain models with serde derive for JSON (users.json).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub email: String,     // Base identity (account name in TOTP)
    pub secret: String,    // Base32 (RFC 4648) TOTP shared secret (NOPAD recommended)
    pub company: String,   // Issuer shown in authenticator apps
}
RS

# -------- src/state.rs --------
cat > src/state.rs <<'RS'
// state.rs
// AppState and helpers to load users and look them up.

use crate::models::User;
use std::{fs, env};

#[derive(Clone)]
pub struct AppState {
    pub users: Vec<User>,
}

pub fn load_users_from_env() -> Vec<User> {
    // USERS_FILE can be provided via .env or environment; defaults to "users.json"
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "users.json".to_string());
    let users_json = fs::read_to_string(&users_file)
        .unwrap_or_else(|e| panic!("users.json not found ({}): {}", users_file, e));
    serde_json::from_str::<Vec<User>>(&users_json).expect("invalid users.json")
}

pub fn find_user<'a>(state: &'a AppState, email: &str) -> Option<User> {
    state.users.iter().find(|u| u.email == email).cloned()
}
RS

# -------- src/totp.rs --------
cat > src/totp.rs <<'RS'
// totp.rs
// TOTP utilities: build a TOTP instance and generate Base32 secrets.

use anyhow::Result;
use data_encoding::BASE32_NOPAD;
use rand::RngCore;
use totp_rs::{Algorithm, Secret, TOTP};

pub const MIN_SECRET_BYTES: usize = 16;      // 128 bits (mandatory minimum)
pub const DEFAULT_SECRET_BYTES: usize = 20;  // 160 bits (recommended)

/// Build a TOTP instance using user's issuer (company), account name (email), and Base32 secret.
/// Validates minimum secret length after Base32 decoding.
pub fn build_totp(issuer: &str, email: &str, base32_secret: &str) -> Result<TOTP> {
    // Decode Base32; accept NOPAD format (recommended).
    let secret = Secret::Encoded(base32_secret.to_string()).to_bytes()?;
    if secret.len() < MIN_SECRET_BYTES {
        anyhow::bail!(
            "Shared secret too short: {} bytes, need >= {} ({} bits)",
            secret.len(),
            MIN_SECRET_BYTES,
            MIN_SECRET_BYTES * 8
        );
    }
    let totp = TOTP::new(
        Algorithm::SHA1,          // compatible with Google Authenticator
        6,                        // digits
        1,                        // skew (±1 timestep to absorb small clock drift)
        30,                       // period in seconds
        secret,                   // secret bytes
        Some(issuer.to_string()), // issuer
        email.to_string(),        // account name
    )?;
    Ok(totp)
}

/// Generate a random Base32 (NOPAD) secret of `bytes` length.
pub fn generate_base32_secret_n(bytes: usize) -> String {
    let n = bytes.max(MIN_SECRET_BYTES);
    let mut buf = vec![0u8; n];
    let mut rng = rand::rng(); // rand 0.9: thread-local RNG
    rng.fill_bytes(&mut buf);
    BASE32_NOPAD.encode(&buf)
}
RS

# -------- src/routes/mod.rs --------
cat > src/routes/mod.rs <<'RS'
// routes/mod.rs
// Public re-exports of all route handlers.

pub mod setup;
pub mod qrcode;
pub mod login;
pub mod secret;

pub use setup::setup;
pub use qrcode::qrcode;
pub use login::login;
pub use secret::secret_generate;
RS

# -------- src/routes/setup.rs --------
cat > src/routes/setup.rs <<'RS'
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
RS

# -------- src/routes/qrcode.rs --------
cat > src/routes/qrcode.rs <<'RS'
// routes/qrcode.rs
// GET /qrcode?email=... -> returns a PNG QR code of the otpauth URL.

use axum::{
    body::Body,
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use image::{ImageFormat, Luma};
use qrcode::QrCode;
use serde::Deserialize;
use std::io::Cursor;
use std::sync::Arc;

use crate::state::{AppState, find_user};
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: String,
}

/// Builds and returns a PNG QR code so clients can scan and enroll.
pub async fn qrcode(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> Response {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&user.company, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url();
                if let Ok(code) = QrCode::new(url.as_bytes()) {
                    let img = code.render::<Luma<u8>>().min_dimensions(200, 200).build();

                    // image 0.25: write_to requires Write + Seek -> Cursor<Vec<u8>>
                    let mut cursor = Cursor::new(Vec::<u8>::new());
                    if image::DynamicImage::ImageLuma8(img)
                        .write_to(&mut cursor, ImageFormat::Png)
                        .is_ok()
                    {
                        let png = cursor.into_inner();
                        return Response::builder()
                            .header("Content-Type", "image/png")
                            .body(Body::from(png))
                            .unwrap();
                    }
                }
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to build qr",
                )
                    .into_response()
            }
            Err(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "invalid secret",
            )
                .into_response(),
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "user not found").into_response()
    }
}
RS

# -------- src/routes/login.rs --------
cat > src/routes/login.rs <<'RS'
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
RS

# -------- src/routes/secret.rs --------
cat > src/routes/secret.rs <<'RS'
// routes/secret.rs
// GET /secret?bytes=20[&email=...] -> returns a fresh Base32 secret; DOES NOT persist.

use axum::{extract::Query, response::IntoResponse, Json};
use serde::Deserialize;

use crate::totp::{generate_base32_secret_n, DEFAULT_SECRET_BYTES, MIN_SECRET_BYTES};

#[derive(Deserialize)]
pub struct SecretQuery {
    pub email: Option<String>, // optional, only echoed back
    pub s: Option<usize>,      // alias for bytes
    pub bytes: Option<usize>,  // preferred param
}

/// Only generates and returns a Base32 secret (NOPAD). No I/O or persistence.
pub async fn secret_generate(Query(q): Query<SecretQuery>) -> impl IntoResponse {
    let mut n = q.bytes.or(q.s).unwrap_or(DEFAULT_SECRET_BYTES);
    if n < MIN_SECRET_BYTES {
        n = MIN_SECRET_BYTES;
    }
    let secret = generate_base32_secret_n(n);

    let mut body = serde_json::Map::new();
    body.insert("secret".to_string(), serde_json::Value::String(secret));
    body.insert("bytes".to_string(), serde_json::Value::Number((n as u64).into()));
    if let Some(email) = q.email {
        body.insert("email".to_string(), serde_json::Value::String(email));
    }
    (axum::http::StatusCode::OK, Json(serde_json::Value::Object(body)))
}
RS

# -------- src/main.rs --------
cat > src/main.rs <<'RS'
// main.rs
// Axum server wiring: loads users from users.json, builds router, and serves on :8080.
//
// Endpoints:
// - GET  /setup?email=...     -> returns otpauth URL with issuer = user's company
// - GET  /qrcode?email=...    -> returns PNG QR code for that otpauth URL
// - POST /login               -> validates {"email","code"} against current TOTP
// - GET  /secret?bytes=20     -> generates a new Base32 secret (no persistence)

use axum::{routing::{get, post}, Router};
use dotenvy::dotenv;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

mod models;
mod state;
mod totp;
mod routes;

use state::{AppState, load_users_from_env};

#[tokio::main]
async fn main() {
    dotenv().ok();

    let users = load_users_from_env();
    let state = Arc::new(AppState { users });

    let app = Router::new()
        .route("/setup", get(routes::setup))
        .route("/qrcode", get(routes::qrcode))
        .route("/login", post(routes::login))
        .route("/secret", get(routes::secret_generate))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on http://{addr}");
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
RS

# -------- users.json (example) --------
cat > users.json <<'JSON'
[
  { "email": "alfredo@example.com", "secret": "KVSYYQOFAACHZYGG7HIA53SUPXHUT4X2", "company": "miempresa" }
]
JSON

echo "Done. Files created:"
find src -maxdepth 2 -type f -print | sed 's|^| - |'
echo "Also wrote ./users.json (example)."
echo
echo "Run with:  cargo run"
