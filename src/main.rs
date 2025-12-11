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
        .route("/api/tiempo", get(routes::tiempo_data))
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
        .route("/tiempo", get(routes::tiempo_page))
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
        .route(
            "/admin/accounts",
            get(routes::accounts_index).post(routes::accounts_create),
        )
        .route("/admin/accounts/new", get(routes::accounts_new))
        .route("/admin/accounts/{id}/edit", get(routes::accounts_edit))
        .route("/admin/accounts/{id}/update", post(routes::accounts_update))
        .route("/admin/accounts/{id}/delete", post(routes::accounts_delete))
        .route(
            "/admin/categories",
            get(routes::categories_index).post(routes::categories_create),
        )
        .route("/admin/categories/new", get(routes::categories_new))
        .route("/admin/categories/{id}/edit", get(routes::categories_edit))
        .route(
            "/admin/categories/{id}/update",
            post(routes::categories_update),
        )
        .route(
            "/admin/categories/{id}/delete",
            post(routes::categories_delete),
        )
        .route(
            "/admin/contacts",
            get(routes::contacts_index).post(routes::contacts_create),
        )
        .route("/admin/contacts/new", get(routes::contacts_new))
        .route("/admin/contacts/{id}/edit", get(routes::contacts_edit))
        .route("/admin/contacts/{id}/update", post(routes::contacts_update))
        .route("/admin/contacts/{id}/delete", post(routes::contacts_delete))
        .route(
            "/admin/recurring_plans",
            get(routes::recurring_plans_index).post(routes::recurring_plans_create),
        )
        .route(
            "/admin/recurring_plans/new",
            get(routes::recurring_plans_new),
        )
        .route(
            "/admin/recurring_plans/{id}/edit",
            get(routes::recurring_plans_edit),
        )
        .route(
            "/admin/recurring_plans/{id}/update",
            post(routes::recurring_plans_update),
        )
        .route(
            "/admin/recurring_plans/{id}/delete",
            post(routes::recurring_plans_delete),
        )
        .route(
            "/admin/recurring_plans/{id}/generate",
            post(routes::recurring_plans_generate),
        )
        .route(
            "/admin/planned_entries",
            get(routes::planned_entries_index).post(routes::planned_entries_create),
        )
        .route(
            "/admin/planned_entries/new",
            get(routes::planned_entries_new),
        )
        .route(
            "/admin/planned_entries/{id}/edit",
            get(routes::planned_entries_edit),
        )
        .route(
            "/admin/planned_entries/{id}/update",
            post(routes::planned_entries_update),
        )
        .route(
            "/admin/planned_entries/{id}/delete",
            post(routes::planned_entries_delete),
        )
        .route(
            "/admin/transactions",
            get(routes::transactions_index).post(routes::transactions_create),
        )
        .route("/admin/transactions/new", get(routes::transactions_new))
        .route(
            "/admin/transactions/{id}/edit",
            get(routes::transactions_edit),
        )
        .route(
            "/admin/transactions/{id}/update",
            post(routes::transactions_update),
        )
        .route(
            "/admin/transactions/{id}/delete",
            post(routes::transactions_delete),
        )
        .route(
            "/admin/forecasts",
            get(routes::forecasts_index).post(routes::forecasts_create),
        )
        .route("/admin/forecasts/new", get(routes::forecasts_new))
        .route("/admin/forecasts/{id}/edit", get(routes::forecasts_edit))
        .route(
            "/admin/forecasts/{id}/update",
            post(routes::forecasts_update),
        )
        .route(
            "/admin/forecasts/{id}/delete",
            post(routes::forecasts_delete),
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

    let addr = SocketAddr::from(([0, 0, 0, 0], 8090));
    println!("Listening on http://{addr}");
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
