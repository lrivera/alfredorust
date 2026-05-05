#[path = "common/mod.rs"]
mod common;

use alfredodev::{
    models::{ProjectPriority, ProjectStatus, ResourceType},
    state::{
        advance_project_concept_status, advance_project_phase, create_concept_status,
        create_project, create_project_concept, create_resource, create_resource_log,
        create_resource_usage, create_resource_with_cost, delete_project, delete_resource,
        delete_resource_log, delete_resource_usage, end_resource_log, get_initial_concept_status,
        get_project_by_id_for_company, get_project_concept_by_id_for_company,
        get_resource_by_id_for_company, get_resource_log_by_id_for_company,
        get_resource_usage_by_id_for_company, list_active_resources, list_companies,
        list_concept_statuses, list_project_concepts, list_resource_logs_for_project,
        list_resource_usage_allocations, list_resource_usage_allocations_for_concept,
        list_resources, project_status_summary_by_quantity, replace_hourly_resource_usage_grid,
        replace_resource_usage_allocations, replace_resource_usage_allocations_equal,
        update_project, update_resource, update_resource_allowed_statuses, update_resource_log,
        update_resource_usage,
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
async fn project_concepts_statuses_and_quantity_summary_work() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();

    let initial = get_initial_concept_status(&state, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert!(initial.is_initial);

    let cnc_status_id = create_concept_status(
        &state,
        &company_id,
        "CNC Especial",
        50,
        Some("amber".into()),
        false,
        false,
        true,
    )
    .await
    .unwrap();

    let project_id = create_project(
        &state,
        &company_id,
        "Engranes",
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
        &company_id,
        &project_id,
        Some(initial.id.clone().unwrap()),
        "Engrane A",
        5.0,
        Some("piezas".into()),
        None,
        Some(3.0),
        Some(1000.0),
        None,
        1,
    )
    .await
    .unwrap();
    let concept_b = create_project_concept(
        &state,
        &company_id,
        &project_id,
        Some(cnc_status_id.clone()),
        "Engrane B",
        1.0,
        Some("pieza".into()),
        None,
        None,
        None,
        None,
        2,
    )
    .await
    .unwrap();

    assert_eq!(
        list_project_concepts(&state, &company_id, &project_id)
            .await
            .unwrap()
            .len(),
        2
    );

    let next = advance_project_concept_status(&state, &concept_a, &company_id)
        .await
        .unwrap()
        .unwrap();
    let updated_a = get_project_concept_by_id_for_company(&state, &concept_a, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_a.status_id, next.id.unwrap());

    let summary = project_status_summary_by_quantity(&state, &company_id, &project_id)
        .await
        .unwrap();
    assert!(summary.iter().any(|item| item.quantity == 5.0));
    assert!(summary.iter().any(|item| item.quantity == 1.0));
    assert_eq!(
        list_concept_statuses(&state, &company_id)
            .await
            .unwrap()
            .first()
            .unwrap()
            .position,
        1
    );

    assert!(
        get_project_concept_by_id_for_company(&state, &concept_b, &company_id)
            .await
            .unwrap()
            .is_some()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn resource_usage_allocations_split_costs_across_concepts() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();
    let status_id = get_initial_concept_status(&state, &company_id)
        .await
        .unwrap()
        .unwrap()
        .id
        .unwrap();
    let project_id = create_project(
        &state,
        &company_id,
        "Uso Compartido",
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
        &company_id,
        &project_id,
        Some(status_id.clone()),
        "Material A",
        5.0,
        Some("piezas".into()),
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
        &company_id,
        &project_id,
        Some(status_id),
        "Material B",
        1.0,
        Some("pieza".into()),
        None,
        None,
        None,
        None,
        2,
    )
    .await
    .unwrap();
    let resource_id = create_resource_with_cost(
        &state,
        &company_id,
        "Camión",
        ResourceType::Vehicle,
        true,
        500.0,
        "MXN",
        None,
    )
    .await
    .unwrap();
    update_resource_allowed_statuses(&state, &resource_id, &company_id, vec![status_id.clone()])
        .await
        .unwrap();

    let usage_id = create_resource_usage(
        &state,
        &company_id,
        &resource_id,
        DateTime::parse_rfc3339_str("2026-05-04T09:00:00Z").unwrap(),
        Some(DateTime::parse_rfc3339_str("2026-05-04T11:00:00Z").unwrap()),
        Some("Chofer".into()),
        None,
    )
    .await
    .unwrap();
    let usage = get_resource_usage_by_id_for_company(&state, &usage_id, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(usage.duration_hours, Some(2.0));
    assert_eq!(usage.total_cost, Some(1000.0));

    let allocation_ids = replace_resource_usage_allocations_equal(
        &state,
        &company_id,
        &usage_id,
        &[concept_a.clone(), concept_b.clone()],
    )
    .await
    .unwrap();
    assert_eq!(allocation_ids.len(), 2);
    let allocations = list_resource_usage_allocations(&state, &company_id, &usage_id)
        .await
        .unwrap();
    assert_eq!(allocations.len(), 2);
    assert!(allocations.iter().all(|a| a.allocated_hours == Some(1.0)));
    assert!(allocations.iter().all(|a| a.allocated_cost == Some(500.0)));

    replace_resource_usage_allocations(
        &state,
        &company_id,
        &usage_id,
        &[
            (concept_a.clone(), 0.75, None),
            (concept_b.clone(), 0.25, None),
        ],
    )
    .await
    .unwrap();
    let allocations = list_resource_usage_allocations_for_concept(&state, &company_id, &concept_a)
        .await
        .unwrap();
    assert_eq!(allocations.len(), 1);
    assert_eq!(allocations[0].allocated_cost, Some(750.0));

    update_resource_usage(
        &state,
        &usage_id,
        &company_id,
        DateTime::parse_rfc3339_str("2026-05-04T09:00:00Z").unwrap(),
        Some(DateTime::parse_rfc3339_str("2026-05-04T13:00:00Z").unwrap()),
        500.0,
        None,
        None,
    )
    .await
    .unwrap();
    let allocations = list_resource_usage_allocations_for_concept(&state, &company_id, &concept_a)
        .await
        .unwrap();
    assert_eq!(allocations[0].allocated_hours, Some(3.0));
    assert_eq!(allocations[0].allocated_cost, Some(1500.0));

    delete_resource_usage(&state, &usage_id, &company_id)
        .await
        .unwrap();
    assert!(
        list_resource_usage_allocations(&state, &company_id, &usage_id)
            .await
            .unwrap()
            .is_empty()
    );

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn hourly_resource_grid_replaces_state_concept_allocations() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();
    let company_id = list_companies(&state).await.unwrap()[0].id.clone().unwrap();
    let status_id = get_initial_concept_status(&state, &company_id)
        .await
        .unwrap()
        .unwrap()
        .id
        .unwrap();
    let project_id = create_project(
        &state,
        &company_id,
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
    let concept_a = create_project_concept(
        &state,
        &company_id,
        &project_id,
        Some(status_id.clone()),
        "Concepto A",
        2.0,
        Some("pzas".into()),
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
        &company_id,
        &project_id,
        Some(status_id.clone()),
        "Concepto B",
        3.0,
        Some("pzas".into()),
        None,
        None,
        None,
        None,
        2,
    )
    .await
    .unwrap();
    let resource_id = create_resource_with_cost(
        &state,
        &company_id,
        "CNC Grid",
        ResourceType::Machinery,
        true,
        100.0,
        "MXN",
        None,
    )
    .await
    .unwrap();
    let date = DateTime::parse_rfc3339_str("2026-05-04T00:00:00Z").unwrap();

    replace_hourly_resource_usage_grid(
        &state,
        &company_id,
        &status_id,
        date,
        7,
        10,
        &[
            (concept_a.clone(), 8, resource_id.clone()),
            (concept_b.clone(), 8, resource_id.clone()),
        ],
    )
    .await
    .unwrap();

    let allocations = list_resource_usage_allocations_for_concept(&state, &company_id, &concept_a)
        .await
        .unwrap();
    assert_eq!(allocations.len(), 1);
    assert_eq!(allocations[0].allocated_hours, Some(0.5));
    assert_eq!(allocations[0].allocated_cost, Some(50.0));

    replace_hourly_resource_usage_grid(
        &state,
        &company_id,
        &status_id,
        date,
        7,
        10,
        &[(concept_b.clone(), 8, resource_id.clone())],
    )
    .await
    .unwrap();

    assert!(
        list_resource_usage_allocations_for_concept(&state, &company_id, &concept_a)
            .await
            .unwrap()
            .is_empty()
    );
    let allocations = list_resource_usage_allocations_for_concept(&state, &company_id, &concept_b)
        .await
        .unwrap();
    assert_eq!(allocations[0].allocated_hours, Some(1.0));
    assert_eq!(allocations[0].allocated_cost, Some(100.0));

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
