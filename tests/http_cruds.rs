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
    models::{ProjectPriority, ResourceType},
    routes,
    session::{SESSION_COOKIE_NAME, require_session},
    state::{
        AppState, create_project, create_resource, create_resource_log, create_session,
        list_accounts, list_categories, list_companies, list_contacts, list_forecasts,
        list_planned_entries, list_projects, list_recurring_plans, list_resource_logs,
        list_resources, list_transactions, list_users,
    },
};
use bson::DateTime;

fn build_app(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/setup", get(routes::setup))
        .route("/qrcode", get(routes::qrcode))
        .route("/secret", get(routes::secret_generate))
        .route("/api/tiempo", get(routes::tiempo_data))
        .route("/logout", post(routes::logout))
        .route("/admin/users", get(routes::users_index))
        .route("/admin/users/new", get(routes::users_new))
        .route("/admin/users/{id}/edit", get(routes::users_edit))
        .route("/admin/users/{id}/delete", post(routes::users_delete))
        .route("/admin/users/{id}/qrcode", get(routes::users_qrcode))
        .route("/pdf", get(routes::pdf_editor))
        .route("/tiempo", get(routes::tiempo_page))
        .route("/api/me/companies", get(routes::me_companies))
        .route("/admin/companies", get(routes::companies_index))
        .route("/admin/companies/new", get(routes::companies_new))
        .route("/admin/companies/{id}/edit", get(routes::companies_edit))
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
        // POST routes for recurring plans omitted in tests (use private types)
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
            "/admin/forecasts",
            get(routes::forecasts_index).post(routes::forecasts_create),
        )
        .route("/admin/forecasts/new", get(routes::forecasts_new))
        .route("/admin/forecasts/{id}/edit", get(routes::forecasts_edit))
        // POST routes for forecasts omitted in tests (use private types)
        .route(
            "/admin/projects",
            get(routes::projects_index).post(routes::projects_create),
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
        &format!("/admin/planned_entries/{entry_id}/pay"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pay form must render");
    assert!(body.contains("Registrar pago"));
    assert!(body.contains(&entry.name));

    let app = build_app(shared);
    let status = post_form_with_cookie(
        app,
        &host,
        &format!("/admin/planned_entries/{entry_id}/pay"),
        &token,
        format!("paid_at=2026-05-04&amount=123.45&account_id={account_id}&notes=Pago+de+prueba"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "pay submit must redirect");

    let transactions = list_transactions(&state).await.unwrap();
    assert_eq!(transactions.len(), initial_transactions + 1);
    assert!(
        transactions
            .iter()
            .any(|tx| tx.planned_entry_id == entry.id && (tx.amount - 123.45).abs() < f64::EPSILON),
        "payment must create linked transaction"
    );

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
