#[path = "common/mod.rs"]
mod common;

use alfredodev::{
    models::{ProjectPriority, ProjectStatus, ResourceType},
    state::{
        advance_project_phase, create_project, create_resource, create_resource_log,
        delete_project, delete_resource, delete_resource_log, end_resource_log,
        get_project_by_id_for_company, get_resource_by_id_for_company,
        get_resource_log_by_id_for_company, list_active_resources, list_companies,
        list_resource_logs_for_project, list_resources, update_project, update_resource,
        update_resource_log,
    },
};
use bson::DateTime;

#[tokio::test]
async fn project_resource_crud_and_phase_progression_work() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let project_id = create_project(
        &state,
        &company_id,
        "Project Test",
        None,
        None,
        Some("Initial description".into()),
        ProjectPriority::High,
        Some(10_000.0),
        None,
        Some("Initial notes".into()),
    )
    .await
    .unwrap();

    let project = get_project_by_id_for_company(&state, &project_id, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(project.status, ProjectStatus::Pedidos);
    assert_eq!(project.priority, ProjectPriority::High);

    update_project(
        &state,
        &project_id,
        &company_id,
        "Project Updated",
        None,
        None,
        None,
        ProjectPriority::Urgent,
        Some(12_000.0),
        None,
        None,
    )
    .await
    .unwrap();

    let updated = get_project_by_id_for_company(&state, &project_id, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "Project Updated");
    assert_eq!(updated.priority, ProjectPriority::Urgent);

    let next = advance_project_phase(&state, &project_id, &company_id)
        .await
        .unwrap();
    assert_eq!(next, Some(ProjectStatus::Analisis));

    let resource_id = create_resource(
        &state,
        &company_id,
        "Resource Test",
        ResourceType::Vehicle,
        true,
        Some("Notes".into()),
    )
    .await
    .unwrap();
    assert!(
        list_active_resources(&state, &company_id)
            .await
            .unwrap()
            .iter()
            .any(|resource| resource.id.as_ref() == Some(&resource_id))
    );

    update_resource(
        &state,
        &resource_id,
        &company_id,
        "Resource Updated",
        ResourceType::Equipment,
        false,
        None,
    )
    .await
    .unwrap();
    let resource = get_resource_by_id_for_company(&state, &resource_id, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resource.name, "Resource Updated");
    assert_eq!(resource.resource_type, ResourceType::Equipment);
    assert!(!resource.is_active);
    assert!(list_resources(&state, &company_id).await.unwrap().len() >= 1);

    delete_resource(&state, &resource_id, &company_id)
        .await
        .unwrap();
    assert!(
        get_resource_by_id_for_company(&state, &resource_id, &company_id)
            .await
            .unwrap()
            .is_none()
    );

    delete_project(&state, &project_id, &company_id)
        .await
        .unwrap();
    assert!(
        get_project_by_id_for_company(&state, &project_id, &company_id)
            .await
            .unwrap()
            .is_none()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_log_lifecycle_calculates_duration() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let project_id = create_project(
        &state,
        &company_id,
        "Log Project",
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
    let resource_id = create_resource(
        &state,
        &company_id,
        "Log Resource",
        ResourceType::Machinery,
        true,
        None,
    )
    .await
    .unwrap();
    let started_at = DateTime::parse_rfc3339_str("2026-05-04T10:00:00Z").unwrap();
    let ended_at = DateTime::parse_rfc3339_str("2026-05-04T12:30:00Z").unwrap();

    let log_id = create_resource_log(
        &state,
        &company_id,
        Some(project_id.clone()),
        Some("Produccion".into()),
        Some(resource_id.clone()),
        Some("Log Resource".into()),
        started_at,
        Some("Operator".into()),
        Some("Initial".into()),
    )
    .await
    .unwrap();

    assert_eq!(
        list_resource_logs_for_project(&state, &company_id, &project_id)
            .await
            .unwrap()
            .len(),
        1
    );

    update_resource_log(
        &state,
        &log_id,
        &company_id,
        Some(project_id.clone()),
        Some("Calidad".into()),
        Some(resource_id),
        Some("Log Resource".into()),
        started_at,
        Some(ended_at),
        Some("Operator 2".into()),
        Some("Updated".into()),
    )
    .await
    .unwrap();

    let updated = get_resource_log_by_id_for_company(&state, &log_id, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.phase.as_deref(), Some("Calidad"));
    assert_eq!(updated.duration_hours, Some(2.5));

    let duration = end_resource_log(
        &state,
        &log_id,
        &company_id,
        DateTime::parse_rfc3339_str("2026-05-04T13:00:00Z").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(duration, Some(3.0));

    delete_resource_log(&state, &log_id, &company_id)
        .await
        .unwrap();
    assert!(
        get_resource_log_by_id_for_company(&state, &log_id, &company_id)
            .await
            .unwrap()
            .is_none()
    );

    common::teardown(Some(ctx)).await;
}
