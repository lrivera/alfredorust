//! Shared HTTP test harness: the in-memory Axum router (`build_app`) and the
//! cookie/auth request helpers. Re-exports the imports the domain test files
//! need so each can simply `use common::harness::*`.
#![allow(unused_imports, dead_code)]

pub use std::sync::Arc;

pub use axum::{
    Router,
    body::{Body, to_bytes},
    http::header,
    http::{Request, StatusCode},
    middleware,
    routing::{get, post},
};
pub use tower::ServiceExt; // for oneshot

pub use alfredodev::{
    models::{
        AccountType, ContactType, FlowType, PlannedStatus, ProjectPriority, ResourceType,
        TransactionType, UserPermission, UserRole,
    },
    routes,
    session::{SESSION_COOKIE_NAME, require_session, require_test_tenant},
    state::{
        AppState, CfdiJob, CfdiJobStatus, add_user_to_company, create_account, create_category,
        create_company, create_concept_status, create_contact, create_forecast,
        create_planned_entry, create_project, create_project_concept, create_recurring_plan,
        create_resource, create_resource_log, create_resource_usage, create_sat_config,
        create_session, create_transaction, create_user, create_user_with_permissions,
        get_user_by_id, list_accounts, list_categories, list_companies, list_contacts,
        update_user_with_permissions,
        list_forecasts, list_planned_entries, list_projects, list_recurring_plans,
        list_resource_logs, list_resource_usage_allocations, list_resource_usages, list_resources,
        list_transactions, list_users, update_resource_allowed_statuses,
    },
};
pub use bson::{DateTime, doc};

pub fn build_app(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/setup", get(routes::setup))
        .route("/qrcode", get(routes::qrcode))
        .route("/secret", get(routes::secret_generate))
        .route("/api/tiempo", get(routes::tiempo_data))
        .route("/logout", post(routes::logout))
        .route(
            "/api/account",
            get(routes::account_profile_data_api).post(routes::account_profile_update_api),
        )
        .route(
            "/admin/users",
            get(routes::users_index).post(routes::users_create),
        )
        .route(
            "/api/admin/users",
            get(routes::api_users_index).post(routes::api_users_create),
        )
        .route("/api/admin/users/{id}", get(routes::api_user_detail))
        .route(
            "/api/admin/users/{id}/update",
            post(routes::api_users_update),
        )
        .route(
            "/api/admin/users/{id}/delete",
            post(routes::api_users_delete),
        )
        .route("/admin/users/new", get(routes::users_new))
        .route("/admin/users/{id}/edit", get(routes::users_edit))
        .route("/admin/users/{id}/update", post(routes::users_update))
        .route("/admin/users/{id}/delete", post(routes::users_delete))
        .route("/admin/users/{id}/qrcode", get(routes::users_qrcode))
        .route("/pdf", get(routes::pdf_editor))
        .route("/pdf/preview", post(routes::pdf_preview))
        .route("/tiempo", get(routes::tiempo_page))
        .route("/api/me", get(routes::me))
        .route("/api/me/companies", get(routes::me_companies))
        .route("/admin/companies", get(routes::companies_index))
        .route(
            "/api/admin/companies",
            get(routes::companies_data_api).post(routes::company_create_api),
        )
        .route("/api/admin/companies/{id}", get(routes::company_data_api))
        .route(
            "/api/admin/companies/{id}/update",
            post(routes::company_update_api),
        )
        .route(
            "/api/admin/companies/{id}/cfdis/delete_all",
            post(routes::company_cfdis_delete_all_api),
        )
        .route(
            "/api/admin/companies/{id}/transactions/delete_all",
            post(routes::company_transactions_delete_all_api),
        )
        .route("/admin/companies/new", get(routes::companies_new))
        .route("/admin/companies/{id}/edit", get(routes::companies_edit))
        .route("/admin/cfdis", get(routes::cfdis_index))
        .route("/api/admin/cfdis/data", get(routes::cfdis_data_api))
        .route("/api/admin/cfdis/{uuid}", get(routes::cfdi_data_api))
        .route(
            "/admin/companies/{id}/cfdi/jobs",
            get(routes::company_cfdi_jobs_list),
        )
        .route(
            "/admin/companies/{id}/cfdi/jobs/{job_id}",
            get(routes::company_cfdi_job_status),
        )
        .route(
            "/api/admin/sat-configs",
            get(routes::sat_configs_data_api).post(routes::sat_config_create_api),
        )
        .route(
            "/api/admin/sat-configs/upload",
            post(routes::sat_config_upload_api),
        )
        .route(
            "/api/admin/sat-configs/{id}",
            get(routes::sat_config_data_api),
        )
        .route(
            "/api/admin/sat-configs/{id}/update",
            post(routes::sat_config_update_api),
        )
        .route(
            "/api/admin/sat-configs/{id}/delete",
            post(routes::sat_config_delete_api),
        )
        .route(
            "/admin/companies/{id}/delete",
            post(routes::companies_delete),
        )
        .route(
            "/admin/accounts",
            get(routes::accounts_index).post(routes::accounts_create),
        )
        .route("/api/admin/accounts", get(routes::accounts_data_api))
        .route("/api/admin/accounts/{id}", get(routes::account_data_api))
        .route(
            "/api/admin/accounts/{id}/update",
            post(routes::account_update_api),
        )
        .route(
            "/api/admin/accounts/{id}/delete",
            post(routes::account_delete_api),
        )
        .route("/admin/accounts/new", get(routes::accounts_new))
        .route("/admin/accounts/{id}/edit", get(routes::accounts_edit))
        .route("/admin/accounts/{id}/update", post(routes::accounts_update))
        .route("/admin/accounts/{id}/delete", post(routes::accounts_delete))
        .route(
            "/admin/categories",
            get(routes::categories_index).post(routes::categories_create),
        )
        .route("/api/admin/categories", get(routes::categories_data_api))
        .route("/api/admin/categories/{id}", get(routes::category_data_api))
        .route(
            "/api/admin/categories/{id}/update",
            post(routes::category_update_api),
        )
        .route(
            "/api/admin/categories/{id}/delete",
            post(routes::category_delete_api),
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
        .route("/api/admin/contacts", get(routes::contacts_data_api))
        .route("/api/admin/contacts/{id}", get(routes::contact_data_api))
        .route(
            "/api/admin/contacts/{id}/update",
            post(routes::contact_update_api),
        )
        .route(
            "/api/admin/contacts/{id}/delete",
            post(routes::contact_delete_api),
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
            "/api/admin/recurring-plans",
            get(routes::recurring_plans_data_api).post(routes::recurring_plans_create_api),
        )
        .route(
            "/api/admin/recurring-plans/{id}",
            get(routes::recurring_plan_data_api),
        )
        .route(
            "/api/admin/recurring-plans/{id}/update",
            post(routes::recurring_plan_update_api),
        )
        .route(
            "/api/admin/recurring-plans/{id}/delete",
            post(routes::recurring_plan_delete_api),
        )
        .route(
            "/api/admin/recurring-plans/{id}/generate",
            post(routes::recurring_plan_generate_api),
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
            "/admin/planned_entries/{id}/pay",
            get(routes::planned_entries_pay_form).post(routes::planned_entries_pay),
        )
        .route(
            "/admin/planned_entries/bulk_pay",
            get(routes::planned_entries_bulk_pay_form).post(routes::planned_entries_bulk_pay),
        )
        .route(
            "/api/admin/planned-entries",
            get(routes::planned_entries_data_api).post(routes::planned_entries_create_api),
        )
        .route(
            "/api/admin/planned-entries/bulk-pay",
            post(routes::planned_entries_bulk_pay_api),
        )
        .route(
            "/api/admin/planned-entries/{id}",
            get(routes::planned_entry_data_api),
        )
        .route(
            "/api/admin/planned-entries/{id}/update",
            post(routes::planned_entry_update_api),
        )
        .route(
            "/api/admin/planned-entries/{id}/delete",
            post(routes::planned_entry_delete_api),
        )
        .route(
            "/api/admin/planned-entries/{id}/pay",
            post(routes::planned_entry_pay_api),
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
        // POST routes for transactions omitted in tests (use private types)
        .route(
            "/api/admin/transactions/data",
            get(routes::transactions_data_api),
        )
        .route(
            "/api/admin/transactions",
            post(routes::transactions_create_api),
        )
        .route(
            "/api/admin/transactions/{id}",
            get(routes::transaction_data_api),
        )
        .route(
            "/api/admin/transactions/{id}/update",
            post(routes::transaction_update_api),
        )
        .route(
            "/api/admin/transactions/{id}/delete",
            post(routes::transaction_delete_api),
        )
        .route(
            "/admin/forecasts",
            get(routes::forecasts_index).post(routes::forecasts_create),
        )
        .route("/api/admin/forecasts", get(routes::forecasts_data_api))
        .route("/api/admin/forecasts/{id}", get(routes::forecast_data_api))
        .route(
            "/api/admin/forecasts/{id}/update",
            post(routes::forecast_update_api),
        )
        .route(
            "/api/admin/forecasts/{id}/delete",
            post(routes::forecast_delete_api),
        )
        .route("/admin/forecasts/new", get(routes::forecasts_new))
        .route("/admin/forecasts/{id}/edit", get(routes::forecasts_edit))
        // POST routes for forecasts omitted in tests (use private types)
        .route(
            "/api/admin/orders",
            get(routes::orders_data_api).post(routes::orders_create_api),
        )
        .route("/api/admin/orders/{id}", get(routes::order_data_api))
        .route(
            "/api/admin/orders/{id}/update",
            post(routes::order_update_api),
        )
        .route(
            "/api/admin/orders/{id}/delete",
            post(routes::order_delete_api),
        )
        .route(
            "/api/admin/orders/{id}/complete",
            post(routes::order_complete_api),
        )
        .route(
            "/admin/projects",
            get(routes::projects_index).post(routes::projects_create),
        )
        .route(
            "/api/admin/projects",
            get(routes::projects_data_api).post(routes::projects_create_api),
        )
        .route("/api/admin/projects/{id}", get(routes::project_data_api))
        .route(
            "/api/admin/projects/{id}/update",
            post(routes::project_update_api),
        )
        .route(
            "/api/admin/projects/{id}/delete",
            post(routes::project_delete_api),
        )
        .route(
            "/api/admin/projects/{id}/advance",
            post(routes::project_advance_api),
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
        .route("/admin/projects/new", get(routes::projects_new))
        .route("/admin/projects/{id}/edit", get(routes::projects_edit))
        .route("/admin/projects/{id}/update", post(routes::projects_update))
        .route("/admin/projects/{id}/delete", post(routes::projects_delete))
        .route(
            "/admin/projects/{id}/advance",
            post(routes::projects_advance),
        )
        .route(
            "/admin/resources",
            get(routes::resources_index).post(routes::resources_create),
        )
        .route(
            "/api/admin/resources",
            get(routes::resources_data_api).post(routes::resources_create_api),
        )
        .route("/api/admin/resources/{id}", get(routes::resource_data_api))
        .route(
            "/api/admin/resources/{id}/update",
            post(routes::resource_update_api),
        )
        .route(
            "/api/admin/resources/{id}/delete",
            post(routes::resource_delete_api),
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
        .route(
            "/api/admin/resource_logs",
            get(routes::resource_logs_data_api).post(routes::resource_logs_create_api),
        )
        .route(
            "/api/admin/resource_logs/{id}",
            get(routes::resource_log_data_api),
        )
        .route(
            "/api/admin/resource_logs/{id}/update",
            post(routes::resource_log_update_api),
        )
        .route(
            "/api/admin/resource_logs/{id}/delete",
            post(routes::resource_log_delete_api),
        )
        .route(
            "/api/admin/resource_logs/{id}/end",
            post(routes::resource_log_end_api),
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
            "/api/admin/resource_usages",
            get(routes::api_resource_usages_index).post(routes::api_resource_usages_create),
        )
        .route(
            "/api/admin/resource_usages/grid",
            get(routes::api_resource_usages_grid_view).post(routes::api_resource_usages_grid_save),
        )
        .route(
            "/api/admin/resource_usages/{id}",
            get(routes::api_resource_usage_detail),
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
            require_session,
        ));

    Router::new()
        .route("/", get(routes::home))
        .route("/login", post(routes::login))
        .merge(protected)
        .with_state(state)
}

pub async fn get_with_cookie(app: Router, host: &str, path: &str, token: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .uri(path)
        .header("host", host)
        .header("cookie", format!("{SESSION_COOKIE_NAME}={token}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.expect("request failed");
    let status = res.status();
    let body_bytes = to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("body read failed");
    let body = String::from_utf8_lossy(&body_bytes).to_string();
    (status, body)
}

pub async fn post_form_with_cookie(
    app: Router,
    host: &str,
    path: &str,
    token: &str,
    form_body: String,
) -> StatusCode {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("host", host)
        .header("cookie", format!("{SESSION_COOKIE_NAME}={token}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    app.oneshot(req).await.expect("request failed").status()
}

pub async fn post_form_with_cookie_response(
    app: Router,
    host: &str,
    path: &str,
    token: &str,
    form_body: String,
) -> (StatusCode, Option<String>, String) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("host", host)
        .header("cookie", format!("{SESSION_COOKIE_NAME}={token}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let res = app.oneshot(req).await.expect("request failed");
    let status = res.status();
    let location = res
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_bytes = to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("body read failed");
    let body = String::from_utf8_lossy(&body_bytes).to_string();
    (status, location, body)
}

pub async fn post_json_with_cookie(
    app: Router,
    host: &str,
    path: &str,
    token: &str,
    payload: serde_json::Value,
) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("host", host)
        .header("cookie", format!("{SESSION_COOKIE_NAME}={token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.expect("request failed");
    let status = res.status();
    let body_bytes = to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("body read failed");
    let body = String::from_utf8_lossy(&body_bytes).to_string();
    (status, body)
}

pub async fn post_multipart_with_cookie(
    app: Router,
    host: &str,
    path: &str,
    token: &str,
    fields: &[(&str, Option<&str>, &[u8])],
) -> (StatusCode, String) {
    let boundary = "TESTBOUNDARYabc123";
    let mut body: Vec<u8> = Vec::new();
    for (name, filename, bytes) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(fname) => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{fname}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            ),
        }
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("host", host)
        .header("cookie", format!("{SESSION_COOKIE_NAME}={token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.expect("request failed");
    let status = res.status();
    let body_bytes = to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body read failed");
    (status, String::from_utf8_lossy(&body_bytes).to_string())
}


pub async fn assert_requires_auth_get(shared: &Arc<AppState>, path: &str) {
    let req = Request::builder()
        .uri(path)
        .header("host", "localhost")
        .body(Body::empty())
        .unwrap();
    let res = build_app(shared.clone())
        .oneshot(req)
        .await
        .expect("request failed");
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "GET {path} must require authentication"
    );
}

pub async fn assert_requires_auth_post(shared: &Arc<AppState>, path: &str) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("host", "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = build_app(shared.clone())
        .oneshot(req)
        .await
        .expect("request failed");
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "POST {path} must require authentication"
    );
}

pub async fn assert_get_denied_no_leak(
    shared: &Arc<AppState>,
    host: &str,
    token: &str,
    path: &str,
    needle: &str,
) {
    let (status, body) = get_with_cookie(build_app(shared.clone()), host, path, token).await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
        "GET {path} must be denied cross-tenant, got {status}: {body}"
    );
    assert!(
        !body.contains(needle),
        "cross-tenant data leak on GET {path}: found `{needle}` in body {body}"
    );
}

pub async fn assert_post_denied(
    shared: &Arc<AppState>,
    host: &str,
    token: &str,
    path: &str,
    payload: serde_json::Value,
) {
    let (status, _) =
        post_json_with_cookie(build_app(shared.clone()), host, path, token, payload).await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
        "POST {path} must be denied cross-tenant, got {status}"
    );
}

