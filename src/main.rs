// main.rs
// Axum server wiring: loads users from users.json, builds router, and serves on :8080.
//
// Endpoints:
// - GET  /                     -> minimal HTML form to POST /login
// - GET  /setup?email=...      -> returns otpauth URL with issuer = user's company
// - GET  /qrcode?email=...     -> returns PNG QR code for that otpauth URL
// - POST /login                -> validates {"email","code"} against current TOTP
// - GET  /secret?bytes=20      -> generates a new Base32 secret (no persistence)

use axum::{routing::{get, post}, Router};
use dotenvy::dotenv;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

mod models;
mod state;
mod totp;
mod routes;
mod session;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let state = Arc::new(
        state::init_state()
            .await
            .expect("failed to initialize MongoDB state"),
    );

    let protected = Router::new()
        .route("/setup", get(routes::setup))
        .route("/qrcode", get(routes::qrcode))
        .route("/secret", get(routes::secret_generate))
        .route("/logout", post(routes::logout));

    let app = Router::new()
        .route("/", get(routes::home))
        .route("/login", post(routes::login))
        .merge(protected)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on http://{addr}");
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
