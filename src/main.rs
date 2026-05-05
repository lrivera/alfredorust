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

mod cfdi;
pub mod filters;
mod models;
mod routes;
mod sat;
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
        .route("/api/sat/cfdi/download", post(routes::sat_cfdi_download))
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
        .route("/api/me/companies", get(routes::me_companies))
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
            "/admin/companies/{id}/cfdis/delete_all",
            post(routes::companies_delete_all_cfdis),
        )
        .route(
            "/admin/companies/{id}/transactions/delete_all",
            post(routes::companies_delete_all_transactions),
        )
        .route("/admin/cfdis", get(routes::cfdis_index))
        .route("/api/admin/cfdis/data", get(routes::cfdis_data_api))
        .route(
            "/admin/companies/{id}/sat_configs",
            post(routes::sat_configs_create),
        )
        .route(
            "/admin/companies/{id}/sat_configs/new",
            get(routes::sat_configs_new),
        )
        .route(
            "/admin/companies/{id}/sat_configs/{config_id}/delete",
            post(routes::sat_configs_delete),
        )
        .route(
            "/admin/companies/{id}/cfdi/download",
            post(routes::company_cfdi_download),
        )
        .route(
            "/admin/companies/{id}/cfdi/jobs",
            get(routes::company_cfdi_jobs_list),
        )
        .route(
            "/admin/companies/{id}/cfdi/jobs/{job_id}",
            get(routes::company_cfdi_job_status),
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
            "/admin/planned_entries/{id}/pay",
            get(routes::planned_entries_pay_form).post(routes::planned_entries_pay),
        )
        .route(
            "/api/admin/transactions/data",
            get(routes::transactions_data_api),
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
        .route(
            "/admin/orders",
            get(routes::orders_index).post(routes::orders_create),
        )
        .route("/admin/orders/new", get(routes::orders_new))
        .route("/admin/orders/{id}/edit", get(routes::orders_edit))
        .route("/admin/orders/{id}/update", post(routes::orders_update))
        .route("/admin/orders/{id}/delete", post(routes::orders_delete))
        .route("/admin/orders/{id}/complete", post(routes::orders_complete))
        .route(
            "/admin/projects",
            get(routes::projects_index).post(routes::projects_create),
        )
        .route("/admin/projects/new", get(routes::projects_new))
        .route("/admin/projects/{id}", get(routes::project_detail))
        .route("/admin/projects/{id}/edit", get(routes::projects_edit))
        .route("/admin/projects/{id}/update", post(routes::projects_update))
        .route("/admin/projects/{id}/delete", post(routes::projects_delete))
        .route(
            "/admin/projects/{id}/advance",
            post(routes::projects_advance),
        )
        .route(
            "/admin/projects/{project_id}/concepts/new",
            get(routes::project_concepts_new),
        )
        .route(
            "/admin/projects/{project_id}/concepts",
            post(routes::project_concepts_create_form),
        )
        .route(
            "/admin/project_concepts/{id}/edit",
            get(routes::project_concepts_edit),
        )
        .route(
            "/admin/project_concepts/{id}/update",
            post(routes::project_concepts_update_form),
        )
        .route(
            "/admin/project_concepts/{id}/advance",
            post(routes::project_concepts_advance_form),
        )
        .route(
            "/admin/project_concepts/{id}/delete",
            post(routes::project_concepts_delete_form),
        )
        .route(
            "/admin/concept_statuses",
            get(routes::concept_statuses_index).post(routes::concept_statuses_create),
        )
        .route(
            "/admin/concept_statuses/new",
            get(routes::concept_statuses_new),
        )
        .route(
            "/admin/concept_statuses/{id}/edit",
            get(routes::concept_statuses_edit),
        )
        .route(
            "/admin/concept_statuses/{id}/update",
            post(routes::concept_statuses_update_form),
        )
        .route(
            "/admin/concept_statuses/{id}/delete",
            post(routes::concept_statuses_delete_form),
        )
        .route(
            "/admin/resources",
            get(routes::resources_index).post(routes::resources_create),
        )
        .route("/admin/resources/new", get(routes::resources_new))
        .route("/admin/resources/{id}/edit", get(routes::resources_edit))
        .route(
            "/admin/resources/{id}/update",
            post(routes::resources_update),
        )
        .route(
            "/admin/resources/{id}/delete",
            post(routes::resources_delete),
        )
        .route(
            "/admin/resource_logs",
            get(routes::resource_logs_index).post(routes::resource_logs_create),
        )
        .route("/admin/resource_logs/new", get(routes::resource_logs_new))
        .route(
            "/admin/resource_logs/{id}/edit",
            get(routes::resource_logs_edit),
        )
        .route(
            "/admin/resource_logs/{id}/update",
            post(routes::resource_logs_update),
        )
        .route(
            "/admin/resource_logs/{id}/delete",
            post(routes::resource_logs_delete),
        )
        .route(
            "/admin/resource_logs/{id}/end",
            post(routes::resource_logs_end),
        )
        .route(
            "/admin/resource_usages",
            get(routes::resource_usages_index).post(routes::resource_usages_save_grid),
        )
        .route(
            "/admin/resource_usages/create",
            post(routes::resource_usages_create_form),
        )
        .route(
            "/admin/resource_usages/new",
            get(routes::resource_usages_new),
        )
        .route(
            "/admin/resource_usages/{id}/edit",
            get(routes::resource_usages_edit),
        )
        .route(
            "/admin/resource_usages/{id}/update",
            post(routes::resource_usages_update_form),
        )
        .route(
            "/admin/resource_usages/{id}/delete",
            post(routes::resource_usages_delete_form),
        )
        .route(
            "/api/admin/concept_statuses",
            get(routes::api_concept_statuses_index).post(routes::api_concept_statuses_create),
        )
        .route(
            "/api/admin/concept_statuses/{id}/update",
            post(routes::api_concept_statuses_update),
        )
        .route(
            "/api/admin/concept_statuses/{id}/delete",
            post(routes::api_concept_statuses_delete),
        )
        .route(
            "/api/admin/projects/{project_id}/concepts",
            get(routes::api_project_concepts_index).post(routes::api_project_concepts_create),
        )
        .route(
            "/api/admin/projects/{project_id}/status_summary",
            get(routes::api_project_status_summary),
        )
        .route(
            "/api/admin/project_concepts/{id}/update",
            post(routes::api_project_concepts_update),
        )
        .route(
            "/api/admin/project_concepts/{id}/advance",
            post(routes::api_project_concepts_advance),
        )
        .route(
            "/api/admin/project_concepts/{id}/delete",
            post(routes::api_project_concepts_delete),
        )
        .route(
            "/api/admin/resource_usages",
            get(routes::api_resource_usages_index).post(routes::api_resource_usages_create),
        )
        .route(
            "/api/admin/resource_usages/{id}/update",
            post(routes::api_resource_usages_update),
        )
        .route(
            "/api/admin/resource_usages/{id}/delete",
            post(routes::api_resource_usages_delete),
        )
        .route(
            "/api/admin/resource_usages/{id}/allocations",
            get(routes::api_resource_usage_allocations_index)
                .post(routes::api_resource_usage_allocations_replace),
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
