use axum::{
    body::Body,
    extract::{Json, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{env, fs, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use anyhow::Result;
use image::{ImageFormat, Luma};
use qrcode::QrCode;
use totp_rs::{Algorithm, Secret, TOTP};

use dotenvy::dotenv;
use rand::RngCore;               // rng().fill_bytes()
use std::io::Cursor;             // PNG en memoria (Write + Seek)
use data_encoding::BASE32_NOPAD;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    email: String,
    secret: String,   // Base32 (RFC4648), e.g. "JBSWY3DPEHPK3PXP"
    company: String,  // <- issuer por usuario (antes era estático)
}

#[derive(Clone)]
struct AppState {
    users: Vec<User>, // ya no guardamos issuer global
}

#[derive(Deserialize)]
struct EmailQuery {
    email: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    code: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Ruta del JSON (por .env USERS_FILE o "users.json")
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "users.json".to_string());
    let users_json = fs::read_to_string(&users_file)
        .unwrap_or_else(|e| panic!("users.json not found ({}): {}", users_file, e));
    let users: Vec<User> = serde_json::from_str(&users_json).expect("invalid users.json");

    let state = Arc::new(AppState { users });

    let app = Router::new()
        .route("/setup", get(setup))
        .route("/qrcode", get(qrcode))
        .route("/login", post(login))
        .route("/secret", get(secret_generate)) // genera secret (NO guarda)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on http://{addr}");
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Busca user por email
fn find_user(state: &AppState, email: &str) -> Option<User> {
    state.users.iter().find(|u| u.email == email).cloned()
}

// Construye un TOTP desde el secret Base32 del usuario, con issuer = company
fn build_totp(issuer: &str, email: &str, base32_secret: &str) -> Result<TOTP> {
    let secret = Secret::Encoded(base32_secret.to_string()).to_bytes()?;
    // (Opcional: validar mínimo 16 bytes)
    if secret.len() < 16 {
        anyhow::bail!("Shared secret too short: {} bytes, need >= 16 (128 bits)", secret.len());
    }
    let totp = TOTP::new(
        Algorithm::SHA1,          // algoritmo estándar de Google Authenticator
        6,                        // dígitos
        1,                        // skew (±1 timestep)
        30,                       // periodo en segundos
        secret,                   // bytes del secret
        Some(issuer.to_string()), // issuer (company del usuario)
        email.to_string(),        // account name
    )?;
    Ok(totp)
}

/// /setup?email=...  -> devuelve la otpauth URL (issuer = company del usuario)
async fn setup(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> impl IntoResponse {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&user.company, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url(); // v5.7: sin parámetros, devuelve String
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

/// /qrcode?email=...  -> PNG con el QR del otpauth URL (issuer = company del usuario)
async fn qrcode(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> Response {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&user.company, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url();
                if let Ok(code) = QrCode::new(url.as_bytes()) {
                    let img = code.render::<Luma<u8>>().min_dimensions(200, 200).build();

                    // write_to requiere Write + Seek -> Cursor<Vec<u8>>
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

/// POST /login  { "email": "...", "code": "123456" }
async fn login(
    State(st): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
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

/* --------- /secret (solo genera, no guarda) --------- */

#[derive(Deserialize)]
struct SecretQuery {
    email: Option<String>, // opcional, solo informativo
    s: Option<usize>,      // alias de bytes
    bytes: Option<usize>,  // preferido
}

const DEFAULT_SECRET_BYTES: usize = 20; // 160 bits recomendado
const MIN_SECRET_BYTES: usize = 16;     // 128 bits mínimo

// Genera un secret Base32 (NOPAD) de N bytes
fn generate_base32_secret_n(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut buf);
    BASE32_NOPAD.encode(&buf)
}

// GET /secret?bytes=20  -> { "secret": "<BASE32>", "bytes": 20, "email": "..."? }
async fn secret_generate(Query(q): Query<SecretQuery>) -> impl IntoResponse {
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
