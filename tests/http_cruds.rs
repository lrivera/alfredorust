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
        AccountType, FlowType, PlannedStatus, ProjectPriority, ResourceType, TransactionType,
        UserPermission, UserRole,
    },
    routes,
    session::{SESSION_COOKIE_NAME, require_session},
    state::{
        AppState, add_user_to_company, create_account, create_category, create_company,
        create_forecast, create_planned_entry, create_project, create_recurring_plan,
        create_resource, create_resource_log, create_resource_usage, create_session,
        create_transaction, create_user, create_user_with_permissions, get_user_by_id,
        list_accounts, list_categories, list_companies, list_contacts, list_forecasts,
        list_planned_entries, list_projects, list_recurring_plans, list_resource_logs,
        list_resources, list_transactions, list_users,
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
            "/admin/users",
            get(routes::users_index).post(routes::users_create),
        )
        .route("/admin/users/new", get(routes::users_new))
        .route("/admin/users/{id}/edit", get(routes::users_edit))
        .route("/admin/users/{id}/update", post(routes::users_update))
        .route("/admin/users/{id}/delete", post(routes::users_delete))
        .route("/admin/users/{id}/qrcode", get(routes::users_qrcode))
        .route("/pdf", get(routes::pdf_editor))
        .route("/tiempo", get(routes::tiempo_page))
        .route("/api/me/companies", get(routes::me_companies))
        .route("/admin/companies", get(routes::companies_index))
        .route("/admin/companies/new", get(routes::companies_new))
        .route("/admin/companies/{id}/edit", get(routes::companies_edit))
        .route("/admin/cfdis", get(routes::cfdis_index))
        .route("/api/admin/cfdis/data", get(routes::cfdis_data_api))
        .route("/api/admin/cfdis/{uuid}", get(routes::cfdi_data_api))
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
            get(routes::recurring_plans_data_api),
        )
        .route(
            "/api/admin/recurring-plans/{id}",
            get(routes::recurring_plan_data_api),
        )
        .route(
            "/admin/recurring_plans/new",
            get(routes::recurring_plans_new),
        )
        .route(
            "/admin/recurring_plans/{id}/edit",
            get(routes::recurring_plans_edit),
        )
        // POST routes for recurring plans omitted in tests (use private types)
        .route(
            "/admin/planned_entries",
            get(routes::planned_entries_index).post(routes::planned_entries_create),
        )
        .route(
            "/api/admin/planned-entries",
            get(routes::planned_entries_data_api),
        )
        .route(
            "/api/admin/planned-entries/{id}",
            get(routes::planned_entry_data_api),
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
        // POST routes for planned entries omitted in tests (use private types)
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
            "/api/admin/transactions/{id}",
            get(routes::transaction_data_api),
        )
        .route(
            "/admin/forecasts",
            get(routes::forecasts_index).post(routes::forecasts_create),
        )
        .route("/api/admin/forecasts", get(routes::forecasts_data_api))
        .route("/api/admin/forecasts/{id}", get(routes::forecast_data_api))
        .route("/admin/forecasts/new", get(routes::forecasts_new))
        .route("/admin/forecasts/{id}/edit", get(routes::forecasts_edit))
        // POST routes for forecasts omitted in tests (use private types)
        .route(
            "/admin/projects",
            get(routes::projects_index).post(routes::projects_create),
        )
        .route("/api/admin/projects", get(routes::projects_data_api))
        .route("/api/admin/projects/{id}", get(routes::project_data_api))
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
        .route("/api/admin/resources", get(routes::resources_data_api))
        .route("/api/admin/resources/{id}", get(routes::resource_data_api))
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
            get(routes::resource_logs_data_api),
        )
        .route(
            "/api/admin/resource_logs/{id}",
            get(routes::resource_log_data_api),
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
            "/api/admin/resource_usages/{id}",
            get(routes::api_resource_usage_detail),
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
