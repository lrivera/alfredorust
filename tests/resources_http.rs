#[path = "common/mod.rs"]
mod common;

use common::harness::*;

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

    // The JSON grid VIEW reflects the saved selection (concept row, hour-8 cell,
    // the allowed resource marked selected). This is what the SPA renders.
    let (status, body) = get_with_cookie(
        build_app(shared.clone()),
        host,
        &format!("/api/admin/resource_usages/grid?date={today}&status_id=all"),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let grid: serde_json::Value = serde_json::from_str(&body).expect("valid grid JSON");
    let rows = grid["rows"].as_array().expect("rows array");
    let row = rows
        .iter()
        .find(|r| r["concept_id"] == concept_id.to_hex())
        .expect("a row for the seeded concept");
    let cell = row["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["hour"] == 8)
        .expect("hour-8 cell");
    let resource = cell["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["resource_id"] == resource_id.to_hex())
        .expect("the allowed resource appears in the cell");
    assert_eq!(resource["selected"], true, "saved selection must be marked selected");

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


