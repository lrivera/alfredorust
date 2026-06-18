#[path = "common/mod.rs"]
mod common;

use common::harness::*;

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
    let token = create_session(&state, &admin.username).await.unwrap();
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
    let restricted_token = create_session(&state, &restricted.username).await.unwrap();
    let permitted_token = create_session(&state, &permitted.username).await.unwrap();
    let timeline_token = create_session(&state, &timeline.username).await.unwrap();
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
    let token = create_session(&state, &user.username).await.unwrap();
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
    assert!(users.iter().any(|u| u["username"] == "users-json-admin@example.com"));
    assert!(users.iter().any(|u| u["username"] == "users-json-staff@example.com"));
    assert!(!users.iter().any(|u| u["username"] == "users-json-bonly@example.com"));
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
            "username": "users-json-new@example.com",
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
            "username": "users-json-bad@example.com",
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
    assert_eq!(detail["username"], "users-json-new@example.com");
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
            "username": "users-json-updated@example.com",
            "memberships": [ { "company_id": company_a.to_hex(), "role": "admin" } ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let new_oid = bson::oid::ObjectId::parse_str(&new_id).unwrap();
    let updated = get_user_by_id(&state, &new_oid).await.unwrap().unwrap();
    assert_eq!(updated.username, "users-json-updated@example.com");
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
            "username": "hijacked@example.com",
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
    assert_eq!(victim.username, "victim-tenant-b@example.com");
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


#[tokio::test]
async fn me_endpoint_bootstraps_active_tenant_profile_and_companies() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let company_a = create_company(&state, "Me Bootstrap A", "me-boot-a", "MXN", true, None)
        .await
        .unwrap();
    let company_b = create_company(&state, "Me Bootstrap B", "me-boot-b", "MXN", true, None)
        .await
        .unwrap();

    // Admin on A (with a permission), Staff on B (different permission), so we
    // can prove the active-tenant role/permissions follow the request host.
    let user_id = create_user_with_permissions(
        &state,
        "me-boot@example.com",
        "TOTPSECRET",
        &[
            (
                company_a.clone(),
                UserRole::Admin,
                vec![UserPermission::ViewProjects],
            ),
            (
                company_b.clone(),
                UserRole::Staff,
                vec![UserPermission::ViewTimeline],
            ),
        ],
    )
    .await
    .unwrap();
    let user = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    let token = create_session(&state, &user.username).await.unwrap();

    // Bootstrap on tenant A's subdomain.
    let (status, body) = get_with_cookie(
        build_app(shared.clone()),
        "me-boot-a.miapp.local",
        "/api/me",
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "authenticated /api/me must succeed");

    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(json["username"], "me-boot@example.com");
    assert_eq!(json["company"], "Me Bootstrap A");
    assert_eq!(json["company_slug"], "me-boot-a");
    assert_eq!(json["role"], "admin");
    let perms: Vec<String> = json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect();
    assert!(
        perms.contains(&UserPermission::ViewProjects.as_str().to_string()),
        "active-tenant (A) permissions must be returned, got {perms:?}"
    );

    // Both memberships are listed, with A flagged active.
    let companies = json["companies"].as_array().unwrap();
    assert_eq!(companies.len(), 2, "both memberships must be listed");
    let a = companies
        .iter()
        .find(|c| c["slug"] == "me-boot-a")
        .expect("company A present");
    let b = companies
        .iter()
        .find(|c| c["slug"] == "me-boot-b")
        .expect("company B present");
    assert_eq!(a["active"], true, "host-resolved company A is active");
    assert_eq!(b["active"], false, "company B is not active on host A");

    // The TOTP secret must never leak through the bootstrap payload.
    assert!(
        !body.contains("TOTPSECRET"),
        "/api/me must not expose the TOTP secret"
    );

    // Without a session cookie the endpoint is unauthorized.
    let req = Request::builder()
        .uri("/api/me")
        .header("host", "me-boot-a.miapp.local")
        .body(Body::empty())
        .unwrap();
    let res = build_app(shared.clone())
        .oneshot(req)
        .await
        .expect("request failed");
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "/api/me without a session must be 401"
    );

    common::teardown(Some(ctx)).await;
}

fn build_test_gated(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/test", get(routes::test_dashboard))
        .layer(middleware::from_fn(require_test_tenant))
        .layer(middleware::from_fn_with_state(state.clone(), require_session))
        .with_state(state)
}


#[tokio::test]
async fn test_tooling_is_gated_to_login_and_the_test_tenant() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let shared = Arc::new(state.clone());

    let test_co = create_company(&state, "Test Co", "test", "MXN", true, None)
        .await
        .unwrap();
    let other_co = create_company(&state, "Other Co", "other-co", "MXN", true, None)
        .await
        .unwrap();
    create_user_with_permissions(&state, "t-tenant@example.com", "S", &[(test_co, UserRole::Admin, vec![])])
        .await
        .unwrap();
    create_user_with_permissions(&state, "o-tenant@example.com", "S", &[(other_co, UserRole::Admin, vec![])])
        .await
        .unwrap();
    let t_token = create_session(&state, "t-tenant@example.com").await.unwrap();
    let o_token = create_session(&state, "o-tenant@example.com").await.unwrap();

    // Unauthenticated → 401 (require_session runs first).
    let req = Request::builder()
        .uri("/test")
        .header("host", "test.miapp.local")
        .body(Body::empty())
        .unwrap();
    let res = build_test_gated(shared.clone())
        .oneshot(req)
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Logged in on the test tenant → 200.
    let (status, body) =
        get_with_cookie(build_test_gated(shared.clone()), "test.miapp.local", "/test", &t_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Test tooling"));

    // Logged in on a NON-test tenant → 404 (invisible).
    let (status, _) = get_with_cookie(
        build_test_gated(shared.clone()),
        "other-co.miapp.local",
        "/test",
        &o_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::teardown(Some(ctx)).await;
}

