use std::{collections::HashMap, sync::Arc};

use askama::Template;
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use bson::{DateTime, oid::ObjectId};
use serde::Deserialize;

use crate::{
    session::SessionUser,
    state::{
        AppState, advance_project_concept_status, create_concept_status, create_project_concept,
        create_resource_usage, delete_concept_status, delete_project_concept,
        delete_resource_usage, get_concept_status_by_id_for_company, get_initial_concept_status,
        get_project_by_id_for_company, get_project_concept_by_id_for_company,
        get_resource_usage_by_id_for_company, list_active_project_concepts,
        list_active_project_concepts_for_status, list_concept_statuses, list_project_concepts,
        list_projects, list_resource_usage_allocations, list_resource_usages,
        list_resource_usages_for_slot, list_resources, project_status_summary_by_quantity,
        replace_hourly_resource_usage_grid, replace_resource_usage_allocations,
        replace_resource_usage_allocations_equal, update_concept_status, update_project_concept,
        update_resource_usage,
    },
};

use super::finance::helpers::{SimpleOption, require_admin_active};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn parse_oid(id: &str) -> Result<ObjectId, Response> {
    ObjectId::parse_str(id).map_err(|_| json_error(StatusCode::BAD_REQUEST, "invalid id"))
}

#[derive(Debug, Deserialize)]
pub struct ConceptStatusPayload {
    pub name: String,
    pub position: i32,
    pub color: Option<String>,
    #[serde(default)]
    pub is_initial: bool,
    #[serde(default)]
    pub is_terminal: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

pub async fn api_concept_statuses_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    match list_concept_statuses(&state, &company_id).await {
        Ok(items) => Json(items).into_response(),
        Err(_) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "could not list statuses"),
    }
}

pub async fn api_concept_statuses_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConceptStatusPayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    match create_concept_status(
        &state,
        &company_id,
        &payload.name,
        payload.position,
        payload.color,
        payload.is_initial,
        payload.is_terminal,
        payload.is_active,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_concept_statuses_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ConceptStatusPayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match update_concept_status(
        &state,
        &id,
        &company_id,
        &payload.name,
        payload.position,
        payload.color,
        payload.is_initial,
        payload.is_terminal,
        payload.is_active,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_concept_statuses_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match delete_concept_status(&state, &id, &company_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProjectConceptPayload {
    pub status_id: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub estimated_hours: Option<f64>,
    pub estimated_cost: Option<f64>,
    pub notes: Option<String>,
    #[serde(default)]
    pub position: i32,
}

pub async fn api_project_concepts_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let project_id = match parse_oid(&project_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match list_project_concepts(&state, &company_id, &project_id).await {
        Ok(items) => Json(items).into_response(),
        Err(_) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "could not list concepts"),
    }
}

pub async fn api_project_concepts_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Json(payload): Json<ProjectConceptPayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let project_id = match parse_oid(&project_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let status_id = match payload.status_id.as_deref() {
        Some("") | None => None,
        Some(id) => match parse_oid(id) {
            Ok(id) => Some(id),
            Err(resp) => return resp,
        },
    };
    match create_project_concept(
        &state,
        &company_id,
        &project_id,
        status_id,
        &payload.name,
        payload.quantity,
        payload.unit,
        payload.description,
        payload.estimated_hours,
        payload.estimated_cost,
        payload.notes,
        payload.position,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_project_concepts_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ProjectConceptPayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let status_id = match payload.status_id.as_deref() {
        Some(status_id) => match parse_oid(status_id) {
            Ok(id) => id,
            Err(resp) => return resp,
        },
        None => match get_initial_concept_status(&state, &company_id).await {
            Ok(Some(status)) => match status.id {
                Some(id) => id,
                None => return json_error(StatusCode::BAD_REQUEST, "initial status missing id"),
            },
            _ => return json_error(StatusCode::BAD_REQUEST, "initial status not configured"),
        },
    };
    match update_project_concept(
        &state,
        &id,
        &company_id,
        &status_id,
        &payload.name,
        payload.quantity,
        payload.unit,
        payload.description,
        payload.estimated_hours,
        payload.estimated_cost,
        payload.notes,
        payload.position,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_project_concepts_advance(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match advance_project_concept_status(&state, &id, &company_id).await {
        Ok(status) => Json(status).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_project_concepts_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match delete_project_concept(&state, &id, &company_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_project_status_summary(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let project_id = match parse_oid(&project_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match project_status_summary_by_quantity(&state, &company_id, &project_id).await {
        Ok(summary) => Json(summary).into_response(),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not calculate summary",
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct ResourceUsagePayload {
    pub resource_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
}

fn parse_datetime(value: &str) -> Result<DateTime, Response> {
    DateTime::parse_rfc3339_str(value)
        .map_err(|_| json_error(StatusCode::BAD_REQUEST, "invalid datetime"))
}

pub async fn api_resource_usages_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    match list_resource_usages(&state, &company_id).await {
        Ok(items) => Json(items).into_response(),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not list resource usages",
        ),
    }
}

pub async fn api_resource_usages_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResourceUsagePayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let resource_id = match parse_oid(&payload.resource_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let started_at = match parse_datetime(&payload.started_at) {
        Ok(dt) => dt,
        Err(resp) => return resp,
    };
    let ended_at = match payload.ended_at.as_deref() {
        Some(value) if !value.is_empty() => match parse_datetime(value) {
            Ok(dt) => Some(dt),
            Err(resp) => return resp,
        },
        _ => None,
    };
    match create_resource_usage(
        &state,
        &company_id,
        &resource_id,
        started_at,
        ended_at,
        payload.operator_name,
        payload.notes,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct ResourceUsageUpdatePayload {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub hourly_cost_snapshot: f64,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
}

pub async fn api_resource_usages_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ResourceUsageUpdatePayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let started_at = match parse_datetime(&payload.started_at) {
        Ok(dt) => dt,
        Err(resp) => return resp,
    };
    let ended_at = match payload.ended_at.as_deref() {
        Some(value) if !value.is_empty() => match parse_datetime(value) {
            Ok(dt) => Some(dt),
            Err(resp) => return resp,
        },
        _ => None,
    };
    match update_resource_usage(
        &state,
        &id,
        &company_id,
        started_at,
        ended_at,
        payload.hourly_cost_snapshot,
        payload.operator_name,
        payload.notes,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct AllocationPayload {
    pub concept_id: String,
    pub ratio: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationsPayload {
    pub allocations: Option<Vec<AllocationPayload>>,
    pub concept_ids: Option<Vec<String>>,
}

pub async fn api_resource_usage_allocations_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match list_resource_usage_allocations(&state, &company_id, &id).await {
        Ok(items) => Json(items).into_response(),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not list allocations",
        ),
    }
}

pub async fn api_resource_usage_allocations_replace(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<AllocationsPayload>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let usage_id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let result = if let Some(allocations) = payload.allocations {
        let mut parsed = Vec::new();
        for allocation in allocations {
            let concept_id = match parse_oid(&allocation.concept_id) {
                Ok(id) => id,
                Err(resp) => return resp,
            };
            parsed.push((concept_id, allocation.ratio, allocation.notes));
        }
        replace_resource_usage_allocations(&state, &company_id, &usage_id, &parsed).await
    } else if let Some(concept_ids) = payload.concept_ids {
        let mut parsed = Vec::new();
        for concept_id in concept_ids {
            parsed.push(match parse_oid(&concept_id) {
                Ok(id) => id,
                Err(resp) => return resp,
            });
        }
        replace_resource_usage_allocations_equal(&state, &company_id, &usage_id, &parsed).await
    } else {
        return json_error(
            StatusCode::BAD_REQUEST,
            "allocations or concept_ids are required",
        );
    };

    match result {
        Ok(ids) => Json(serde_json::json!({
            "ids": ids.into_iter().map(|id| id.to_hex()).collect::<Vec<_>>()
        }))
        .into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

pub async fn api_resource_usages_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let id = match parse_oid(&id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match delete_resource_usage(&state, &id, &company_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

#[derive(Template)]
#[template(path = "admin/concept_statuses/index.html")]
struct ConceptStatusesTemplate {
    statuses: Vec<ConceptStatusRow>,
}

struct ConceptStatusRow {
    id: String,
    name: String,
    position: i32,
    color: String,
    is_initial: bool,
    is_terminal: bool,
    is_active: bool,
}

#[derive(Template)]
#[template(path = "admin/concept_statuses/form.html")]
struct ConceptStatusFormTemplate {
    action: String,
    name: String,
    position: String,
    color: String,
    is_initial: bool,
    is_terminal: bool,
    is_active: bool,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConceptStatusForm {
    pub name: String,
    pub position: String,
    pub color: String,
    pub is_initial: Option<String>,
    pub is_terminal: Option<String>,
    pub is_active: Option<String>,
}

pub async fn concept_statuses_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let statuses = list_concept_statuses(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|s| ConceptStatusRow {
            id: s.id.map(|id| id.to_hex()).unwrap_or_default(),
            name: s.name,
            position: s.position,
            color: s.color.unwrap_or_default(),
            is_initial: s.is_initial,
            is_terminal: s.is_terminal,
            is_active: s.is_active,
        })
        .collect();
    render(ConceptStatusesTemplate { statuses })
}

pub async fn concept_statuses_new(session_user: SessionUser) -> Result<Html<String>, StatusCode> {
    require_admin_active(&session_user)?;
    render(ConceptStatusFormTemplate {
        action: "/admin/concept_statuses".into(),
        name: String::new(),
        position: "10".into(),
        color: "sky".into(),
        is_initial: false,
        is_terminal: false,
        is_active: true,
        is_edit: false,
        errors: None,
    })
}

pub async fn concept_statuses_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ConceptStatusForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let position = form.position.parse().unwrap_or(10);
    let result = create_concept_status(
        &state,
        &company_id,
        &form.name,
        position,
        clean_optional(form.color.clone()),
        form.is_initial.is_some(),
        form.is_terminal.is_some(),
        form.is_active.is_some(),
    )
    .await;
    match result {
        Ok(_) => Redirect::to("/admin/concept_statuses").into_response(),
        Err(err) => render(ConceptStatusFormTemplate {
            action: "/admin/concept_statuses".into(),
            name: form.name,
            position: form.position,
            color: form.color,
            is_initial: form.is_initial.is_some(),
            is_terminal: form.is_terminal.is_some(),
            is_active: form.is_active.is_some(),
            is_edit: false,
            errors: Some(err.to_string()),
        })
        .into_response(),
    }
}

pub async fn concept_statuses_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let status = get_concept_status_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    render(ConceptStatusFormTemplate {
        action: format!("/admin/concept_statuses/{id}/update"),
        name: status.name,
        position: status.position.to_string(),
        color: status.color.unwrap_or_default(),
        is_initial: status.is_initial,
        is_terminal: status.is_terminal,
        is_active: status.is_active,
        is_edit: true,
        errors: None,
    })
}

pub async fn concept_statuses_update_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ConceptStatusForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let result = update_concept_status(
        &state,
        &oid,
        &company_id,
        &form.name,
        form.position.parse().unwrap_or(10),
        clean_optional(form.color),
        form.is_initial.is_some(),
        form.is_terminal.is_some(),
        form.is_active.is_some(),
    )
    .await;
    match result {
        Ok(_) => Redirect::to("/admin/concept_statuses").into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

pub async fn concept_statuses_delete_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match delete_concept_status(&state, &oid, &company_id).await {
        Ok(_) => Redirect::to("/admin/concept_statuses").into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

#[derive(Template)]
#[template(path = "admin/projects/detail.html")]
struct ProjectDetailTemplate {
    project_id: String,
    title: String,
    description: String,
    summary: Vec<ProjectSummaryRow>,
    concepts: Vec<ProjectConceptRow>,
}

struct ProjectSummaryRow {
    name: String,
    quantity: String,
    unit: String,
}

struct ProjectConceptRow {
    id: String,
    name: String,
    quantity: String,
    unit: String,
    status: String,
    estimated_hours: String,
    estimated_cost: String,
    position: i32,
}

#[derive(Template)]
#[template(path = "admin/project_concepts/form.html")]
struct ProjectConceptFormTemplate {
    action: String,
    project_id: String,
    name: String,
    quantity: String,
    unit: String,
    status_id: String,
    description: String,
    estimated_hours: String,
    estimated_cost: String,
    notes: String,
    position: String,
    statuses: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConceptFormData {
    pub name: String,
    pub quantity: String,
    pub unit: String,
    pub status_id: String,
    pub description: String,
    pub estimated_hours: String,
    pub estimated_cost: String,
    pub notes: String,
    pub position: String,
}

pub async fn project_detail(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let project_id = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project = get_project_by_id_for_company(&state, &project_id, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let statuses = list_concept_statuses(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let concepts = list_project_concepts(&state, &company_id, &project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let summary = project_status_summary_by_quantity(&state, &company_id, &project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|item| ProjectSummaryRow {
            name: item.name,
            quantity: format_quantity(item.quantity),
            unit: item.unit.unwrap_or_else(|| "pzas".into()),
        })
        .collect();
    let rows = concepts
        .into_iter()
        .map(|c| {
            let status_name = statuses
                .iter()
                .find(|s| s.id.as_ref() == Some(&c.status_id))
                .map(|s| s.name.clone())
                .unwrap_or_default();
            ProjectConceptRow {
                id: c.id.map(|id| id.to_hex()).unwrap_or_default(),
                name: c.name,
                quantity: format_quantity(c.quantity),
                unit: c.unit.unwrap_or_default(),
                status: status_name,
                estimated_hours: c
                    .estimated_hours
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_default(),
                estimated_cost: c
                    .estimated_cost
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_default(),
                position: c.position,
            }
        })
        .collect();
    render(ProjectDetailTemplate {
        project_id: id,
        title: project.title,
        description: project.description.unwrap_or_default(),
        summary,
        concepts: rows,
    })
}

pub async fn project_concepts_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let statuses = status_options(&state, &company_id, None).await?;
    render(ProjectConceptFormTemplate {
        action: format!("/admin/projects/{project_id}/concepts"),
        project_id,
        name: String::new(),
        quantity: "1".into(),
        unit: "piezas".into(),
        status_id: String::new(),
        description: String::new(),
        estimated_hours: String::new(),
        estimated_cost: String::new(),
        notes: String::new(),
        position: "1".into(),
        statuses,
        is_edit: false,
        errors: None,
    })
}

pub async fn project_concepts_create_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Form(form): Form<ProjectConceptFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let project_oid = match ObjectId::parse_str(&project_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let status_id = ObjectId::parse_str(&form.status_id).ok();
    let result = create_project_concept(
        &state,
        &company_id,
        &project_oid,
        status_id,
        &form.name,
        form.quantity.parse().unwrap_or(1.0),
        clean_optional(form.unit),
        clean_optional(form.description),
        form.estimated_hours.parse().ok(),
        form.estimated_cost.parse().ok(),
        clean_optional(form.notes),
        form.position.parse().unwrap_or(0),
    )
    .await;
    match result {
        Ok(_) => Redirect::to(&format!("/admin/projects/{project_id}")).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

pub async fn project_concepts_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let concept = get_project_concept_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let statuses = status_options(&state, &company_id, Some(&concept.status_id)).await?;
    render(ProjectConceptFormTemplate {
        action: format!("/admin/project_concepts/{id}/update"),
        project_id: concept.project_id.to_hex(),
        name: concept.name,
        quantity: format_quantity(concept.quantity),
        unit: concept.unit.unwrap_or_default(),
        status_id: concept.status_id.to_hex(),
        description: concept.description.unwrap_or_default(),
        estimated_hours: concept
            .estimated_hours
            .map(|v| format!("{v:.2}"))
            .unwrap_or_default(),
        estimated_cost: concept
            .estimated_cost
            .map(|v| format!("{v:.2}"))
            .unwrap_or_default(),
        notes: concept.notes.unwrap_or_default(),
        position: concept.position.to_string(),
        statuses,
        is_edit: true,
        errors: None,
    })
}

pub async fn project_concepts_update_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ProjectConceptFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let status_id = match ObjectId::parse_str(&form.status_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let concept = match get_project_concept_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match update_project_concept(
        &state,
        &oid,
        &company_id,
        &status_id,
        &form.name,
        form.quantity.parse().unwrap_or(1.0),
        clean_optional(form.unit),
        clean_optional(form.description),
        form.estimated_hours.parse().ok(),
        form.estimated_cost.parse().ok(),
        clean_optional(form.notes),
        form.position.parse().unwrap_or(0),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/admin/projects/{}", concept.project_id.to_hex()))
            .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

pub async fn project_concepts_advance_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let concept = match get_project_concept_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match advance_project_concept_status(&state, &oid, &company_id).await {
        Ok(_) => Redirect::to(&format!("/admin/projects/{}", concept.project_id.to_hex()))
            .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

pub async fn project_concepts_delete_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let concept = match get_project_concept_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match delete_project_concept(&state, &oid, &company_id).await {
        Ok(_) => Redirect::to(&format!("/admin/projects/{}", concept.project_id.to_hex()))
            .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

#[derive(Template)]
#[template(path = "admin/resource_usages/index.html")]
struct ResourceUsagesTemplate {
    date: String,
    statuses: Vec<SimpleOption>,
    selected_status_id: String,
    selected_status_name: String,
    hours: Vec<HourHeader>,
    rows: Vec<ResourceUsageGridRow>,
    resources_empty: bool,
}

struct HourHeader {
    hour: i32,
    is_work_hour: bool,
}

struct ResourceUsageGridRow {
    concept_id: String,
    status_name: String,
    project_title: String,
    concept_name: String,
    quantity: String,
    unit: String,
    cells: Vec<ResourceUsageGridCell>,
}

struct ResourceUsageGridCell {
    hour: i32,
    is_work_hour: bool,
    resources: Vec<ResourceUsageGridResource>,
    labels: String,
}

struct ResourceUsageGridResource {
    field_name: String,
    resource_id: String,
    label: String,
    selected: bool,
}

#[derive(Template)]
#[template(path = "admin/resource_usages/form.html")]
struct ResourceUsageFormTemplate {
    action: String,
    resources: Vec<SimpleOption>,
    concepts: Vec<SimpleOption>,
    resource_id: String,
    started_at: String,
    ended_at: String,
    operator_name: String,
    notes: String,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceUsageFormData {
    pub resource_id: String,
    pub started_at: String,
    pub ended_at: String,
    pub concept_ids: Option<Vec<String>>,
    pub operator_name: String,
    pub notes: String,
}

pub async fn resource_usages_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let statuses_raw = list_concept_statuses(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let selected_status_id = params.get("status_id").and_then(|id| {
        if id == "all" {
            None
        } else {
            ObjectId::parse_str(id).ok()
        }
    });
    let selected_status_name = selected_status_id
        .as_ref()
        .and_then(|selected_status_id| {
            statuses_raw
                .iter()
                .find(|status| status.id.as_ref() == Some(selected_status_id))
        })
        .map(|status| status.name.clone())
        .unwrap_or_else(|| "Todos".to_string());

    let date = params
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let date_start = parse_html_date(&date).ok_or(StatusCode::BAD_REQUEST)?;
    let hours: Vec<HourHeader> = (0..24)
        .map(|hour| HourHeader {
            hour,
            is_work_hour: (7..=22).contains(&hour),
        })
        .collect();
    let resources = list_resources(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|resource| resource.is_active)
        .collect::<Vec<_>>();
    let resources_empty = resources.is_empty();

    let concepts = if let Some(selected_status_id) = &selected_status_id {
        list_active_project_concepts_for_status(&state, &company_id, selected_status_id).await
    } else {
        list_active_project_concepts(&state, &company_id).await
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let status_names = statuses_raw
        .iter()
        .filter_map(|status| Some((status.id.clone()?, status.name.clone())))
        .collect::<HashMap<_, _>>();

    let mut selected_cells: HashMap<(String, i32, String), String> = HashMap::new();
    for hour in &hours {
        let started_at =
            add_hours_for_route(date_start, hour.hour).ok_or(StatusCode::BAD_REQUEST)?;
        let ended_at =
            add_hours_for_route(date_start, hour.hour + 1).ok_or(StatusCode::BAD_REQUEST)?;
        for usage in list_resource_usages_for_slot(&state, &company_id, started_at, ended_at)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            let Some(usage_id) = usage.id.clone() else {
                continue;
            };
            let allocations = list_resource_usage_allocations(&state, &company_id, &usage_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            for allocation in allocations {
                selected_cells.insert(
                    (
                        allocation.concept_id.to_hex(),
                        hour.hour,
                        usage.resource_id.to_hex(),
                    ),
                    usage.resource_name_snapshot.clone(),
                );
            }
        }
    }

    let mut rows = Vec::new();
    for concept in concepts {
        let Some(concept_id) = concept.id.clone() else {
            continue;
        };
        let project_title = get_project_by_id_for_company(&state, &concept.project_id, &company_id)
            .await
            .ok()
            .flatten()
            .map(|project| project.title)
            .unwrap_or_default();
        let mut cells = Vec::new();
        for hour in &hours {
            let mut cell_resources = Vec::new();
            let labels = selected_cells
                .iter()
                .filter_map(|((selected_concept_id, selected_hour, _), label)| {
                    if selected_concept_id == &concept_id.to_hex() && *selected_hour == hour.hour {
                        Some(label.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for resource in &resources {
                let Some(resource_id) = resource.id.clone() else {
                    continue;
                };
                if !resource.allowed_status_ids.contains(&concept.status_id) {
                    continue;
                }
                let selected = selected_cells.contains_key(&(
                    concept_id.to_hex(),
                    hour.hour,
                    resource_id.to_hex(),
                ));
                cell_resources.push(ResourceUsageGridResource {
                    field_name: format!(
                        "cell_{}_{}_{}",
                        concept_id.to_hex(),
                        hour.hour,
                        resource_id.to_hex()
                    ),
                    resource_id: resource_id.to_hex(),
                    label: resource.name.clone(),
                    selected,
                });
            }
            cells.push(ResourceUsageGridCell {
                hour: hour.hour,
                is_work_hour: hour.is_work_hour,
                resources: cell_resources,
                labels: labels.join(", "),
            });
        }
        rows.push(ResourceUsageGridRow {
            concept_id: concept_id.to_hex(),
            status_name: status_names
                .get(&concept.status_id)
                .cloned()
                .unwrap_or_else(|| "Estado".to_string()),
            project_title,
            concept_name: concept.name,
            quantity: format_quantity(concept.quantity),
            unit: concept.unit.unwrap_or_else(|| "pzas".to_string()),
            cells,
        });
    }

    let mut statuses = vec![SimpleOption {
        selected: selected_status_id.is_none(),
        value: "all".to_string(),
        label: "Todos".to_string(),
    }];
    statuses.extend(statuses_raw.into_iter().filter_map(|status| {
        let id = status.id?;
        Some(SimpleOption {
            selected: selected_status_id.as_ref() == Some(&id),
            value: id.to_hex(),
            label: status.name,
        })
    }));

    render(ResourceUsagesTemplate {
        date,
        statuses,
        selected_status_id: selected_status_id
            .as_ref()
            .map(|id| id.to_hex())
            .unwrap_or_else(|| "all".to_string()),
        selected_status_name,
        hours,
        rows,
        resources_empty,
    })
}

pub async fn resource_usages_save_grid(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let date = form
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let status_id = form.get("status_id").and_then(|id| {
        if id == "all" {
            None
        } else {
            ObjectId::parse_str(id).ok()
        }
    });
    let date_start = match parse_html_date(&date) {
        Some(date) => date,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let mut selections = Vec::new();
    for key in form.keys() {
        let Some(rest) = key.strip_prefix("cell_") else {
            continue;
        };
        let parts: Vec<&str> = rest.split('_').collect();
        if parts.len() != 3 {
            continue;
        }
        let Ok(concept_id) = ObjectId::parse_str(parts[0]) else {
            continue;
        };
        let Ok(hour) = parts[1].parse::<i32>() else {
            continue;
        };
        let Ok(resource_id) = ObjectId::parse_str(parts[2]) else {
            continue;
        };
        selections.push((concept_id, hour, resource_id));
    }

    match replace_hourly_resource_usage_grid(
        &state,
        &company_id,
        status_id.as_ref(),
        date_start,
        0,
        24,
        &selections,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!(
            "/admin/resource_usages?date={}&status_id={}",
            date,
            status_id
                .as_ref()
                .map(|id| id.to_hex())
                .unwrap_or_else(|| "all".to_string())
        ))
        .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

pub async fn resource_usages_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    render(ResourceUsageFormTemplate {
        action: "/admin/resource_usages/create".into(),
        resources: resource_options(&state, &company_id, None).await?,
        concepts: concept_options(&state, &company_id, &[]).await?,
        resource_id: String::new(),
        started_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string(),
        ended_at: String::new(),
        operator_name: String::new(),
        notes: String::new(),
        is_edit: false,
        errors: None,
    })
}

pub async fn resource_usages_create_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ResourceUsageFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let resource_id = match ObjectId::parse_str(&form.resource_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let started_at = match parse_html_datetime(&form.started_at) {
        Some(dt) => dt,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let ended_at = if form.ended_at.is_empty() {
        None
    } else {
        parse_html_datetime(&form.ended_at)
    };
    let result = create_resource_usage(
        &state,
        &company_id,
        &resource_id,
        started_at,
        ended_at,
        clean_optional(form.operator_name),
        clean_optional(form.notes),
    )
    .await;
    let usage_id = match result {
        Ok(id) => id,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let concept_ids: Vec<ObjectId> = form
        .concept_ids
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| ObjectId::parse_str(id).ok())
        .collect();
    if !concept_ids.is_empty() {
        if let Err(err) =
            replace_resource_usage_allocations_equal(&state, &company_id, &usage_id, &concept_ids)
                .await
        {
            return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
        }
    }
    Redirect::to("/admin/resource_usages").into_response()
}

pub async fn resource_usages_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let usage = get_resource_usage_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let selected_allocations: Vec<ObjectId> =
        list_resource_usage_allocations(&state, &company_id, &oid)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.concept_id)
            .collect();
    render(ResourceUsageFormTemplate {
        action: format!("/admin/resource_usages/{id}/update"),
        resources: resource_options(&state, &company_id, Some(&usage.resource_id)).await?,
        concepts: concept_options(&state, &company_id, &selected_allocations).await?,
        resource_id: usage.resource_id.to_hex(),
        started_at: format_html_datetime(&usage.started_at),
        ended_at: usage
            .ended_at
            .map(|d| format_html_datetime(&d))
            .unwrap_or_default(),
        operator_name: usage.operator_name.unwrap_or_default(),
        notes: usage.notes.unwrap_or_default(),
        is_edit: true,
        errors: None,
    })
}

pub async fn resource_usages_update_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ResourceUsageFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let usage = match get_resource_usage_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let started_at = match parse_html_datetime(&form.started_at) {
        Some(dt) => dt,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let ended_at = if form.ended_at.is_empty() {
        None
    } else {
        parse_html_datetime(&form.ended_at)
    };
    if let Err(err) = update_resource_usage(
        &state,
        &oid,
        &company_id,
        started_at,
        ended_at,
        usage.hourly_cost_snapshot,
        clean_optional(form.operator_name),
        clean_optional(form.notes),
    )
    .await
    {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }
    let concept_ids: Vec<ObjectId> = form
        .concept_ids
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| ObjectId::parse_str(id).ok())
        .collect();
    if !concept_ids.is_empty() {
        if let Err(err) =
            replace_resource_usage_allocations_equal(&state, &company_id, &oid, &concept_ids).await
        {
            return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
        }
    }
    Redirect::to("/admin/resource_usages").into_response()
}

pub async fn resource_usages_delete_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match delete_resource_usage(&state, &oid, &company_id).await {
        Ok(_) => Redirect::to("/admin/resource_usages").into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn status_options(
    state: &AppState,
    company_id: &ObjectId,
    selected: Option<&ObjectId>,
) -> Result<Vec<SimpleOption>, StatusCode> {
    Ok(list_concept_statuses(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|s| {
            let value = s.id.map(|id| id.to_hex()).unwrap_or_default();
            SimpleOption {
                selected: selected.map(|id| id.to_hex()) == Some(value.clone()),
                value,
                label: s.name,
            }
        })
        .collect())
}

async fn resource_options(
    state: &AppState,
    company_id: &ObjectId,
    selected: Option<&ObjectId>,
) -> Result<Vec<SimpleOption>, StatusCode> {
    Ok(list_resources(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|r| r.is_active || selected == r.id.as_ref())
        .map(|r| {
            let value = r.id.map(|id| id.to_hex()).unwrap_or_default();
            SimpleOption {
                selected: selected.map(|id| id.to_hex()) == Some(value.clone()),
                label: format!("{} · ${:.2}/h", r.name, r.hourly_cost),
                value,
            }
        })
        .collect())
}

async fn concept_options(
    state: &AppState,
    company_id: &ObjectId,
    selected: &[ObjectId],
) -> Result<Vec<SimpleOption>, StatusCode> {
    let projects = list_projects(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for project in projects {
        let Some(project_id) = project.id else {
            continue;
        };
        let concepts = list_project_concepts(state, company_id, &project_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        for concept in concepts {
            let Some(concept_id) = concept.id else {
                continue;
            };
            out.push(SimpleOption {
                selected: selected.contains(&concept_id),
                value: concept_id.to_hex(),
                label: format!(
                    "{} · {} ({})",
                    project.title,
                    concept.name,
                    format_quantity(concept.quantity)
                ),
            });
        }
    }
    Ok(out)
}

fn clean_optional(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn format_quantity(value: f64) -> String {
    if (value.fract()).abs() < 0.0001 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn format_html_datetime(dt: &DateTime) -> String {
    let ms = dt.timestamp_millis();
    let secs = ms / 1000;
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M")
        .to_string()
}

fn parse_html_datetime(value: &str) -> Option<DateTime> {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .ok()
        .map(|dt| DateTime::from_millis(dt.and_utc().timestamp_millis()))
}

fn parse_html_date(value: &str) -> Option<DateTime> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|dt| DateTime::from_millis(dt.and_utc().timestamp_millis()))
}

fn add_hours_for_route(date_start: DateTime, hour: i32) -> Option<DateTime> {
    if !(0..=24).contains(&hour) {
        return None;
    }
    Some(DateTime::from_millis(
        date_start.timestamp_millis() + i64::from(hour) * 3_600_000,
    ))
}
