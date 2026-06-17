#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::header,
    http::{Request, StatusCode},
    middleware,
    routing::{get, post},
};
use tower::ServiceExt; // for oneshot

use alfredodev::{
    models::{
        AccountType, ContactType, FlowType, PlannedStatus, ProjectPriority, ResourceType,
        TransactionType, UserPermission, UserRole,
    },
    routes,
    session::{SESSION_COOKIE_NAME, require_session},
    state::{
        AppState, CfdiJob, CfdiJobStatus, add_user_to_company, create_account, create_category,
        create_company, create_concept_status, create_contact, create_forecast,
        create_planned_entry,
        create_project, create_project_concept, create_recurring_plan, create_resource,
        create_resource_log, create_resource_usage, create_sat_config, create_session,
        create_transaction, create_user, create_user_with_permissions, get_user_by_id,
        list_accounts, list_categories, list_companies, list_contacts, list_forecasts,
        list_planned_entries, list_projects, list_recurring_plans, list_resource_logs,
        list_resource_usage_allocations, list_resource_usages, list_resources, list_transactions,
        list_users, update_resource_allowed_statuses,
    },
};
use bson::{DateTime, doc};

fn build_app(state: Arc<AppState>) -> Router {
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
            post(routes::api_resource_usages_grid_save),
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

async fn get_with_cookie(app: Router, host: &str, path: &str, token: &str) -> (StatusCode, String) {
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

async fn post_form_with_cookie(
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

async fn post_form_with_cookie_response(
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

async fn post_json_with_cookie(
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

#[tokio::test]
async fn account_json_profile_redacts_secret_and_updates_from_payload() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(
        &state,
        "Account JSON Co",
        "account-json-co",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let user_id = create_user_with_permissions(
        &state,
        "account-json@example.com",
        "OLDSECRET",
        &[(company.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "account-json@example.com")
        .await
        .unwrap();
    let host = "account-json-co.miapp.local";

    let (status, body) =
        get_with_cookie(build_app(shared.clone()), host, "/api/account", &token).await;
    assert_eq!(status, StatusCode::OK);
    let profile: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(profile["email"], "account-json@example.com");
    assert!(profile.get("secret").is_none());
    assert!(profile.get("otpauth_url").is_none());

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/account",
        &token,
        serde_json::json!({
            "email": "account-json-updated@example.com",
            "secret": "NEWSECRET"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert_eq!(updated.email, "account-json-updated@example.com");
    assert_eq!(updated.secret, "NEWSECRET");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn company_admin_json_endpoints_enforce_admin_and_update_metadata() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Admin Company JSON A",
        "admin-company-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Admin Company JSON B",
        "admin-company-json-b",
        "USD",
        true,
        None,
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "company-admin-json@example.com",
        "SECRET",
        &[
            (company_a.clone(), UserRole::Admin, vec![]),
            (company_b.clone(), UserRole::Admin, vec![]),
        ],
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "company-staff-json@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    let admin_token = create_session(&state, "company-admin-json@example.com")
        .await
        .unwrap();
    let staff_token = create_session(&state, "company-staff-json@example.com")
        .await
        .unwrap();
    let host = "admin-company-json-a.miapp.local";

    let (status, body) = get_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/companies",
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let companies: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert!(
        companies
            .iter()
            .any(|company| company["slug"] == "admin-company-json-a")
    );
    assert!(
        companies
            .iter()
            .any(|company| company["slug"] == "admin-company-json-b")
    );

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/companies",
        &admin_token,
        serde_json::json!({
            "name": "Admin Company JSON C",
            "slug": "admin-company-json-c",
            "default_currency": "MXN",
            "is_active": true,
            "notes": "Created through JSON"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(created["id"].as_str().is_some());

    // Staff must be forbidden on the company JSON admin endpoints. Assert this
    // BEFORE renaming company_a's slug below, otherwise the host subdomain no
    // longer resolves to the staff user's company and the request would 401
    // instead of 403.
    let (status, _) = get_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/companies",
        &staff_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/companies/{}/update", company_b.to_hex()),
        &staff_token,
        serde_json::json!({
            "name": "Forbidden",
            "slug": "forbidden",
            "default_currency": "MXN"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let path = format!("/api/admin/companies/{}/update", company_a.to_hex());
    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        &path,
        &admin_token,
        serde_json::json!({
            "name": "Admin Company JSON A Updated",
            "slug": "admin-company-json-a-updated",
            "default_currency": "USD",
            "is_active": false,
            "notes": "Updated through JSON"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let updated = list_companies(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|company| company.id.as_ref() == Some(&company_a))
        .unwrap();
    assert_eq!(updated.name, "Admin Company JSON A Updated");
    assert_eq!(updated.slug, "admin-company-json-a-updated");
    assert_eq!(updated.default_currency, "USD");
    assert!(!updated.is_active);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn finance_endpoints_render_seeded_data() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    // Use the first seeded user/session and its active company for host/subdomain.
    let user = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", user.company_slug);
    let token = create_session(&state, &user.email).await.unwrap();

    // Collect expected strings from the database to assert they appear in responses.
    let companies = list_companies(&state).await.unwrap();
    assert!(!companies.is_empty(), "seeded companies present");
    let accounts = list_accounts(&state).await.unwrap();
    let categories = list_categories(&state).await.unwrap();
    let contacts = list_contacts(&state).await.unwrap();
    let plans = list_recurring_plans(&state).await.unwrap();
    let planned_entries = list_planned_entries(&state).await.unwrap();
    let transactions = list_transactions(&state).await.unwrap();
    let forecasts = list_forecasts(&state).await.unwrap();

    assert!(!accounts.is_empty(), "seeded accounts present");
    assert!(!categories.is_empty(), "seeded categories present");
    assert!(!contacts.is_empty(), "seeded contacts present");
    assert!(!plans.is_empty(), "seeded recurring plans present");
    assert!(
        !planned_entries.is_empty(),
        "seeded planned entries present"
    );
    assert!(!transactions.is_empty(), "seeded transactions present");
    assert!(!forecasts.is_empty(), "seeded forecasts present");

    let cases = vec![
        ("/admin/accounts", accounts[0].name.clone()),
        ("/admin/categories", categories[0].name.clone()),
        ("/admin/contacts", contacts[0].name.clone()),
        ("/admin/recurring_plans", plans[0].name.clone()),
        ("/admin/planned_entries", planned_entries[0].name.clone()),
        ("/admin/transactions", "tx-root".to_string()),
        (
            "/api/admin/transactions/data",
            transactions[0].description.clone(),
        ),
        ("/admin/forecasts", forecasts[0].currency.clone()),
    ];

    for (path, expected) in cases {
        let app = build_app(shared.clone());
        let (status, body) = get_with_cookie(app, &host, path, &token).await;
        assert_eq!(status, StatusCode::OK, "GET {path} must return 200");
        assert!(
            body.contains(&expected),
            "page {path} should contain seeded data: {expected}"
        );
    }

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn finance_json_endpoints_scope_to_active_tenant() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Finance JSON A",
        "finance-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Finance JSON B",
        "finance-json-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let user_id = create_user(
        &state,
        "finance-json-admin@example.com",
        "SECRET",
        &[
            (company_a.clone(), UserRole::Admin),
            (company_b.clone(), UserRole::Admin),
        ],
    )
    .await
    .unwrap();
    let user = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    let token = create_session(&state, &user.email).await.unwrap();
    let host_a = "finance-json-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "finance-json-category-a",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "finance-json-category-b",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let account_a = create_account(
        &state,
        &company_a,
        "finance-json-account-a",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let account_b = create_account(
        &state,
        &company_b,
        "finance-json-account-b",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let forecast_a = create_forecast(
        &state,
        &company_a,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        None,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        DateTime::parse_rfc3339_str("2026-12-31T00:00:00Z").unwrap(),
        "MXN",
        1000.0,
        500.0,
        500.0,
        None,
        None,
        None,
        Some("finance-json-forecast-a".into()),
        None,
    )
    .await
    .unwrap();
    let forecast_b = create_forecast(
        &state,
        &company_b,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        None,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        DateTime::parse_rfc3339_str("2026-12-31T00:00:00Z").unwrap(),
        "MXN",
        1000.0,
        500.0,
        500.0,
        None,
        None,
        None,
        Some("finance-json-forecast-b".into()),
        None,
    )
    .await
    .unwrap();
    let recurring_plan_a = create_recurring_plan(
        &state,
        &company_a,
        "finance-json-plan-a",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        250.0,
        "monthly",
        Some(10),
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        None,
        true,
        1,
        None,
    )
    .await
    .unwrap();
    let recurring_plan_b = create_recurring_plan(
        &state,
        &company_b,
        "finance-json-plan-b",
        FlowType::Expense,
        &category_b,
        &account_b,
        None,
        250.0,
        "monthly",
        Some(10),
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        None,
        true,
        1,
        None,
    )
    .await
    .unwrap();
    let planned_entry_a = create_planned_entry(
        &state,
        &company_a,
        None,
        None,
        None,
        "finance-json-entry-a",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        75.0,
        DateTime::parse_rfc3339_str("2026-02-01T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    let planned_entry_b = create_planned_entry(
        &state,
        &company_b,
        None,
        None,
        None,
        "finance-json-entry-b",
        FlowType::Expense,
        &category_b,
        &account_b,
        None,
        75.0,
        DateTime::parse_rfc3339_str("2026-02-01T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    let transaction_a = create_transaction(
        &state,
        &company_a,
        DateTime::parse_rfc3339_str("2026-03-01T00:00:00Z").unwrap(),
        "finance-json-transaction-a",
        TransactionType::Expense,
        &category_a,
        Some(account_a.clone()),
        None,
        42.0,
        None,
        None,
        true,
        None,
        None,
        None,
        Some("MXN".into()),
        None,
    )
    .await
    .unwrap();
    let transaction_b = create_transaction(
        &state,
        &company_b,
        DateTime::parse_rfc3339_str("2026-03-01T00:00:00Z").unwrap(),
        "finance-json-transaction-b",
        TransactionType::Expense,
        &category_b,
        Some(account_b.clone()),
        None,
        42.0,
        None,
        None,
        true,
        None,
        None,
        None,
        Some("MXN".into()),
        None,
    )
    .await
    .unwrap();

    let list_cases = vec![
        (
            "/api/admin/accounts",
            "finance-json-account-a",
            "finance-json-account-b",
        ),
        (
            "/api/admin/categories",
            "finance-json-category-a",
            "finance-json-category-b",
        ),
        (
            "/api/admin/forecasts",
            "finance-json-forecast-a",
            "finance-json-forecast-b",
        ),
        (
            "/api/admin/recurring-plans",
            "finance-json-plan-a",
            "finance-json-plan-b",
        ),
        (
            "/api/admin/planned-entries",
            "finance-json-entry-a",
            "finance-json-entry-b",
        ),
        (
            "/api/admin/transactions/data",
            "finance-json-transaction-a",
            "finance-json-transaction-b",
        ),
    ];

    for (path, visible, hidden) in list_cases {
        let app = build_app(shared.clone());
        let (status, body) = get_with_cookie(app, host_a, path, &token).await;
        assert_eq!(status, StatusCode::OK, "GET {path} must return 200");
        serde_json::from_str::<serde_json::Value>(&body).expect("response must be JSON");
        assert!(
            body.contains(visible),
            "{path} must include active tenant data"
        );
        assert!(
            !body.contains(hidden),
            "{path} must not include other tenant data"
        );
    }

    let detail_cases = vec![
        (
            format!("/api/admin/accounts/{}", account_a.to_hex()),
            StatusCode::OK,
            "finance-json-account-a",
        ),
        (
            format!("/api/admin/accounts/{}", account_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
        (
            format!("/api/admin/categories/{}", category_a.to_hex()),
            StatusCode::OK,
            "finance-json-category-a",
        ),
        (
            format!("/api/admin/categories/{}", category_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
        (
            format!("/api/admin/forecasts/{}", forecast_a.to_hex()),
            StatusCode::OK,
            "finance-json-forecast-a",
        ),
        (
            format!("/api/admin/forecasts/{}", forecast_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
        (
            format!("/api/admin/recurring-plans/{}", recurring_plan_a.to_hex()),
            StatusCode::OK,
            "finance-json-plan-a",
        ),
        (
            format!("/api/admin/recurring-plans/{}", recurring_plan_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
        (
            format!("/api/admin/planned-entries/{}", planned_entry_a.to_hex()),
            StatusCode::OK,
            "finance-json-entry-a",
        ),
        (
            format!("/api/admin/planned-entries/{}", planned_entry_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
        (
            format!("/api/admin/transactions/{}", transaction_a.to_hex()),
            StatusCode::OK,
            "finance-json-transaction-a",
        ),
        (
            format!("/api/admin/transactions/{}", transaction_b.to_hex()),
            StatusCode::FORBIDDEN,
            "",
        ),
    ];

    for (path, expected_status, expected_body) in detail_cases {
        let app = build_app(shared.clone());
        let (status, body) = get_with_cookie(app, host_a, &path, &token).await;
        assert_eq!(
            status, expected_status,
            "GET {path} returned unexpected status"
        );
        if expected_status == StatusCode::OK {
            serde_json::from_str::<serde_json::Value>(&body).expect("response must be JSON");
            assert!(
                body.contains(expected_body),
                "{path} should return expected record"
            );
        }
    }

    let staff_id = create_user(
        &state,
        "finance-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let app = build_app(shared);
    let (status, _) = get_with_cookie(app, host_a, "/api/admin/accounts", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn recurring_plan_json_mutations_scope_and_report_generation_side_effects() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Recurring Plan Mutation A",
        "recurring-plan-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Recurring Plan Mutation B",
        "recurring-plan-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "recurring-plan-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host_a = "recurring-plan-mutation-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "Recurring Plan Category A",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "Recurring Plan Category B",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let account_a = create_account(
        &state,
        &company_a,
        "Recurring Plan Account A",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/recurring-plans",
        &token,
        serde_json::json!({
            "name": "Recurring plan created",
            "flow_type": "expense",
            "category_id": category_a.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 100.0,
            "frequency": "monthly",
            "day_of_month": 15,
            "start_date": "2026-07-01T00:00:00Z",
            "is_active": true,
            "version": 1,
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let plan_id = created["id"].as_str().expect("created id").to_string();
    assert!(
        created["side_effects"]["planned_entries_generated"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/recurring-plans/{plan_id}/update"),
        &token,
        serde_json::json!({
            "name": "Recurring plan updated",
            "flow_type": "expense",
            "category_id": category_a.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 125.0,
            "frequency": "monthly",
            "day_of_month": 20,
            "start_date": "2026-07-01T00:00:00Z",
            "is_active": true,
            "version": 1,
            "notes": "updated"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let updated: serde_json::Value = serde_json::from_str(&body).expect("update response JSON");
    assert_eq!(updated["side_effects"]["previous_version"], 1);

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/recurring-plans",
        &token,
        serde_json::json!({
            "name": "Bad tenant ref",
            "flow_type": "expense",
            "category_id": category_b.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 1.0,
            "frequency": "monthly",
            "start_date": "2026-07-01T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/recurring-plans/{plan_id}/generate"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let generated: serde_json::Value = serde_json::from_str(&body).expect("generate response JSON");
    assert!(
        generated["side_effects"]["planned_entries_after"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/recurring-plans/{plan_id}/delete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn order_json_mutations_scope_and_report_planned_entry_side_effects() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Order Mutation A",
        "order-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Order Mutation B",
        "order-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "order-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host_a = "order-mutation-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "Order Category A",
        FlowType::Income,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "Order Category B",
        FlowType::Income,
        None,
        None,
    )
    .await
    .unwrap();
    let account_a = create_account(
        &state,
        &company_a,
        "Order Account A",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/orders",
        &token,
        serde_json::json!({
            "title": "Order created",
            "category_id": category_a.to_hex(),
            "account_id": account_a.to_hex(),
            "status": "confirmed",
            "amount": 250.0,
            "scheduled_at": "2026-07-01T00:00:00Z",
            "items": [{"description": "Service", "quantity": 1.0, "unit_price": 250.0}],
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let order_id = created["id"].as_str().expect("created id").to_string();
    assert_eq!(created["side_effects"]["planned_entry_created"], true);
    assert!(
        list_planned_entries(&state)
            .await
            .unwrap()
            .into_iter()
            .any(
                |entry| entry.service_order_id.as_ref().map(|id| id.to_hex())
                    == Some(order_id.clone())
            )
    );

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/orders/{order_id}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(body.contains("Order created"));

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/orders",
        &token,
        serde_json::json!({
            "title": "Bad tenant ref",
            "category_id": category_b.to_hex(),
            "account_id": account_a.to_hex(),
            "status": "pending",
            "amount": 1.0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/orders/{order_id}/update"),
        &token,
        serde_json::json!({
            "title": "Order updated",
            "category_id": category_a.to_hex(),
            "account_id": account_a.to_hex(),
            "status": "in_progress",
            "amount": 300.0,
            "items": [{"description": "Service", "quantity": 2.0, "unit_price": 150.0}]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/orders/{order_id}/complete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let completed: serde_json::Value = serde_json::from_str(&body).expect("complete response JSON");
    assert_eq!(completed["side_effects"]["order_completed"], true);

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/orders/{order_id}/delete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn planned_entry_json_mutations_scope_and_create_payment_side_effects() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Planned Entry Mutation A",
        "planned-entry-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Planned Entry Mutation B",
        "planned-entry-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "planned-entry-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host_a = "planned-entry-mutation-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "Planned Entry Category A",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "Planned Entry Category B",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let account_a = create_account(
        &state,
        &company_a,
        "Planned Entry Account A",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/planned-entries",
        &token,
        serde_json::json!({
            "name": "Planned entry created",
            "flow_type": "expense",
            "category_id": category_a.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 100.0,
            "due_date": "2026-07-01T00:00:00Z",
            "status": "planned",
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let entry_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/planned-entries/{entry_id}/update"),
        &token,
        serde_json::json!({
            "name": "Planned entry updated",
            "flow_type": "expense",
            "category_id": category_a.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 120.0,
            "due_date": "2026-07-02T00:00:00Z",
            "status": "planned",
            "notes": "updated"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let updated: serde_json::Value = serde_json::from_str(&body).expect("update response JSON");
    assert_eq!(
        updated["side_effects"]["planned_entry_recalculated"],
        entry_id
    );

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/planned-entries",
        &token,
        serde_json::json!({
            "name": "Bad tenant ref",
            "flow_type": "expense",
            "category_id": category_b.to_hex(),
            "account_expected_id": account_a.to_hex(),
            "amount_estimated": 1.0,
            "due_date": "2026-07-01T00:00:00Z",
            "status": "planned"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/planned-entries/{entry_id}/pay"),
        &token,
        serde_json::json!({
            "paid_at": "2026-07-03T00:00:00Z",
            "amount": 120.0,
            "account_id": account_a.to_hex(),
            "notes": "paid"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let paid: serde_json::Value = serde_json::from_str(&body).expect("pay response JSON");
    assert_eq!(paid["side_effects"]["transaction_created"], true);
    assert_eq!(paid["side_effects"]["planned_entry_recalculated"], entry_id);
    let transactions = list_transactions(&state).await.unwrap();
    assert!(transactions.iter().any(|tx| {
        tx.company_id == company_a
            && tx.planned_entry_id.as_ref().map(|id| id.to_hex()) == Some(entry_id.clone())
    }));

    let bulk_a = create_planned_entry(
        &state,
        &company_a,
        None,
        None,
        None,
        "Bulk pay A",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        10.0,
        DateTime::parse_rfc3339_str("2026-08-01T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    let bulk_b = create_planned_entry(
        &state,
        &company_a,
        None,
        None,
        None,
        "Bulk pay B",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        20.0,
        DateTime::parse_rfc3339_str("2026-08-02T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/planned-entries/bulk-pay",
        &token,
        serde_json::json!({
            "entry_ids": [bulk_a.to_hex(), bulk_b.to_hex()],
            "paid_at": "2026-08-03T00:00:00Z",
            "account_id": account_a.to_hex()
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bulk_paid: serde_json::Value = serde_json::from_str(&body).expect("bulk pay response JSON");
    assert_eq!(bulk_paid["side_effects"]["transactions_created"], 2);

    let delete_target = create_planned_entry(
        &state,
        &company_a,
        None,
        None,
        None,
        "Delete planned entry",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        5.0,
        DateTime::parse_rfc3339_str("2026-09-01T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();
    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!(
            "/api/admin/planned-entries/{}/delete",
            delete_target.to_hex()
        ),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn transaction_json_mutations_scope_and_report_side_effects() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Transaction Mutation A",
        "transaction-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Transaction Mutation B",
        "transaction-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "transaction-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host_a = "transaction-mutation-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "Transaction Mutation Category A",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "Transaction Mutation Category B",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let account_a = create_account(
        &state,
        &company_a,
        "Transaction Mutation Account A",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let planned_entry = create_planned_entry(
        &state,
        &company_a,
        None,
        None,
        None,
        "Transaction Mutation Planned Entry",
        FlowType::Expense,
        &category_a,
        &account_a,
        None,
        100.0,
        DateTime::parse_rfc3339_str("2026-07-01T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/transactions",
        &token,
        serde_json::json!({
            "date": "2026-07-01T12:00:00Z",
            "description": "Transaction mutation created",
            "transaction_type": "expense",
            "category_id": category_a.to_hex(),
            "account_from_id": account_a.to_hex(),
            "amount": 42.5,
            "planned_entry_id": planned_entry.to_hex(),
            "is_confirmed": true,
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    assert_eq!(
        created["side_effects"]["planned_entry_recalculated"],
        planned_entry.to_hex()
    );
    let transaction_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/transactions/{transaction_id}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Transaction mutation created"));

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/transactions/{transaction_id}/update"),
        &token,
        serde_json::json!({
            "date": "2026-07-02T12:00:00Z",
            "description": "Transaction mutation updated",
            "transaction_type": "expense",
            "category_id": category_a.to_hex(),
            "account_from_id": account_a.to_hex(),
            "amount": 50.0,
            "planned_entry_id": planned_entry.to_hex(),
            "is_confirmed": false,
            "notes": "updated"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/transactions/{transaction_id}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let updated: serde_json::Value = serde_json::from_str(&body).expect("detail response JSON");
    assert_eq!(updated["description"], "Transaction mutation updated");
    assert_eq!(updated["amount"], 50.0);
    assert_eq!(updated["is_confirmed"], false);

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/transactions",
        &token,
        serde_json::json!({
            "date": "2026-07-01T12:00:00Z",
            "description": "Bad tenant ref",
            "transaction_type": "expense",
            "category_id": category_b.to_hex(),
            "amount": 1.0,
            "is_confirmed": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/transactions/{transaction_id}/delete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let deleted: serde_json::Value = serde_json::from_str(&body).expect("delete response JSON");
    assert_eq!(
        deleted["side_effects"]["planned_entry_recalculated"],
        planned_entry.to_hex()
    );

    let app = build_app(shared);
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/transactions/{transaction_id}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn cfdi_json_endpoints_scope_to_active_tenant() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "CFDI JSON A", "cfdi-json-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "CFDI JSON B", "cfdi-json-b", "MXN", true, None)
        .await
        .unwrap();
    let user_id = create_user(
        &state,
        "cfdi-json-admin@example.com",
        "SECRET",
        &[
            (company_a.clone(), UserRole::Admin),
            (company_b.clone(), UserRole::Admin),
        ],
    )
    .await
    .unwrap();
    let user = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    let token = create_session(&state, &user.email).await.unwrap();
    let host_a = "cfdi-json-a.miapp.local";
    let uuid_a = "11111111-1111-1111-1111-111111111111";
    let uuid_b = "22222222-2222-2222-2222-222222222222";

    state
        .cfdis
        .insert_one(doc! {
            "company_id": company_a.to_hex(),
            "uuid": uuid_a,
            "comprobante": {
                "serie": "A",
                "folio": "100",
                "tipoDeComprobante": "I",
                "fecha": "2026-01-01T00:00:00",
                "subTotal": "100.00",
                "total": "116.00",
                "moneda": "MXN",
                "formaPago": "03",
                "metodoPago": "PUE",
            },
            "emisor": { "rfc": "AAA010101AAA", "nombre": "CFDI Emisor A" },
            "receptor": { "rfc": "XAXX010101000", "nombre": "CFDI Receptor A" },
            "impuestos": { "totalImpuestosTrasladados": "16.00" },
            "conceptos": [{
                "descripcion": "CFDI concepto A",
                "cantidad": "1",
                "valorUnitario": "100.00",
                "importe": "100.00",
            }],
        })
        .await
        .unwrap();
    state
        .cfdis
        .insert_one(doc! {
            "company_id": company_b.to_hex(),
            "uuid": uuid_b,
            "comprobante": {
                "serie": "B",
                "folio": "200",
                "tipoDeComprobante": "I",
                "fecha": "2026-01-02T00:00:00",
                "subTotal": "200.00",
                "total": "232.00",
                "moneda": "MXN",
                "formaPago": "03",
                "metodoPago": "PUE",
            },
            "emisor": { "rfc": "BBB010101BBB", "nombre": "CFDI Emisor B" },
            "receptor": { "rfc": "XAXX010101000", "nombre": "CFDI Receptor B" },
            "impuestos": { "totalImpuestosTrasladados": "32.00" },
            "conceptos": [{
                "descripcion": "CFDI concepto B",
                "cantidad": "1",
                "valorUnitario": "200.00",
                "importe": "200.00",
            }],
        })
        .await
        .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(app, host_a, "/api/admin/cfdis/data", &token).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_str::<serde_json::Value>(&body).expect("response must be JSON");
    assert!(body.contains(uuid_a));
    assert!(body.contains("CFDI concepto A"));
    assert!(!body.contains(uuid_b));
    assert!(!body.contains("CFDI concepto B"));

    let app = build_app(shared.clone());
    let (status, body) =
        get_with_cookie(app, host_a, &format!("/api/admin/cfdis/{uuid_a}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let detail: serde_json::Value = serde_json::from_str(&body).expect("detail must be JSON");
    assert_eq!(detail["uuid"], uuid_a);
    assert_eq!(detail["folio"], "100");
    assert_eq!(detail["conceptos"][0]["descripcion"], "CFDI concepto A");

    let app = build_app(shared.clone());
    let (status, _body) =
        get_with_cookie(app, host_a, &format!("/api/admin/cfdis/{uuid_b}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let staff_id = create_user(
        &state,
        "cfdi-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let app = build_app(shared);
    let (status, _body) = get_with_cookie(app, host_a, "/api/admin/cfdis/data", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn cfdi_job_endpoints_scope_to_company_and_admin() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "CFDI Jobs A", "cfdi-jobs-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "CFDI Jobs B", "cfdi-jobs-b", "MXN", true, None)
        .await
        .unwrap();
    let admin_id = create_user(
        &state,
        "cfdi-jobs-admin@example.com",
        "SECRET",
        &[
            (company_a.clone(), UserRole::Admin),
            (company_b.clone(), UserRole::Admin),
        ],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "cfdi-jobs-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "cfdi-jobs-a.miapp.local";

    {
        let mut jobs = state.jobs.lock().await;
        jobs.insert(
            "job-a".into(),
            CfdiJob {
                job_id: "job-a".into(),
                company_id: company_a.to_hex(),
                label: "2026-01".into(),
                chunk_start: "2026-01-01".into(),
                started_at: "2026-01-15".into(),
                status: CfdiJobStatus::Queued,
            },
        );
        jobs.insert(
            "job-b".into(),
            CfdiJob {
                job_id: "job-b".into(),
                company_id: company_b.to_hex(),
                label: "2026-02".into(),
                chunk_start: "2026-02-01".into(),
                started_at: "2026-02-15".into(),
                status: CfdiJobStatus::Running,
            },
        );
    }

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/admin/companies/{}/cfdi/jobs", company_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("job-a"));
    assert!(!body.contains("job-b"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/admin/companies/{}/cfdi/jobs/job-a", company_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let job: serde_json::Value = serde_json::from_str(&body).expect("job must be JSON");
    assert_eq!(job["job_id"], "job-a");

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/admin/companies/{}/cfdi/jobs/job-b", company_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared);
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/admin/companies/{}/cfdi/jobs", company_a.to_hex()),
        &staff_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn sat_config_json_endpoints_scope_and_redact_sensitive_fields() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "SAT JSON A", "sat-json-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "SAT JSON B", "sat-json-b", "MXN", true, None)
        .await
        .unwrap();
    let admin_id = create_user(
        &state,
        "sat-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "sat-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "sat-json-a.miapp.local";

    let config_a = bson::oid::ObjectId::new();
    create_sat_config(
        &state,
        config_a.clone(),
        company_a.clone(),
        "AAA010101AAA".into(),
        "uploads/sat/company-a/cert.cer".into(),
        "uploads/sat/company-a/private.key".into(),
        "dummy-password-a".into(),
        Some("Primary FIEL".into()),
    )
    .await
    .unwrap();
    let config_b = bson::oid::ObjectId::new();
    create_sat_config(
        &state,
        config_b.clone(),
        company_b.clone(),
        "BBB010101BBB".into(),
        "uploads/sat/company-b/cert.cer".into(),
        "uploads/sat/company-b/private.key".into(),
        "dummy-password-b".into(),
        Some("Hidden FIEL".into()),
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(app, host_a, "/api/admin/sat-configs", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("AAA010101AAA"));
    assert!(body.contains("Primary FIEL"));
    assert!(!body.contains("BBB010101BBB"));
    assert!(!body.contains("dummy-password-a"));
    assert!(!body.contains("private.key"));
    assert!(!body.contains("cert.cer"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/sat-configs/{}", config_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let config: serde_json::Value = serde_json::from_str(&body).expect("config must be JSON");
    assert_eq!(config["rfc"], "AAA010101AAA");
    assert_eq!(config["label"], "Primary FIEL");
    assert!(config.get("key_password").is_none());
    assert!(config.get("key_path").is_none());
    assert!(config.get("cer_path").is_none());

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/sat-configs/{}", config_b.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/sat-configs",
        &admin_token,
        serde_json::json!({
            "rfc": "CCC010101CCC",
            "cer_path": "uploads/sat/company-a/new.cer",
            "key_path": "uploads/sat/company-a/new.key",
            "key_password": "new-secret",
            "label": "New FIEL"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert!(!body.contains("new-secret"));
    assert!(!body.contains("new.key"));
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let created_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/sat-configs/{created_id}/update"),
        &admin_token,
        serde_json::json!({
            "rfc": "DDD010101DDD",
            "cer_path": "uploads/sat/company-a/updated.cer",
            "key_path": "uploads/sat/company-a/updated.key",
            "key_password": "updated-secret",
            "label": "Updated FIEL"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(!body.contains("updated-secret"));
    assert!(!body.contains("updated.key"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/sat-configs/{created_id}"),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(body.contains("DDD010101DDD"));
    assert!(!body.contains("updated-secret"));
    assert!(!body.contains("updated.key"));

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/sat-configs",
        &staff_token,
        serde_json::json!({
            "rfc": "EEE010101EEE",
            "cer_path": "x.cer",
            "key_path": "x.key",
            "key_password": "secret"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/sat-configs/{created_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared);
    let (status, _body) =
        get_with_cookie(app, host_a, "/api/admin/sat-configs", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn project_json_endpoints_scope_and_redact_money() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Project JSON A",
        "project-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Project JSON B",
        "project-json-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "project-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user_with_permissions(
        &state,
        "project-json-staff@example.com",
        "SECRET",
        &[(
            company_a.clone(),
            UserRole::Staff,
            vec![UserPermission::ViewProjects],
        )],
    )
    .await
    .unwrap();
    let blocked_id = create_user(
        &state,
        "project-json-blocked@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let blocked = get_user_by_id(&state, &blocked_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let blocked_token = create_session(&state, &blocked.email).await.unwrap();
    let host_a = "project-json-a.miapp.local";

    let project_a = create_project(
        &state,
        &company_a,
        "Project JSON visible",
        None,
        None,
        Some("Visible project".into()),
        ProjectPriority::High,
        Some(1234.56),
        Some(DateTime::parse_rfc3339_str("2026-04-01T00:00:00Z").unwrap()),
        Some("Visible notes".into()),
    )
    .await
    .unwrap();
    let project_b = create_project(
        &state,
        &company_b,
        "Project JSON hidden",
        None,
        None,
        Some("Hidden project".into()),
        ProjectPriority::Low,
        Some(9999.99),
        None,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(app, host_a, "/api/admin/projects", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    let projects: serde_json::Value = serde_json::from_str(&body).expect("projects must be JSON");
    assert!(body.contains("Project JSON visible"));
    assert!(!body.contains("Project JSON hidden"));
    assert!(
        projects
            .as_array()
            .unwrap()
            .iter()
            .any(|project| project["total_budget"] == 1234.56)
    );

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{}", project_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let project: serde_json::Value = serde_json::from_str(&body).expect("project must be JSON");
    assert_eq!(project["title"], "Project JSON visible");
    assert_eq!(project["priority"], "high");
    assert_eq!(project["total_budget"], 1234.56);

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{}", project_b.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{}", project_a.to_hex()),
        &staff_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let project: serde_json::Value = serde_json::from_str(&body).expect("project must be JSON");
    assert_eq!(project["title"], "Project JSON visible");
    assert!(project["total_budget"].is_null());

    let app = build_app(shared);
    let (status, _body) = get_with_cookie(app, host_a, "/api/admin/projects", &blocked_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn project_json_mutations_scope_and_advance_base_project() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Project Mutation A",
        "project-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Project Mutation B",
        "project-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "project-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user_with_permissions(
        &state,
        "project-mutation-staff@example.com",
        "SECRET",
        &[(
            company_a.clone(),
            UserRole::Staff,
            vec![UserPermission::ViewProjects],
        )],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "project-mutation-a.miapp.local";

    let category_a = create_category(
        &state,
        &company_a,
        "Project Mutation Category A",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let category_b = create_category(
        &state,
        &company_b,
        "Project Mutation Category B",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/projects",
        &admin_token,
        serde_json::json!({
            "title": "Project created",
            "category_id": category_a.to_hex(),
            "description": "created",
            "priority": "high",
            "total_budget": 1000.0,
            "scheduled_at": "2026-07-01T00:00:00Z",
            "notes": "created notes"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let project_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/projects",
        &staff_token,
        serde_json::json!({ "title": "Forbidden", "priority": "medium" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/projects",
        &admin_token,
        serde_json::json!({
            "title": "Bad tenant ref",
            "category_id": category_b.to_hex(),
            "priority": "medium"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{project_id}/update"),
        &admin_token,
        serde_json::json!({
            "title": "Project updated",
            "category_id": category_a.to_hex(),
            "description": "updated",
            "priority": "urgent",
            "total_budget": 1200.0,
            "scheduled_at": "2026-07-02T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{project_id}/advance"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let advanced: serde_json::Value = serde_json::from_str(&body).expect("advance response JSON");
    assert_eq!(advanced["status"], "analisis");

    assert!(
        list_projects(&state, &company_a)
            .await
            .unwrap()
            .into_iter()
            .any(|project| {
                project.id.as_ref().map(|id| id.to_hex()) == Some(project_id.clone())
                    && project.title == "Project updated"
            })
    );

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/projects/{project_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn project_workflow_json_mutations_manage_statuses_and_concepts() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(
        &state,
        "Project Workflow JSON",
        "project-workflow-json",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "project-workflow-json-admin@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "project-workflow-json-staff@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host = "project-workflow-json.miapp.local";

    let project_id = create_project(
        &state,
        &company,
        "Workflow Project",
        None,
        None,
        None,
        ProjectPriority::Medium,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        "/api/admin/concept_statuses",
        &admin_token,
        serde_json::json!({
            "name": "Started",
            "position": 1,
            "is_initial": true,
            "is_active": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let initial_status_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        "/api/admin/concept_statuses",
        &admin_token,
        serde_json::json!({
            "name": "Done",
            "position": 2,
            "is_terminal": true,
            "is_active": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let terminal_status_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/projects/{}/concepts", project_id.to_hex()),
        &admin_token,
        serde_json::json!({
            "status_id": initial_status_id,
            "name": "Workflow Concept",
            "quantity": 2.0,
            "unit": "job",
            "position": 1
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let concept_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host,
        &format!("/api/admin/projects/{}/status_summary", project_id.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Started"));

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/project_concepts/{concept_id}/advance"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(body.contains("Done"));

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/project_concepts/{concept_id}/update"),
        &admin_token,
        serde_json::json!({
            "status_id": terminal_status_id,
            "name": "Workflow Concept Updated",
            "quantity": 3.0,
            "unit": "job",
            "position": 2
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/project_concepts/{concept_id}/delete"),
        &staff_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/project_concepts/{concept_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/concept_statuses/{terminal_status_id}/update"),
        &admin_token,
        serde_json::json!({
            "name": "Archived",
            "position": 3,
            "is_terminal": true,
            "is_active": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/concept_statuses/{terminal_status_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_json_endpoints_scope_to_active_tenant_and_admins() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Resource JSON A",
        "resource-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Resource JSON B",
        "resource-json-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "resource-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "resource-json-a.miapp.local";

    let resource_a = create_resource(
        &state,
        &company_a,
        "Resource JSON visible",
        ResourceType::Machinery,
        true,
        Some("Visible notes".into()),
    )
    .await
    .unwrap();
    let resource_b = create_resource(
        &state,
        &company_b,
        "Resource JSON hidden",
        ResourceType::Vehicle,
        true,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(app, host_a, "/api/admin/resources", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Resource JSON visible"));
    assert!(!body.contains("Resource JSON hidden"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resources/{}", resource_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resource: serde_json::Value = serde_json::from_str(&body).expect("resource must be JSON");
    assert_eq!(resource["name"], "Resource JSON visible");
    assert_eq!(resource["resource_type"], "machinery");

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resources/{}", resource_b.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let app = build_app(shared);
    let (status, _body) = get_with_cookie(app, host_a, "/api/admin/resources", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_json_mutations_scope_and_validate_allowed_statuses() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Resource Mutation A",
        "resource-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Resource Mutation B",
        "resource-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "resource-mutation-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "resource-mutation-a.miapp.local";

    let status_a = create_concept_status(
        &state, &company_a, "Allowed", 1, None, true, false, false, true,
    )
    .await
    .unwrap();
    let status_b = create_concept_status(
        &state, &company_b, "Hidden", 1, None, true, false, false, true,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/resources",
        &admin_token,
        serde_json::json!({
            "name": "Resource created",
            "resource_type": "vehicle",
            "is_active": true,
            "hourly_cost": 250.0,
            "currency": "MXN",
            "allowed_status_ids": [status_a.to_hex()],
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let resource_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/resources",
        &staff_token,
        serde_json::json!({ "name": "Forbidden", "resource_type": "machinery" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/resources",
        &admin_token,
        serde_json::json!({
            "name": "Bad tenant status",
            "resource_type": "machinery",
            "allowed_status_ids": [status_b.to_hex()]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resources/{resource_id}/update"),
        &admin_token,
        serde_json::json!({
            "name": "Resource updated",
            "resource_type": "equipment",
            "is_active": false,
            "hourly_cost": 300.0,
            "currency": "USD",
            "allowed_status_ids": [status_a.to_hex()]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let resources = list_resources(&state, &company_a).await.unwrap();
    assert!(resources.iter().any(|resource| {
        resource.id.as_ref().map(|id| id.to_hex()) == Some(resource_id.clone())
            && resource.name == "Resource updated"
            && !resource.is_active
            && resource.hourly_cost == 300.0
            && resource.currency == "USD"
    }));

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resources/{resource_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_log_json_endpoints_scope_to_active_tenant_and_admins() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Resource Log JSON A",
        "resource-log-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Resource Log JSON B",
        "resource-log-json-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-log-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "resource-log-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "resource-log-json-a.miapp.local";

    let log_a = create_resource_log(
        &state,
        &company_a,
        None,
        Some("Phase A".into()),
        None,
        Some("Resource Log JSON visible".into()),
        DateTime::parse_rfc3339_str("2026-05-01T10:00:00Z").unwrap(),
        Some("Operator A".into()),
        Some("Visible notes".into()),
    )
    .await
    .unwrap();
    let log_b = create_resource_log(
        &state,
        &company_b,
        None,
        Some("Phase B".into()),
        None,
        Some("Resource Log JSON hidden".into()),
        DateTime::parse_rfc3339_str("2026-05-02T10:00:00Z").unwrap(),
        Some("Operator B".into()),
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) =
        get_with_cookie(app, host_a, "/api/admin/resource_logs", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Resource Log JSON visible"));
    assert!(!body.contains("Resource Log JSON hidden"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_logs/{}", log_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let log: serde_json::Value = serde_json::from_str(&body).expect("resource log must be JSON");
    assert_eq!(log["phase"], "Phase A");
    assert_eq!(log["resource_name"], "Resource Log JSON visible");
    assert_eq!(log["operator_name"], "Operator A");

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_logs/{}", log_b.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let app = build_app(shared);
    let (status, _body) =
        get_with_cookie(app, host_a, "/api/admin/resource_logs", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_log_json_mutations_scope_and_compute_duration() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Resource Log Mutation A",
        "resource-log-mutation-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Resource Log Mutation B",
        "resource-log-mutation-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-log-mutation-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host_a = "resource-log-mutation-a.miapp.local";

    let project_a = create_project(
        &state,
        &company_a,
        "Resource Log Project A",
        None,
        None,
        None,
        ProjectPriority::Medium,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let project_b = create_project(
        &state,
        &company_b,
        "Resource Log Project B",
        None,
        None,
        None,
        ProjectPriority::Medium,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let resource_a = create_resource(
        &state,
        &company_a,
        "Resource Log Resource A",
        ResourceType::Machinery,
        true,
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/resource_logs",
        &token,
        serde_json::json!({
            "project_id": project_a.to_hex(),
            "phase": "assembly",
            "resource_id": resource_a.to_hex(),
            "started_at": "2026-07-01T10:00:00Z",
            "operator_name": "Operator",
            "notes": "created"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let log_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, _body) = post_json_with_cookie(
        app,
        host_a,
        "/api/admin/resource_logs",
        &token,
        serde_json::json!({
            "project_id": project_b.to_hex(),
            "started_at": "2026-07-01T10:00:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_logs/{log_id}/update"),
        &token,
        serde_json::json!({
            "project_id": project_a.to_hex(),
            "phase": "testing",
            "resource_id": resource_a.to_hex(),
            "started_at": "2026-07-01T10:00:00Z",
            "ended_at": "2026-07-01T11:30:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_logs/{log_id}/end"),
        &token,
        serde_json::json!({ "ended_at": "2026-07-01T12:00:00Z" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ended: serde_json::Value = serde_json::from_str(&body).expect("end response JSON");
    assert_eq!(ended["duration_hours"], 2.0);

    let logs = list_resource_logs(&state, &company_a).await.unwrap();
    assert!(logs.iter().any(|log| {
        log.id.as_ref().map(|id| id.to_hex()) == Some(log_id.clone())
            && log.phase.as_deref() == Some("testing")
            && log.duration_hours == Some(2.0)
    }));

    let app = build_app(shared);
    let (status, body) = post_json_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_logs/{log_id}/delete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_usage_json_get_scopes_to_active_tenant_and_admins() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(
        &state,
        "Resource Usage JSON A",
        "resource-usage-json-a",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let company_b = create_company(
        &state,
        "Resource Usage JSON B",
        "resource-usage-json-b",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-usage-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let staff_id = create_user(
        &state,
        "resource-usage-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let staff = get_user_by_id(&state, &staff_id).await.unwrap().unwrap();
    let admin_token = create_session(&state, &admin.email).await.unwrap();
    let staff_token = create_session(&state, &staff.email).await.unwrap();
    let host_a = "resource-usage-json-a.miapp.local";

    let resource_a = create_resource(
        &state,
        &company_a,
        "Usage JSON visible resource",
        ResourceType::Machinery,
        true,
        None,
    )
    .await
    .unwrap();
    let resource_b = create_resource(
        &state,
        &company_b,
        "Usage JSON hidden resource",
        ResourceType::Vehicle,
        true,
        None,
    )
    .await
    .unwrap();
    let usage_a = create_resource_usage(
        &state,
        &company_a,
        &resource_a,
        DateTime::parse_rfc3339_str("2026-06-01T10:00:00Z").unwrap(),
        Some(DateTime::parse_rfc3339_str("2026-06-01T12:00:00Z").unwrap()),
        Some("Operator A".into()),
        Some("Visible usage".into()),
    )
    .await
    .unwrap();
    let usage_b = create_resource_usage(
        &state,
        &company_b,
        &resource_b,
        DateTime::parse_rfc3339_str("2026-06-02T10:00:00Z").unwrap(),
        None,
        Some("Operator B".into()),
        None,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) =
        get_with_cookie(app, host_a, "/api/admin/resource_usages", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Usage JSON visible resource"));
    assert!(!body.contains("Usage JSON hidden resource"));

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_usages/{}", usage_a.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let usage: serde_json::Value = serde_json::from_str(&body).expect("usage must be JSON");
    assert_eq!(
        usage["resource_name_snapshot"],
        "Usage JSON visible resource"
    );
    assert_eq!(usage["operator_name"], "Operator A");
    // Flat, SPA-friendly shape: a plain `id` string and ISO dates, never raw
    // MongoDB extended JSON (`_id` / `$oid` / `$date`).
    assert_eq!(usage["id"], usage_a.to_hex());
    assert!(usage.get("_id").is_none());
    assert!(!body.contains("$oid"));
    assert!(!body.contains("$date"));
    assert!(usage["started_at"].as_str().unwrap().contains('T'));

    let app = build_app(shared.clone());
    let (status, _body) = get_with_cookie(
        app,
        host_a,
        &format!("/api/admin/resource_usages/{}", usage_b.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let app = build_app(shared);
    let (status, _body) =
        get_with_cookie(app, host_a, "/api/admin/resource_usages", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_usage_json_mutations_manage_allocations_and_delete() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(
        &state,
        "Resource Usage Mutation",
        "resource-usage-mutation",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let admin_id = create_user(
        &state,
        "resource-usage-mutation-admin@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();
    let admin = get_user_by_id(&state, &admin_id).await.unwrap().unwrap();
    let token = create_session(&state, &admin.email).await.unwrap();
    let host = "resource-usage-mutation.miapp.local";

    let resource_id = create_resource(
        &state,
        &company,
        "Mutation resource",
        ResourceType::Machinery,
        true,
        None,
    )
    .await
    .unwrap();
    let status_id = create_concept_status(
        &state,
        &company,
        "In Progress",
        1,
        None,
        true,
        false,
        false,
        true,
    )
    .await
    .unwrap();
    let project_id = create_project(
        &state,
        &company,
        "Allocation Project",
        None,
        None,
        None,
        ProjectPriority::Medium,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let concept_a = create_project_concept(
        &state,
        &company,
        &project_id,
        Some(status_id.clone()),
        "Concept A",
        1.0,
        None,
        None,
        None,
        None,
        None,
        1,
    )
    .await
    .unwrap();
    let concept_b = create_project_concept(
        &state,
        &company,
        &project_id,
        Some(status_id),
        "Concept B",
        1.0,
        None,
        None,
        None,
        None,
        None,
        2,
    )
    .await
    .unwrap();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        "/api/admin/resource_usages",
        &token,
        serde_json::json!({
            "resource_id": resource_id.to_hex(),
            "started_at": "2026-06-01T10:00:00Z",
            "ended_at": "2026-06-01T12:00:00Z",
            "operator_name": "Mutation Operator",
            "notes": "Created through JSON",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_str(&body).expect("create response JSON");
    let usage_id = created["id"].as_str().expect("created id").to_string();

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}/update"),
        &token,
        serde_json::json!({
            "started_at": "2026-06-01T11:00:00Z",
            "ended_at": "2026-06-01T13:00:00Z",
            "hourly_cost_snapshot": 250.0,
            "operator_name": "Updated Operator",
            "notes": "Updated through JSON",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}/allocations"),
        &token,
        serde_json::json!({ "concept_ids": [concept_a.to_hex(), concept_b.to_hex()] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let usage_oid = bson::oid::ObjectId::parse_str(&usage_id).unwrap();
    let allocations = list_resource_usage_allocations(&state, &company, &usage_oid)
        .await
        .unwrap();
    assert_eq!(allocations.len(), 2);
    assert!(
        allocations
            .iter()
            .all(|allocation| (allocation.allocation_ratio - 0.5).abs() < 0.0001)
    );

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}/allocations"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(&concept_a.to_hex()));
    assert!(body.contains(&concept_b.to_hex()));

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}/allocations"),
        &token,
        serde_json::json!({
            "allocations": [
                { "concept_id": concept_a.to_hex(), "ratio": 0.7, "notes": "primary" },
                { "concept_id": concept_b.to_hex(), "ratio": 0.3, "notes": "secondary" }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let allocations = list_resource_usage_allocations(&state, &company, &usage_oid)
        .await
        .unwrap();
    assert_eq!(allocations.len(), 2);
    assert!(allocations.iter().any(|allocation| {
        allocation.concept_id == concept_a && (allocation.allocation_ratio - 0.7).abs() < 0.0001
    }));

    let app = build_app(shared.clone());
    let (status, body) = post_json_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}/delete"),
        &token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let allocations = list_resource_usage_allocations(&state, &company, &usage_oid)
        .await
        .unwrap();
    assert!(allocations.is_empty());

    let app = build_app(shared);
    let (status, _body) = get_with_cookie(
        app,
        host,
        &format!("/api/admin/resource_usages/{usage_id}"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn planned_entry_pay_endpoint_creates_transaction() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let user = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", user.company_slug);
    let token = create_session(&state, &user.email).await.unwrap();
    let company_id = user.company_id.clone();
    let project_id = create_project(
        &state,
        &company_id,
        "Proyecto Pago CFDI",
        None,
        None,
        None,
        ProjectPriority::Medium,
        Some(1000.0),
        None,
        None,
    )
    .await
    .unwrap();

    let entry = list_planned_entries(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.company_id == company_id && entry.id.is_some())
        .expect("seeded planned entry for active company");
    let entry_id = entry.id.clone().unwrap().to_hex();

    let account = list_accounts(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|account| {
            account.company_id == company_id && account.is_active && account.id.is_some()
        })
        .expect("seeded active account for active company");
    let account_id = account.id.unwrap().to_hex();
    let initial_transactions = list_transactions(&state).await.unwrap().len();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        &host,
        &format!("/admin/planned_entries/{entry_id}/pay?return_to=/tiempo"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pay form must render");
    assert!(body.contains("Registrar pago"));
    assert!(body.contains(&entry.name));
    assert!(body.contains(r#"name="return_to" value="/tiempo""#));

    let app = build_app(shared);
    let (status, location, _body) = post_form_with_cookie_response(
        app,
        &host,
        &format!("/admin/planned_entries/{entry_id}/pay"),
        &token,
        format!(
            "paid_at=2026-05-04&amount=123.45&account_id={account_id}&project_id={}&notes=Pago+de+prueba&return_to=/tiempo",
            project_id.to_hex()
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "pay submit must redirect");
    assert_eq!(location.as_deref(), Some("/tiempo"));

    let transactions = list_transactions(&state).await.unwrap();
    assert_eq!(transactions.len(), initial_transactions + 1);
    assert!(
        transactions.iter().any(|tx| tx.planned_entry_id == entry.id
            && tx.project_id == Some(project_id.clone())
            && (tx.amount - 123.45).abs() < f64::EPSILON),
        "payment must create linked transaction"
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn planned_entry_pay_validation_rerenders_form_instead_of_blank_page() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let user = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", user.company_slug);
    let token = create_session(&state, &user.email).await.unwrap();
    let company_id = user.company_id.clone();

    let entry = list_planned_entries(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.company_id == company_id && entry.id.is_some())
        .expect("seeded planned entry for active company");
    let entry_id = entry.id.clone().unwrap().to_hex();
    let initial_transactions = list_transactions(&state).await.unwrap().len();

    let app = build_app(shared);
    let (status, _location, body) = post_form_with_cookie_response(
        app,
        &host,
        &format!("/admin/planned_entries/{entry_id}/pay"),
        &token,
        "paid_at=2026-05-04&amount=123.45&account_id=not-an-id&return_to=/tiempo".into(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "invalid pay submit must re-render the form"
    );
    assert!(body.contains("Selecciona una cuenta válida"));
    assert!(body.contains(r#"name="return_to" value="/tiempo""#));
    assert_eq!(
        list_transactions(&state).await.unwrap().len(),
        initial_transactions
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn planned_entries_bulk_pay_creates_transactions() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let user = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", user.company_slug);
    let token = create_session(&state, &user.email).await.unwrap();
    let company_id = user.company_id.clone();

    let account = list_accounts(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|account| {
            account.company_id == company_id && account.is_active && account.id.is_some()
        })
        .expect("seeded active account for active company");
    let account_id = account.id.as_ref().unwrap().to_hex();
    let category = list_categories(&state)
        .await
        .unwrap()
        .into_iter()
        .find(|category| {
            category.company_id == company_id && category.flow_type == FlowType::Expense
        })
        .expect("seeded expense category for active company");
    create_planned_entry(
        &state,
        &company_id,
        None,
        None,
        None,
        "Pago masivo extra",
        FlowType::Expense,
        category.id.as_ref().unwrap(),
        account.id.as_ref().unwrap(),
        None,
        345.67,
        DateTime::parse_rfc3339_str("2026-05-05T00:00:00Z").unwrap(),
        PlannedStatus::Planned,
        None,
    )
    .await
    .unwrap();

    let entries: Vec<_> = list_planned_entries(&state)
        .await
        .unwrap()
        .into_iter()
        .filter(|entry| entry.company_id == company_id && entry.id.is_some())
        .filter(|entry| {
            !matches!(
                entry.status,
                PlannedStatus::Covered | PlannedStatus::Cancelled
            )
        })
        .take(2)
        .collect();
    assert_eq!(entries.len(), 2, "payable entries present");
    let entry_ids = entries
        .iter()
        .map(|entry| entry.id.as_ref().unwrap().to_hex())
        .collect::<Vec<_>>()
        .join(",");
    let initial_transactions = list_transactions(&state).await.unwrap().len();

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(
        app,
        &host,
        &format!("/admin/planned_entries/bulk_pay?ids={entry_ids}&return_to=/tiempo"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Pagar compromisos seleccionados"));

    let app = build_app(shared);
    let (status, location, _body) = post_form_with_cookie_response(
        app,
        &host,
        "/admin/planned_entries/bulk_pay",
        &token,
        format!(
            "entry_ids={entry_ids}&paid_at=2026-05-04&account_id={account_id}&return_to=/tiempo"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/tiempo"));
    assert_eq!(
        list_transactions(&state).await.unwrap().len(),
        initial_transactions + 2
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn users_update_can_add_allowed_company_membership() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let admin = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", admin.company_slug);
    let token = create_session(&state, &admin.email).await.unwrap();
    let primary_company_id = admin.company_id.clone();
    let extra_company_id = create_company(
        &state,
        "Compania Endpoint",
        "compania-endpoint",
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    add_user_to_company(&state, &admin.id, &extra_company_id, UserRole::Admin)
        .await
        .unwrap();

    let target_user_id = create_user(
        &state,
        "target-endpoint@example.com",
        "TARGETSECRET",
        &[(primary_company_id.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();

    let app = build_app(shared);
    let (status, location, body) = post_form_with_cookie_response(
        app,
        &host,
        &format!("/admin/users/{}/update", target_user_id.to_hex()),
        &token,
        format!(
            "email=target-endpoint%40example.com&secret=TARGETSECRET&company_ids={}&company_ids={}&role_{}=staff&role_{}=admin",
            primary_company_id.to_hex(),
            extra_company_id.to_hex(),
            primary_company_id.to_hex(),
            extra_company_id.to_hex()
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "user update failed with body: {body}"
    );
    assert_eq!(location.as_deref(), Some("/admin/users"));

    let target = get_user_by_id(&state, &target_user_id)
        .await
        .unwrap()
        .expect("target user exists");
    assert!(target.company_ids.contains(&primary_company_id));
    assert!(target.company_ids.contains(&extra_company_id));
    assert_eq!(
        target
            .company_ids
            .iter()
            .position(|id| id == &extra_company_id)
            .and_then(|idx| target.company_roles.get(idx)),
        Some(&UserRole::Admin)
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn staff_permissions_gate_project_resource_usage_and_timeline_routes() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = list_companies(&state).await.unwrap().remove(0);
    let company_id = company.id.clone().unwrap();
    let host = format!("{}.miapp.local", company.slug);
    create_project(
        &state,
        &company_id,
        "Proyecto Staff Permisos",
        None,
        None,
        None,
        ProjectPriority::Medium,
        Some(1000.0),
        None,
        None,
    )
    .await
    .unwrap();

    let restricted_id = create_user(
        &state,
        "staff-restricted@example.com",
        "SECRET",
        &[(company_id.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let permitted_id = create_user_with_permissions(
        &state,
        "staff-permitted@example.com",
        "SECRET",
        &[(
            company_id.clone(),
            UserRole::Staff,
            vec![
                UserPermission::ViewProjects,
                UserPermission::EditResourceUsageToday,
            ],
        )],
    )
    .await
    .unwrap();
    let timeline_id = create_user_with_permissions(
        &state,
        "staff-timeline@example.com",
        "SECRET",
        &[(
            company_id.clone(),
            UserRole::Staff,
            vec![UserPermission::ViewTimeline],
        )],
    )
    .await
    .unwrap();

    let restricted = get_user_by_id(&state, &restricted_id)
        .await
        .unwrap()
        .unwrap();
    let permitted = get_user_by_id(&state, &permitted_id)
        .await
        .unwrap()
        .unwrap();
    let timeline = get_user_by_id(&state, &timeline_id).await.unwrap().unwrap();
    let restricted_token = create_session(&state, &restricted.email).await.unwrap();
    let permitted_token = create_session(&state, &permitted.email).await.unwrap();
    let timeline_token = create_session(&state, &timeline.email).await.unwrap();
    let today_date = chrono::Utc::now().date_naive();
    let today = today_date.format("%Y-%m-%d").to_string();
    let four_days_ago = (today_date - chrono::Duration::days(4))
        .format("%Y-%m-%d")
        .to_string();
    let five_days_ago = (today_date - chrono::Duration::days(5))
        .format("%Y-%m-%d")
        .to_string();

    let app = build_app(shared.clone());
    let (status, _) = get_with_cookie(app, &host, "/admin/projects", &restricted_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, body) = get_with_cookie(app, &host, "/admin/projects", &permitted_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Proyecto Staff Permisos"));
    assert!(!body.contains("$1,000.00"));

    let app = build_app(shared.clone());
    let (status, _) = get_with_cookie(
        app,
        &host,
        &format!("/admin/resource_usages?date={today}&status_id=all"),
        &restricted_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, _) = get_with_cookie(
        app,
        &host,
        &format!("/admin/resource_usages?date={today}&status_id=all"),
        &permitted_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        "/admin/resource_usages",
        &permitted_token,
        format!("date={today}&status_id=all"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        "/admin/resource_usages",
        &permitted_token,
        format!("date={four_days_ago}&status_id=all"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        "/admin/resource_usages",
        &permitted_token,
        format!("date={five_days_ago}&status_id=all"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared.clone());
    let (status, _) = get_with_cookie(app, &host, "/tiempo", &restricted_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = build_app(shared);
    let (status, _) = get_with_cookie(app, &host, "/tiempo", &timeline_token).await;
    assert_eq!(status, StatusCode::OK);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn project_resource_endpoints_render_and_submit() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let user = list_users(&state).await.unwrap().remove(0);
    let host = format!("{}.miapp.local", user.company_slug);
    let token = create_session(&state, &user.email).await.unwrap();
    let company_id = user.company_id.clone();

    let project_id = create_project(
        &state,
        &company_id,
        "Proyecto Endpoint",
        None,
        None,
        Some("Descripcion endpoint".into()),
        ProjectPriority::Medium,
        Some(2500.0),
        None,
        None,
    )
    .await
    .unwrap();
    let resource_id = create_resource(
        &state,
        &company_id,
        "Recurso Endpoint",
        ResourceType::Equipment,
        true,
        Some("Notas recurso".into()),
    )
    .await
    .unwrap();
    let log_id = create_resource_log(
        &state,
        &company_id,
        Some(project_id.clone()),
        Some("Produccion".into()),
        Some(resource_id.clone()),
        Some("Recurso Endpoint".into()),
        DateTime::parse_rfc3339_str("2026-05-04T10:00:00Z").unwrap(),
        Some("Operador Endpoint".into()),
        None,
    )
    .await
    .unwrap();

    let get_cases = vec![
        (
            "/admin/projects".to_string(),
            "Proyecto Endpoint".to_string(),
        ),
        (
            "/admin/projects/new".to_string(),
            "Nuevo Proyecto".to_string(),
        ),
        (
            format!("/admin/projects/{}/edit", project_id.to_hex()),
            format!("/admin/projects/{}/update", project_id.to_hex()),
        ),
        (
            "/admin/resources".to_string(),
            "Recurso Endpoint".to_string(),
        ),
        (
            "/admin/resources/new".to_string(),
            "Nuevo Recurso".to_string(),
        ),
        (
            format!("/admin/resources/{}/edit", resource_id.to_hex()),
            format!("/admin/resources/{}/update", resource_id.to_hex()),
        ),
        (
            "/admin/resource_logs".to_string(),
            "Recurso Endpoint".to_string(),
        ),
        (
            "/admin/resource_logs/new".to_string(),
            "Nuevo Registro de Recurso".to_string(),
        ),
        (
            format!("/admin/resource_logs/{}/edit", log_id.to_hex()),
            format!("/admin/resource_logs/{}/update", log_id.to_hex()),
        ),
    ];

    for (path, expected) in get_cases {
        let app = build_app(shared.clone());
        let (status, body) = get_with_cookie(app, &host, &path, &token).await;
        assert_eq!(status, StatusCode::OK, "GET {path} must return 200");
        assert!(
            body.contains(&expected),
            "page {path} should contain expected text: {expected}"
        );
    }

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        &format!("/admin/projects/{}/update", project_id.to_hex()),
        &token,
        "title=Proyecto+Actualizado&description=Desc&contact_id=&category_id=&priority=high&total_budget=3000&scheduled_at=2026-05-20&notes=Notas".into(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "project update must redirect"
    );

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        &format!("/admin/resources/{}/update", resource_id.to_hex()),
        &token,
        "name=Recurso+Actualizado&resource_type=vehicle&is_active=on&notes=Notas".into(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "resource update must redirect"
    );

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        &format!("/admin/resource_logs/{}/update", log_id.to_hex()),
        &token,
        format!(
            "project_id={}&phase=Calidad&resource_id={}&started_at=2026-05-04T10%3A00&ended_at=2026-05-04T12%3A00&operator_name=Operador&notes=Notas",
            project_id.to_hex(),
            resource_id.to_hex()
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "resource log update must redirect"
    );

    let app = build_app(shared.clone());
    let status = post_form_with_cookie(
        app,
        &host,
        &format!("/admin/resource_logs/{}/end", log_id.to_hex()),
        &token,
        String::new(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "resource log end must redirect"
    );

    assert!(
        list_projects(&state, &company_id)
            .await
            .unwrap()
            .iter()
            .any(|project| project.title == "Proyecto Actualizado")
    );
    assert!(
        list_resources(&state, &company_id)
            .await
            .unwrap()
            .iter()
            .any(|resource| resource.name == "Recurso Actualizado")
    );
    assert!(
        list_resource_logs(&state, &company_id)
            .await
            .unwrap()
            .iter()
            .any(|log| log.phase.as_deref() == Some("Calidad") && log.duration_hours.is_some())
    );

    common::teardown(Some(ctx)).await;
}

// ---------------------------------------------------------------------------
// New JSON API coverage: users admin, resource usage grid bulk-save, SAT upload
// ---------------------------------------------------------------------------

async fn post_multipart_with_cookie(
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

#[tokio::test]
async fn users_json_api_crud_scopes_and_enforces_admin() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "Users JSON A", "users-json-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "Users JSON B", "users-json-b", "MXN", true, None)
        .await
        .unwrap();

    let admin_id = create_user_with_permissions(
        &state,
        "users-json-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "users-json-staff@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    let b_only_id = create_user_with_permissions(
        &state,
        "users-json-bonly@example.com",
        "SECRET",
        &[(company_b.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();

    let admin_token = create_session(&state, "users-json-admin@example.com")
        .await
        .unwrap();
    let staff_token = create_session(&state, "users-json-staff@example.com")
        .await
        .unwrap();
    let host = "users-json-a.miapp.local";

    let (status, body) =
        get_with_cookie(build_app(shared.clone()), host, "/api/admin/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let users: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert!(users.iter().any(|u| u["email"] == "users-json-admin@example.com"));
    assert!(users.iter().any(|u| u["email"] == "users-json-staff@example.com"));
    assert!(!users.iter().any(|u| u["email"] == "users-json-bonly@example.com"));
    assert!(users.iter().all(|u| u.get("secret").is_none()));

    let (status, _) =
        get_with_cookie(build_app(shared.clone()), host, "/api/admin/users", &staff_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/users",
        &admin_token,
        serde_json::json!({
            "email": "users-json-new@example.com",
            "memberships": [
                { "company_id": company_a.to_hex(), "role": "staff", "permissions": ["view_projects"] }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let new_id = created["id"].as_str().unwrap().to_string();
    assert!(created.get("secret").is_none());

    // membership only in a company the admin does not administer -> rejected
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/users",
        &admin_token,
        serde_json::json!({
            "email": "users-json-bad@example.com",
            "memberships": [ { "company_id": company_b.to_hex(), "role": "admin" } ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, body) = get_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/users/{new_id}"),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let detail: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(detail["email"], "users-json-new@example.com");
    assert_eq!(detail["memberships"][0]["company_id"], company_a.to_hex());
    assert_eq!(detail["memberships"][0]["role"], "staff");

    // user that only belongs to company B is invisible to company A admin
    let (status, _) = get_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/users/{}", b_only_id.to_hex()),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/users/{new_id}/update"),
        &admin_token,
        serde_json::json!({
            "email": "users-json-updated@example.com",
            "memberships": [ { "company_id": company_a.to_hex(), "role": "admin" } ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let new_oid = bson::oid::ObjectId::parse_str(&new_id).unwrap();
    let updated = get_user_by_id(&state, &new_oid).await.unwrap().unwrap();
    assert_eq!(updated.email, "users-json-updated@example.com");
    assert_eq!(updated.role, UserRole::Admin);

    // cannot delete yourself
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/users/{}/delete", admin_id.to_hex()),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/users/{new_id}/delete"),
        &admin_token,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(get_user_by_id(&state, &new_oid).await.unwrap().is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_usages_grid_json_saves_and_enforces_today_window() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(&state, "Grid JSON Co", "grid-json-co", "MXN", true, None)
        .await
        .unwrap();
    create_user_with_permissions(
        &state,
        "grid-json-admin@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "grid-json-staff-noperm@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "grid-json-staff-today@example.com",
        "SECRET",
        &[(
            company.clone(),
            UserRole::Staff,
            vec![UserPermission::EditResourceUsageToday],
        )],
    )
    .await
    .unwrap();
    let admin_token = create_session(&state, "grid-json-admin@example.com")
        .await
        .unwrap();
    let staff_noperm_token = create_session(&state, "grid-json-staff-noperm@example.com")
        .await
        .unwrap();
    let staff_today_token = create_session(&state, "grid-json-staff-today@example.com")
        .await
        .unwrap();
    let host = "grid-json-co.miapp.local";

    let status_id = create_concept_status(
        &state, &company, "Produccion", 1, None, true, false, false, true,
    )
    .await
    .unwrap();
    let project_id = create_project(
        &state,
        &company,
        "Grid Project",
        None,
        None,
        None,
        ProjectPriority::Medium,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let concept_id = create_project_concept(
        &state,
        &company,
        &project_id,
        Some(status_id.clone()),
        "Grid Concept",
        1.0,
        Some("job".into()),
        None,
        None,
        None,
        None,
        1,
    )
    .await
    .unwrap();
    let resource_id = create_resource(
        &state,
        &company,
        "Grid Machine",
        ResourceType::Machinery,
        true,
        None,
    )
    .await
    .unwrap();
    update_resource_allowed_statuses(&state, &resource_id, &company, vec![status_id.clone()])
        .await
        .unwrap();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/resource_usages/grid",
        &admin_token,
        serde_json::json!({
            "date": today,
            "status_id": "all",
            "selections": [
                { "concept_id": concept_id.to_hex(), "hour": 8, "resource_id": resource_id.to_hex() }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let usages = list_resource_usages(&state, &company).await.unwrap();
    assert_eq!(usages.len(), 1, "one usage should be created for the selected hour");
    let usage_id = usages[0].id.clone().unwrap();
    let allocations = list_resource_usage_allocations(&state, &company, &usage_id)
        .await
        .unwrap();
    assert!(allocations.iter().any(|a| a.concept_id == concept_id));

    // staff without the today permission cannot save
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/resource_usages/grid",
        &staff_noperm_token,
        serde_json::json!({ "date": today, "selections": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // staff with the today permission can save today's grid
    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/resource_usages/grid",
        &staff_today_token,
        serde_json::json!({ "date": today, "status_id": "all", "selections": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // ...but not a date outside the 4-day window
    let old_date = (chrono::Utc::now() - chrono::Duration::days(10))
        .format("%Y-%m-%d")
        .to_string();
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/resource_usages/grid",
        &staff_today_token,
        serde_json::json!({ "date": old_date, "selections": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // malformed date is a client error
    let (status, _) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/resource_usages/grid",
        &admin_token,
        serde_json::json!({ "date": "not-a-date", "selections": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn sat_config_upload_json_creates_config_and_enforces_admin() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(&state, "SAT Upload Co", "sat-upload-co", "MXN", true, None)
        .await
        .unwrap();
    create_user_with_permissions(
        &state,
        "sat-upload-admin@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    create_user_with_permissions(
        &state,
        "sat-upload-staff@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    let admin_token = create_session(&state, "sat-upload-admin@example.com")
        .await
        .unwrap();
    let staff_token = create_session(&state, "sat-upload-staff@example.com")
        .await
        .unwrap();
    let host = "sat-upload-co.miapp.local";

    let cert_bytes: &[u8] = b"\x30\x82DUMMYCERTDATA";
    let key_bytes: &[u8] = b"\x30\x82DUMMYKEYDATA";
    let (status, body) = post_multipart_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/sat-configs/upload",
        &admin_token,
        &[
            ("rfc", None, b"aaa010101aaa"),
            ("label", None, b"Test FIEL"),
            ("key_password", None, b"supersecret"),
            ("cer_file", Some("cert.cer"), cert_bytes),
            ("key_file", Some("private.key"), key_bytes),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(created["id"].as_str().is_some());

    let (status, body) = get_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/sat-configs",
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("AAA010101AAA"), "RFC should be uppercased: {body}");
    assert!(body.contains("Test FIEL"));
    assert!(!body.contains("supersecret"));
    assert!(!body.contains("private.key"));
    assert!(!body.contains("cert.cer"));

    // missing key_file -> validation error
    let (status, _) = post_multipart_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/sat-configs/upload",
        &admin_token,
        &[
            ("rfc", None, b"bbb010101bbb"),
            ("key_password", None, b"x"),
            ("cer_file", Some("c.cer"), b"data"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // staff cannot upload
    let (status, _) = post_multipart_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/admin/sat-configs/upload",
        &staff_token,
        &[
            ("rfc", None, b"ccc010101ccc"),
            ("key_password", None, b"x"),
            ("cer_file", Some("c.cer"), b"data"),
            ("key_file", Some("k.key"), b"data"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(format!("uploads/sat/{}", company.to_hex()));

    common::teardown(Some(ctx)).await;
}

// ---------------------------------------------------------------------------
// Security: authentication enforcement + cross-tenant isolation
//
// These tests treat the API as hostile input: an authenticated user of tenant A
// must never be able to read, mutate, or delete a record that belongs to tenant
// B, and every protected endpoint must reject requests with no session.
// ---------------------------------------------------------------------------

async fn assert_requires_auth_get(shared: &Arc<AppState>, path: &str) {
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

async fn assert_requires_auth_post(shared: &Arc<AppState>, path: &str) {
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

async fn assert_get_denied_no_leak(
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

async fn assert_post_denied(
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

#[tokio::test]
async fn protected_endpoints_require_authentication() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let shared = Arc::new(ctx.state.clone());

    // A representative read endpoint from every resource group.
    for path in [
        "/api/account",
        "/api/me/companies",
        "/api/admin/users",
        "/api/admin/companies",
        "/api/admin/accounts",
        "/api/admin/categories",
        "/api/admin/contacts",
        "/api/admin/forecasts",
        "/api/admin/recurring-plans",
        "/api/admin/planned-entries",
        "/api/admin/transactions/data",
        "/api/admin/orders",
        "/api/admin/projects",
        "/api/admin/resources",
        "/api/admin/resource_logs",
        "/api/admin/resource_usages",
        "/api/admin/concept_statuses",
        "/api/admin/sat-configs",
        "/api/admin/cfdis/data",
        "/api/tiempo",
    ] {
        assert_requires_auth_get(&shared, path).await;
    }

    // Representative write endpoints, including the three newly added ones.
    for path in [
        "/api/admin/users",
        "/api/admin/accounts",
        "/api/admin/companies",
        "/api/admin/sat-configs/upload",
        "/api/admin/resource_usages/grid",
        "/api/admin/transactions",
        "/pdf/preview",
    ] {
        assert_requires_auth_post(&shared, path).await;
    }

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn cross_tenant_record_access_is_denied() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "Tenant A", "tenant-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "Tenant B", "tenant-b", "MXN", true, None)
        .await
        .unwrap();

    // Attacker: admin of tenant A ONLY, probing tenant B's record ids.
    create_user_with_permissions(
        &state,
        "tenant-a-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "tenant-a-admin@example.com")
        .await
        .unwrap();
    let host = "tenant-a.miapp.local";

    // Seed tenant B records with unique, greppable needles.
    let b_account = create_account(
        &state,
        &company_b,
        "tenant-b-secret-account",
        AccountType::Bank,
        "MXN",
        true,
        None,
    )
    .await
    .unwrap();
    let b_category = create_category(
        &state,
        &company_b,
        "tenant-b-secret-category",
        FlowType::Expense,
        None,
        None,
    )
    .await
    .unwrap();
    let b_contact = create_contact(
        &state,
        &company_b,
        "tenant-b-secret-contact",
        ContactType::Customer,
        None,
        Some("tenant-b-secret@example.com".into()),
        None,
        None,
    )
    .await
    .unwrap();
    let b_forecast = create_forecast(
        &state,
        &company_b,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        None,
        DateTime::parse_rfc3339_str("2026-01-01T00:00:00Z").unwrap(),
        DateTime::parse_rfc3339_str("2026-12-31T00:00:00Z").unwrap(),
        "MXN",
        1000.0,
        500.0,
        500.0,
        None,
        None,
        None,
        Some("tenant-b-secret-forecast".into()),
        None,
    )
    .await
    .unwrap();

    // Reads by id must be denied and must not leak the needle.
    assert_get_denied_no_leak(
        &shared,
        host,
        &token,
        &format!("/api/admin/accounts/{}", b_account.to_hex()),
        "tenant-b-secret-account",
    )
    .await;
    assert_get_denied_no_leak(
        &shared,
        host,
        &token,
        &format!("/api/admin/categories/{}", b_category.to_hex()),
        "tenant-b-secret-category",
    )
    .await;
    assert_get_denied_no_leak(
        &shared,
        host,
        &token,
        &format!("/api/admin/contacts/{}", b_contact.to_hex()),
        "tenant-b-secret",
    )
    .await;
    assert_get_denied_no_leak(
        &shared,
        host,
        &token,
        &format!("/api/admin/forecasts/{}", b_forecast.to_hex()),
        "tenant-b-secret-forecast",
    )
    .await;

    // List endpoints must only return tenant A's data (which is empty here).
    for (path, needle) in [
        ("/api/admin/accounts", "tenant-b-secret-account"),
        ("/api/admin/categories", "tenant-b-secret-category"),
        ("/api/admin/contacts", "tenant-b-secret"),
        ("/api/admin/forecasts", "tenant-b-secret-forecast"),
    ] {
        let (status, body) = get_with_cookie(build_app(shared.clone()), host, path, &token).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert!(
            !body.contains(needle),
            "tenant B data leaked in list {path}: {body}"
        );
    }

    // Cross-tenant deletes must be denied...
    for path in [
        format!("/api/admin/accounts/{}/delete", b_account.to_hex()),
        format!("/api/admin/categories/{}/delete", b_category.to_hex()),
        format!("/api/admin/contacts/{}/delete", b_contact.to_hex()),
        format!("/api/admin/forecasts/{}/delete", b_forecast.to_hex()),
    ] {
        assert_post_denied(&shared, host, &token, &path, serde_json::json!({})).await;
    }

    // ...and cross-tenant updates with fully valid payloads (so the request
    // reaches the handler's tenant check rather than failing body validation)
    // must also be denied for every resource.
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/accounts/{}/update", b_account.to_hex()),
        serde_json::json!({ "name": "hijacked", "account_type": "bank" }),
    )
    .await;
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/categories/{}/update", b_category.to_hex()),
        serde_json::json!({ "name": "hijacked", "flow_type": "expense" }),
    )
    .await;
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/contacts/{}/update", b_contact.to_hex()),
        serde_json::json!({ "name": "hijacked", "contact_type": "customer" }),
    )
    .await;
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/forecasts/{}/update", b_forecast.to_hex()),
        serde_json::json!({
            "generated_at": "2026-01-01T00:00:00Z",
            "start_date": "2026-01-01T00:00:00Z",
            "end_date": "2026-12-31T00:00:00Z",
            "currency": "MXN",
            "projected_income_total": 1.0,
            "projected_expense_total": 1.0,
            "projected_net": 0.0
        }),
    )
    .await;

    // The tenant B records must still exist and be unchanged.
    assert!(
        list_accounts(&state)
            .await
            .unwrap()
            .iter()
            .any(|a| a.id.as_ref() == Some(&b_account) && a.name == "tenant-b-secret-account"),
        "tenant B account must survive cross-tenant attacks unchanged"
    );
    assert!(
        list_categories(&state)
            .await
            .unwrap()
            .iter()
            .any(|c| c.id.as_ref() == Some(&b_category) && c.name == "tenant-b-secret-category"),
        "tenant B category must survive cross-tenant attacks unchanged"
    );
    assert!(
        list_contacts(&state)
            .await
            .unwrap()
            .iter()
            .any(|c| c.id.as_ref() == Some(&b_contact) && c.name == "tenant-b-secret-contact"),
        "tenant B contact must survive cross-tenant attacks unchanged"
    );
    assert!(
        list_forecasts(&state)
            .await
            .unwrap()
            .iter()
            .any(|f| f.id.as_ref() == Some(&b_forecast)),
        "tenant B forecast must survive cross-tenant delete"
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn cross_tenant_user_mutations_are_denied() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "User Tenant A", "user-tenant-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "User Tenant B", "user-tenant-b", "MXN", true, None)
        .await
        .unwrap();

    create_user_with_permissions(
        &state,
        "user-tenant-a-admin@example.com",
        "SECRET",
        &[(company_a.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    // Victim belongs ONLY to tenant B.
    let b_user = create_user_with_permissions(
        &state,
        "victim-tenant-b@example.com",
        "VICTIMSECRET",
        &[(company_b.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "user-tenant-a-admin@example.com")
        .await
        .unwrap();
    let host = "user-tenant-a.miapp.local";

    // Read, update, and delete of the tenant B user are all denied.
    assert_get_denied_no_leak(
        &shared,
        host,
        &token,
        &format!("/api/admin/users/{}", b_user.to_hex()),
        "victim-tenant-b@example.com",
    )
    .await;
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/users/{}/update", b_user.to_hex()),
        serde_json::json!({
            "email": "hijacked@example.com",
            "memberships": [ { "company_id": company_a.to_hex(), "role": "admin" } ]
        }),
    )
    .await;
    assert_post_denied(
        &shared,
        host,
        &token,
        &format!("/api/admin/users/{}/delete", b_user.to_hex()),
        serde_json::json!({}),
    )
    .await;

    // The tenant A admin must not see the tenant B user in their listing.
    let (status, body) =
        get_with_cookie(build_app(shared.clone()), host, "/api/admin/users", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("victim-tenant-b@example.com"),
        "tenant B user leaked into tenant A user list: {body}"
    );

    // Victim is intact: still exists, email and secret unchanged.
    let victim = get_user_by_id(&state, &b_user).await.unwrap().unwrap();
    assert_eq!(victim.email, "victim-tenant-b@example.com");
    assert_eq!(victim.secret, "VICTIMSECRET");

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn staff_cannot_reach_admin_json_endpoints() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(&state, "Staff Guard Co", "staff-guard-co", "MXN", true, None)
        .await
        .unwrap();
    create_user_with_permissions(
        &state,
        "staff-guard@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "staff-guard@example.com")
        .await
        .unwrap();
    let host = "staff-guard-co.miapp.local";

    // Admin-only reads must be forbidden for a staff member.
    for path in [
        "/api/admin/users",
        "/api/admin/companies",
        "/api/admin/accounts",
        "/api/admin/categories",
        "/api/admin/contacts",
        "/api/admin/forecasts",
        "/api/admin/recurring-plans",
        "/api/admin/transactions/data",
        "/api/admin/orders",
        "/api/admin/sat-configs",
    ] {
        let (status, body) = get_with_cookie(build_app(shared.clone()), host, path, &token).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "staff must be forbidden on {path}, got {status}: {body}"
        );
    }

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn pdf_preview_renders_for_authenticated_user() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(&state, "PDF Co", "pdf-co", "MXN", true, None)
        .await
        .unwrap();
    create_user_with_permissions(
        &state,
        "pdf-user@example.com",
        "SECRET",
        &[(company.clone(), UserRole::Staff, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "pdf-user@example.com").await.unwrap();
    let host = "pdf-co.miapp.local";

    // An authenticated user gets a JSON envelope (200) regardless of whether the
    // typst binary is present; the handler degrades gracefully to ok:false.
    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/pdf/preview",
        &token,
        serde_json::json!({ "source": "= Test\n\nHello from the test suite." }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let resp: serde_json::Value = serde_json::from_str(&body).expect("preview must be JSON");
    assert!(resp.get("ok").and_then(|v| v.as_bool()).is_some());

    // If typst is available on this machine, the preview must actually compile a
    // real PDF (base64 that decodes to a `%PDF` document).
    let typst_available = std::process::Command::new(
        std::env::var("TYPST_BIN").unwrap_or_else(|_| "typst".to_string()),
    )
    .arg("--version")
    .output()
    .map(|o| o.status.success())
    .unwrap_or(false);

    if typst_available {
        assert_eq!(resp["ok"], true, "typst is installed, preview should succeed: {body}");
        let b64 = resp["pdf_base64"].as_str().expect("pdf_base64 present");
        let bytes = data_encoding::BASE64.decode(b64.as_bytes()).expect("valid base64");
        assert!(
            bytes.starts_with(b"%PDF"),
            "decoded preview must be a real PDF document"
        );
    }

    common::teardown(Some(ctx)).await;
}
