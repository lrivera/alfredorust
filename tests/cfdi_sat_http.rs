#[path = "common/mod.rs"]
mod common;

use common::harness::*;

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
    let token = create_session(&state, &user.username).await.unwrap();
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
    let staff_token = create_session(&state, &staff.username).await.unwrap();
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
    let admin_token = create_session(&state, &admin.username).await.unwrap();
    let staff_token = create_session(&state, &staff.username).await.unwrap();
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
    let admin_token = create_session(&state, &admin.username).await.unwrap();
    let staff_token = create_session(&state, &staff.username).await.unwrap();
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

