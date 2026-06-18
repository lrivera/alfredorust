#[path = "common/mod.rs"]
mod common;

use common::harness::*;

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

/// A blank `secret` on the account update keeps the existing one, so a user can
/// save an email change without knowing or rotating their TOTP secret.
#[tokio::test]
async fn account_json_blank_secret_keeps_existing() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company = create_company(&state, "Account Keep Co", "account-keep-co", "MXN", true, None)
        .await
        .unwrap();
    let user_id = create_user_with_permissions(
        &state,
        "account-keep@example.com",
        "KEEPME",
        &[(company.clone(), UserRole::Admin, vec![])],
    )
    .await
    .unwrap();
    let token = create_session(&state, "account-keep@example.com")
        .await
        .unwrap();
    let host = "account-keep-co.miapp.local";

    let (status, body) = post_json_with_cookie(
        build_app(shared.clone()),
        host,
        "/api/account",
        &token,
        serde_json::json!({
            "email": "account-keep-renamed@example.com",
            "secret": ""
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert_eq!(updated.email, "account-keep-renamed@example.com");
    assert_eq!(updated.secret, "KEEPME", "blank secret must keep the old one");

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


