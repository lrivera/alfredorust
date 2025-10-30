// main.rs
// Axum server wiring: loads users from users.json, builds router, and serves on :8080.
//
// Endpoints:
// - GET  /                     -> minimal HTML form to POST /login
// - GET  /setup?email=...      -> returns otpauth URL with issuer = user's company
// - GET  /qrcode?email=...     -> returns PNG QR code for that otpauth URL
// - POST /login                -> validates {"email","code"} against current TOTP
// - GET  /secret?bytes=20      -> generates a new Base32 secret (no persistence)

use axum::{
    Router, middleware,
    routing::{get, post},
};
use dotenvy::dotenv;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

pub mod filters;
mod models;
mod routes;
mod session;
mod state;
mod totp;

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
        .route("/logout", post(routes::logout))
        .route(
            "/account",
            get(routes::account_edit).post(routes::account_update),
        )
        .route(
            "/admin/users",
            get(routes::users_index).post(routes::users_create),
        )
        .route("/admin/users/new", get(routes::users_new))
        .route("/admin/users/{id}/edit", get(routes::users_edit))
        .route("/admin/users/{id}/update", post(routes::users_update))
        .route("/admin/users/{id}/delete", post(routes::users_delete))
        .route("/admin/users/{id}/qrcode", get(routes::users_qrcode))
        .route("/pdf", get(routes::pdf_editor))
        .route("/pdf/preview", post(routes::pdf_preview))
        .route(
            "/admin/companies",
            get(routes::companies_index).post(routes::companies_create),
        )
        .route("/admin/companies/new", get(routes::companies_new))
        .route("/admin/companies/{id}/edit", get(routes::companies_edit))
        .route(
            "/admin/companies/{id}/update",
            post(routes::companies_update),
        )
        .route(
            "/admin/companies/{id}/delete",
            post(routes::companies_delete),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            session::require_session,
        ));

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
