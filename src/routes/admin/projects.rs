use std::sync::Arc;

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{Project, ProjectPriority, UserPermission},
    session::SessionUser,
    state::{
        AppState, advance_project_phase, create_project, delete_project,
        get_project_by_id_for_company, list_projects, update_project,
    },
};

use super::finance::helpers::{
    SimpleOption, ensure_same_company, require_active_company, require_admin_active,
};
use super::finance::{category_options, contact_options};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/projects/index.html")]
struct ProjectsIndexTemplate {
    projects: Vec<ProjectRow>,
    can_edit: bool,
    can_view_money: bool,
}

struct ProjectRow {
    id: String,
    title: String,
    description: String,
    contact_name: String,
    category_name: String,
    status: String,
    status_label: String,
    priority: String,
    priority_label: String,
    total_budget: String,
    scheduled_at: String,
    completed_at: String,
}

#[derive(Serialize)]
pub struct ProjectData {
    pub id: String,
    pub company_id: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub status_label: String,
    pub priority: String,
    pub priority_label: String,
    pub total_budget: Option<f64>,
    pub scheduled_at: Option<String>,
    pub completed_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ProjectPayload {
    pub title: String,
    pub contact_id: Option<String>,
    pub category_id: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub total_budget: Option<f64>,
    pub scheduled_at: Option<String>,
    pub notes: Option<String>,
}

struct ParsedProjectPayload {
    title: String,
    contact_id: Option<ObjectId>,
    category_id: Option<ObjectId>,
    description: Option<String>,
    priority: ProjectPriority,
    total_budget: Option<f64>,
    scheduled_at: Option<bson::DateTime>,
    notes: Option<String>,
}

fn default_priority() -> String {
    "medium".to_string()
}

pub async fn projects_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.has_permission(UserPermission::ViewProjects) {
        return Err(StatusCode::FORBIDDEN);
    }
    let can_edit = session_user.is_admin();
    let can_view_money = session_user.has_permission(UserPermission::ViewProjectMoney);
    let company_id = require_active_company(&session_user);

    let projects = list_projects(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut rows = Vec::new();
    for p in projects {
        let contact_name = if let Some(ref cid) = p.contact_id {
            crate::state::get_contact_by_id(&state, cid)
                .await
                .ok()
                .flatten()
                .map(|c| c.name)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let category_name = if let Some(ref catid) = p.category_id {
            crate::state::get_category_by_id(&state, catid)
                .await
                .ok()
                .flatten()
                .map(|c| c.name)
                .unwrap_or_default()
        } else {
            String::new()
        };

        rows.push(ProjectRow {
            id: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            title: p.title,
            description: p.description.unwrap_or_default(),
            contact_name,
            category_name,
            status: p.status.as_str().to_string(),
            status_label: p.status.label().to_string(),
            priority: p.priority.as_str().to_string(),
            priority_label: p.priority.label().to_string(),
            total_budget: p
                .total_budget
                .map(|b| format!("{:.2}", b))
                .unwrap_or_default(),
            scheduled_at: p
                .scheduled_at
                .map(|d| {
                    let ms = d.timestamp_millis();
                    let secs = ms / 1000;
                    let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
                    dt.format("%d/%m/%Y").to_string()
                })
                .unwrap_or_default(),
            completed_at: p
                .completed_at
                .map(|d| {
                    let ms = d.timestamp_millis();
                    let secs = ms / 1000;
                    let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
                    dt.format("%d/%m/%Y").to_string()
                })
                .unwrap_or_default(),
        });
    }

    render(ProjectsIndexTemplate {
        projects: rows,
        can_edit,
        can_view_money,
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/projects",
    tag = "operations",
    responses(
        (status = 200, description = "List of projects"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn projects_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProjectData>>, StatusCode> {
    if !session_user.has_permission(UserPermission::ViewProjects) {
        return Err(StatusCode::FORBIDDEN);
    }
    let company_id = require_active_company(&session_user);
    let can_view_money = session_user.has_permission(UserPermission::ViewProjectMoney);
    let projects = list_projects(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        projects
            .into_iter()
            .filter_map(|project| project_data(project, can_view_money))
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/projects/{id}",
    tag = "operations",
    params(("id" = String, Path, description = "Project id")),
    responses(
        (status = 200, description = "Project detail"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn project_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ProjectData>, StatusCode> {
    if !session_user.has_permission(UserPermission::ViewProjects) {
        return Err(StatusCode::FORBIDDEN);
    }
    let company_id = require_active_company(&session_user);
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project = get_project_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let can_view_money = session_user.has_permission(UserPermission::ViewProjectMoney);

    project_data(project, can_view_money)
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/projects/form.html")]
struct ProjectFormTemplate {
    action: String,
    id: String,
    title: String,
    description: String,
    contact_id: String,
    category_id: String,
    priority: String,
    total_budget: String,
    scheduled_at: String,
    notes: String,
    contacts: Vec<SimpleOption>,
    categories: Vec<SimpleOption>,
    priority_options: Vec<(String, String)>,
    is_edit: bool,
    errors: Option<String>,
}

fn priority_options() -> Vec<(String, String)> {
    vec![
        ("low".into(), "Baja".into()),
        ("medium".into(), "Media".into()),
        ("high".into(), "Alta".into()),
        ("urgent".into(), "Urgente".into()),
    ]
}

fn parse_priority(s: &str) -> ProjectPriority {
    match s {
        "low" => ProjectPriority::Low,
        "high" => ProjectPriority::High,
        "urgent" => ProjectPriority::Urgent,
        _ => ProjectPriority::Medium,
    }
}

fn parse_priority_strict(s: &str) -> Result<ProjectPriority, StatusCode> {
    match s {
        "low" => Ok(ProjectPriority::Low),
        "medium" => Ok(ProjectPriority::Medium),
        "high" => Ok(ProjectPriority::High),
        "urgent" => Ok(ProjectPriority::Urgent),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProjectForm {
    pub title: String,
    pub contact_id: String,
    pub category_id: String,
    pub description: String,
    pub priority: String,
    pub total_budget: String,
    pub scheduled_at: String,
    pub notes: String,
}

pub async fn projects_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let contacts: Vec<SimpleOption> = contact_options(&state, None, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let categories: Vec<SimpleOption> = category_options(&state, None, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    render(ProjectFormTemplate {
        action: "/admin/projects".into(),
        id: String::new(),
        title: String::new(),
        description: String::new(),
        contact_id: String::new(),
        category_id: String::new(),
        priority: "medium".into(),
        total_budget: String::new(),
        scheduled_at: String::new(),
        notes: String::new(),
        contacts,
        categories,
        priority_options: priority_options(),
        is_edit: false,
        errors: None,
    })
}

pub async fn projects_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ProjectForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };

    let title = form.title.trim();
    if title.is_empty() {
        let contacts = contact_options(&state, None, &company_id)
            .await
            .unwrap_or_default();
        let categories = category_options(&state, None, &company_id)
            .await
            .unwrap_or_default();
        return render(ProjectFormTemplate {
            action: "/admin/projects".into(),
            id: String::new(),
            title: form.title,
            description: form.description,
            contact_id: form.contact_id,
            category_id: form.category_id,
            priority: form.priority,
            total_budget: form.total_budget,
            scheduled_at: form.scheduled_at,
            notes: form.notes,
            contacts,
            categories,
            priority_options: priority_options(),
            is_edit: false,
            errors: Some("El título es requerido".into()),
        })
        .into_response();
    }

    let contact_id = if form.contact_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.contact_id).ok()
    };
    let category_id = if form.category_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.category_id).ok()
    };
    let total_budget = form.total_budget.parse().ok();
    let scheduled_at = parse_datetime(&form.scheduled_at);
    let priority = parse_priority(&form.priority);
    let description = if form.description.is_empty() {
        None
    } else {
        Some(form.description)
    };
    let notes = if form.notes.is_empty() {
        None
    } else {
        Some(form.notes)
    };

    match create_project(
        &state,
        &company_id,
        title,
        contact_id,
        category_id,
        description,
        priority,
        total_budget,
        scheduled_at,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/projects").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/projects",
    tag = "operations",
    request_body = ProjectPayload,
    responses(
        (status = 201, description = "Project created"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn projects_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProjectPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_project_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    match create_project(
        &state,
        &company_id,
        &parsed.title,
        parsed.contact_id,
        parsed.category_id,
        parsed.description,
        parsed.priority,
        parsed.total_budget,
        parsed.scheduled_at,
        parsed.notes,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn projects_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let project = get_project_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    ensure_same_company(&project.company_id, &company_id)?;

    let contacts: Vec<SimpleOption> = contact_options(&state, None, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let categories: Vec<SimpleOption> = category_options(&state, None, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    render(ProjectFormTemplate {
        action: format!("/admin/projects/{}/update", id),
        id: id.clone(),
        title: project.title,
        description: project.description.unwrap_or_default(),
        contact_id: project.contact_id.map(|i| i.to_hex()).unwrap_or_default(),
        category_id: project.category_id.map(|i| i.to_hex()).unwrap_or_default(),
        priority: project.priority.as_str().to_string(),
        total_budget: project
            .total_budget
            .map(|b| format!("{:.2}", b))
            .unwrap_or_default(),
        scheduled_at: project
            .scheduled_at
            .map(|d| {
                let ms = d.timestamp_millis();
                let secs = ms / 1000;
                let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
                dt.format("%Y-%m-%d").to_string()
            })
            .unwrap_or_default(),
        notes: project.notes.unwrap_or_default(),
        contacts,
        categories,
        priority_options: priority_options(),
        is_edit: true,
        errors: None,
    })
}

pub async fn projects_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ProjectForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(o) => o,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let title = form.title.trim();
    if title.is_empty() {
        let contacts = contact_options(&state, None, &company_id)
            .await
            .unwrap_or_default();
        let categories = category_options(&state, None, &company_id)
            .await
            .unwrap_or_default();
        return render(ProjectFormTemplate {
            action: format!("/admin/projects/{}/update", id),
            id: id.clone(),
            title: form.title,
            description: form.description,
            contact_id: form.contact_id,
            category_id: form.category_id,
            priority: form.priority,
            total_budget: form.total_budget,
            scheduled_at: form.scheduled_at,
            notes: form.notes,
            contacts,
            categories,
            priority_options: priority_options(),
            is_edit: true,
            errors: Some("El título es requerido".into()),
        })
        .into_response();
    }

    let contact_id = if form.contact_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.contact_id).ok()
    };
    let category_id = if form.category_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.category_id).ok()
    };
    let total_budget = form.total_budget.parse().ok();
    let scheduled_at = parse_datetime(&form.scheduled_at);
    let priority = parse_priority(&form.priority);
    let description = if form.description.is_empty() {
        None
    } else {
        Some(form.description)
    };
    let notes = if form.notes.is_empty() {
        None
    } else {
        Some(form.notes)
    };

    match update_project(
        &state,
        &oid,
        &company_id,
        title,
        contact_id,
        category_id,
        description,
        priority,
        total_budget,
        scheduled_at,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/projects").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/projects/{id}/update",
    tag = "operations",
    params(("id" = String, Path, description = "Project id")),
    request_body = ProjectPayload,
    responses(
        (status = 200, description = "Project updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn project_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ProjectPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_project_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let parsed = match parse_project_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    match update_project(
        &state,
        &oid,
        &company_id,
        &parsed.title,
        parsed.contact_id,
        parsed.category_id,
        parsed.description,
        parsed.priority,
        parsed.total_budget,
        parsed.scheduled_at,
        parsed.notes,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn projects_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    delete_project(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/projects").into_response())
}

#[utoipa::path(
    post,
    path = "/api/admin/projects/{id}/delete",
    tag = "operations",
    params(("id" = String, Path, description = "Project id")),
    responses(
        (status = 200, description = "Project deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn project_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    get_project_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    delete_project(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

pub async fn projects_advance(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    advance_project_phase(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/projects").into_response())
}

#[utoipa::path(
    post,
    path = "/api/admin/projects/{id}/advance",
    tag = "operations",
    params(("id" = String, Path, description = "Project id")),
    responses(
        (status = 200, description = "Project phase advanced"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn project_advance_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let status = advance_project_phase(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "status": status.as_str(),
        "status_label": status.label()
    }))
    .into_response())
}

fn parse_datetime(s: &str) -> Option<bson::DateTime> {
    if s.is_empty() {
        return None;
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| {
            let dt = chrono::NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(0, 0, 0)?);
            Some(bson::DateTime::from_millis(dt.and_utc().timestamp_millis()))
        })
}

fn project_data(project: Project, can_view_money: bool) -> Option<ProjectData> {
    let id = project.id?.to_hex();
    Some(ProjectData {
        id,
        company_id: project.company_id.to_hex(),
        contact_id: project.contact_id.map(|id| id.to_hex()),
        category_id: project.category_id.map(|id| id.to_hex()),
        title: project.title,
        description: project.description,
        status: project.status.as_str().to_string(),
        status_label: project.status.label().to_string(),
        priority: project.priority.as_str().to_string(),
        priority_label: project.priority.label().to_string(),
        total_budget: if can_view_money {
            project.total_budget
        } else {
            None
        },
        scheduled_at: project
            .scheduled_at
            .map(|date| date.to_chrono().to_rfc3339()),
        completed_at: project
            .completed_at
            .map(|date| date.to_chrono().to_rfc3339()),
        notes: project.notes,
    })
}

async fn parse_project_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: ProjectPayload,
) -> Result<ParsedProjectPayload, StatusCode> {
    let title = payload.title.trim().to_string();
    if title.is_empty() || payload.total_budget.is_some_and(|amount| amount < 0.0) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let contact_id = parse_optional_object_id(payload.contact_id)?;
    let category_id = parse_optional_object_id(payload.category_id)?;
    validate_project_refs(state, company_id, category_id.as_ref(), contact_id.as_ref()).await?;
    let scheduled_at = match clean_opt(payload.scheduled_at) {
        Some(value) => {
            Some(bson::DateTime::parse_rfc3339_str(&value).map_err(|_| StatusCode::BAD_REQUEST)?)
        }
        None => None,
    };

    Ok(ParsedProjectPayload {
        title,
        contact_id,
        category_id,
        description: clean_opt(payload.description),
        priority: parse_priority_strict(&payload.priority)?,
        total_budget: payload.total_budget,
        scheduled_at,
        notes: clean_opt(payload.notes),
    })
}

fn parse_optional_object_id(value: Option<String>) -> Result<Option<ObjectId>, StatusCode> {
    match clean_opt(value) {
        Some(value) => ObjectId::parse_str(&value)
            .map(Some)
            .map_err(|_| StatusCode::BAD_REQUEST),
        None => Ok(None),
    }
}

fn clean_opt(input: Option<String>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

async fn validate_project_refs(
    state: &AppState,
    company_id: &ObjectId,
    category_id: Option<&ObjectId>,
    contact_id: Option<&ObjectId>,
) -> Result<(), StatusCode> {
    if let Some(category_id) = category_id {
        let category = crate::state::get_category_by_id(state, category_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::BAD_REQUEST)?;
        ensure_same_company(&category.company_id, company_id)?;
    }
    if let Some(contact_id) = contact_id {
        let contact = crate::state::get_contact_by_id(state, contact_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::BAD_REQUEST)?;
        ensure_same_company(&contact.company_id, company_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_priority_defaults_to_medium() {
        assert_eq!(parse_priority("low"), ProjectPriority::Low);
        assert_eq!(parse_priority("high"), ProjectPriority::High);
        assert_eq!(parse_priority("urgent"), ProjectPriority::Urgent);
        assert_eq!(parse_priority("unknown"), ProjectPriority::Medium);
    }

    #[test]
    fn priority_options_cover_all_form_values() {
        let values: Vec<_> = priority_options()
            .into_iter()
            .map(|(value, _)| value)
            .collect();

        assert_eq!(values, vec!["low", "medium", "high", "urgent"]);
    }

    #[test]
    fn parse_datetime_accepts_html_date_input() {
        let parsed = parse_datetime("2026-05-04").unwrap();

        assert_eq!(parsed.timestamp_millis(), 1_777_852_800_000);
        assert!(parse_datetime("").is_none());
        assert!(parse_datetime("2026-05-04T10:00").is_none());
    }
}
