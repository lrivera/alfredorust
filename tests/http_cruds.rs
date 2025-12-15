#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
    routing::{get, post},
    Router,
};
use tower::ServiceExt; // for oneshot

use alfredodev::{
    routes,
    session::{require_session, SESSION_COOKIE_NAME},
    state::{
        create_session, list_accounts, list_categories, list_companies, list_contacts,
        list_forecasts, list_planned_entries, list_recurring_plans, list_transactions, list_users,
        AppState,
    },
};

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
            "/admin/forecasts",
            get(routes::forecasts_index).post(routes::forecasts_create),
        )
        .route("/admin/forecasts/new", get(routes::forecasts_new))
        .route("/admin/forecasts/{id}/edit", get(routes::forecasts_edit))
        // POST routes for forecasts omitted in tests (use private types)
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
    assert!(!planned_entries.is_empty(), "seeded planned entries present");
    assert!(!transactions.is_empty(), "seeded transactions present");
    assert!(!forecasts.is_empty(), "seeded forecasts present");

    let cases = vec![
        ("/admin/accounts", accounts[0].name.clone()),
        ("/admin/categories", categories[0].name.clone()),
        ("/admin/contacts", contacts[0].name.clone()),
        ("/admin/recurring_plans", plans[0].name.clone()),
        ("/admin/planned_entries", planned_entries[0].name.clone()),
        ("/admin/transactions", transactions[0].description.clone()),
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
