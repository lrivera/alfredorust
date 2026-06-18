#[path = "common/mod.rs"]
mod common;

use common::harness::*;

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
    let admin_token = create_session(&state, &admin.username).await.unwrap();
    let staff_token = create_session(&state, &staff.username).await.unwrap();
    let blocked_token = create_session(&state, &blocked.username).await.unwrap();
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
    let admin_token = create_session(&state, &admin.username).await.unwrap();
    let staff_token = create_session(&state, &staff.username).await.unwrap();
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
    let admin_token = create_session(&state, &admin.username).await.unwrap();
    let staff_token = create_session(&state, &staff.username).await.unwrap();
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


