use axum::{
    Router,
    body::Body,
    extract::{Json, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{env, fs, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use anyhow::Result;
use image::{ImageFormat, Luma};
use qrcode::QrCode;
use totp_rs::{Algorithm, Secret, TOTP};

use dotenvy::dotenv;
use rand::RngCore; // para rng().fill_bytes()
use std::io::Cursor; // para PNG en memoria (Write + Seek)

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    email: String,
    secret: String, // Base32 (RFC4648), e.g. "JBSWY3DPEHPK3PXP"
}

#[derive(Clone)]
struct AppState {
    users: Vec<User>,
    issuer: String,
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
    // carga .env (opcional)
    dotenv().ok();

    // puedes definir USERS_FILE en .env, si no existe, usa "users.json"
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "users.json".to_string());
    let users_json = fs::read_to_string(&users_file)
        .unwrap_or_else(|e| panic!("users.json not found ({}): {}", users_file, e));
    let users: Vec<User> = serde_json::from_str(&users_json).expect("invalid users.json");

    let state = Arc::new(AppState {
        users,
        issuer: "MiEmpresa".to_string(),
    });

    let app = Router::new()
        .route("/setup", get(setup))
        .route("/qrcode", get(qrcode))
        .route("/login", post(login))
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

// Construye un TOTP desde el secret Base32 del usuario
fn build_totp(issuer: &str, email: &str, base32_secret: &str) -> Result<TOTP> {
    let secret = Secret::Encoded(base32_secret.to_string()).to_bytes()?;
    let totp = TOTP::new(
        Algorithm::SHA1,          // algoritmo estándar de Google Authenticator
        6,                        // número de dígitos
        1,                        // skew (±1 timestep)
        30,                       // periodo en segundos
        secret,                   // bytes del secret
        Some(issuer.to_string()), // issuer
        email.to_string(),        // account name
    )?;
    Ok(totp)
}

/// /setup?email=...  -> devuelve la otpauth URL
async fn setup(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> impl IntoResponse {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&st.issuer, &user.email, &user.secret) {
            Ok(totp) => {
                let url = totp.get_url(); // v5.7: sin parámetros, devuelve String
                (
                    axum::http::StatusCode::OK,
                    Json(serde_json::json!({
                        "email": user.email,
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

/// /qrcode?email=...  -> PNG con el QR del otpauth URL
async fn qrcode(State(st): State<Arc<AppState>>, Query(q): Query<EmailQuery>) -> Response {
    if let Some(user) = find_user(&st, &q.email) {
        match build_totp(&st.issuer, &user.email, &user.secret) {
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
        match build_totp(&st.issuer, &user.email, &user.secret) {
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

// (Opcional) Genera un secret base32 para crear usuarios nuevos en users.json
#[allow(dead_code)]
fn generate_base32_secret(len: usize) -> String {
    let mut buf = vec![0u8; len];
    // rand 0.9: usa rng() (nuevo nombre de thread_rng)
    let mut rng = rand::rng();
    rng.fill_bytes(&mut buf);
    data_encoding::BASE32_NOPAD.encode(&buf)
}
