use std::sync::Arc;

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::ResourceLog,
    session::SessionUser,
    state::{
        AppState, create_resource_log, delete_resource_log, end_resource_log, get_project_by_id,
        get_resource_by_id as get_resource_by_id_state, get_resource_log_by_id,
        get_resource_log_by_id_for_company, list_projects, list_resource_logs,
        list_resource_logs_for_project, list_resources as list_resources_state,
        update_resource_log,
    },
};

use super::finance::helpers::{SimpleOption, ensure_same_company, require_admin_active};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn resource_log_data(log: ResourceLog) -> Option<ResourceLogData> {
    let id = log.id?.to_hex();
    Some(ResourceLogData {
        id,
        company_id: log.company_id.to_hex(),
        project_id: log.project_id.map(|id| id.to_hex()),
        phase: log.phase,
        resource_id: log.resource_id.map(|id| id.to_hex()),
        resource_name: log.resource_name,
        started_at: log.started_at.to_chrono().to_rfc3339(),
        ended_at: log.ended_at.map(|date| date.to_chrono().to_rfc3339()),
        duration_hours: log.duration_hours,
        operator_name: log.operator_name,
        notes: log.notes,
        created_at: log.created_at.map(|date| date.to_chrono().to_rfc3339()),
    })
}

#[derive(Template)]
#[template(path = "admin/resource_logs/index.html")]
struct ResourceLogsIndexTemplate {
    logs: Vec<ResourceLogRow>,
    project_filter: Option<String>,
}

struct ResourceLogRow {
    id: String,
    project_title: String,
    phase: String,
    resource_name: String,
    started_at: String,
    ended_at: String,
    duration_hours: String,
    operator_name: String,
    notes: String,
}

#[derive(Serialize)]
pub struct ResourceLogData {
    pub id: String,
    pub company_id: String,
    pub project_id: Option<String>,
    pub phase: Option<String>,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_hours: Option<f64>,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ResourceLogPayload {
    pub project_id: Option<String>,
    pub phase: Option<String>,
    pub resource_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct ResourceLogEndPayload {
    pub ended_at: Option<String>,
}

struct ParsedResourceLogPayload {
    project_id: Option<ObjectId>,
    phase: Option<String>,
    resource_id: Option<ObjectId>,
    resource_name: Option<String>,
    started_at: bson::DateTime,
    ended_at: Option<bson::DateTime>,
    operator_name: Option<String>,
    notes: Option<String>,
}

pub async fn resource_logs_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let logs = if let Some(project_id) = params.get("project_id") {
        let oid = ObjectId::parse_str(project_id).map_err(|_| StatusCode::BAD_REQUEST)?;
        list_resource_logs_for_project(&state, &company_id, &oid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        list_resource_logs(&state, &company_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let mut rows = Vec::new();
    for l in logs {
        let project_title = if let Some(ref pid) = l.project_id {
            get_project_by_id(&state, pid)
                .await
                .ok()
                .flatten()
                .map(|p| p.title)
                .unwrap_or_default()
        } else {
            String::new()
        };

        rows.push(ResourceLogRow {
            id: l.id.map(|i| i.to_hex()).unwrap_or_default(),
            project_title,
            phase: l.phase.unwrap_or_default(),
            resource_name: l.resource_name.unwrap_or_default(),
            started_at: format_datetime(&l.started_at),
            ended_at: l.ended_at.map(|d| format_datetime(&d)).unwrap_or_default(),
            duration_hours: l
                .duration_hours
                .map(|h| format!("{:.2}", h))
                .unwrap_or_default(),
            operator_name: l.operator_name.unwrap_or_default(),
            notes: l.notes.unwrap_or_default(),
        });
    }

    render(ResourceLogsIndexTemplate {
        logs: rows,
        project_filter: params.get("project_id").cloned(),
    })
}

pub async fn resource_logs_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ResourceLogData>>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let logs = if let Some(project_id) = params.get("project_id") {
        let oid = ObjectId::parse_str(project_id).map_err(|_| StatusCode::BAD_REQUEST)?;
        list_resource_logs_for_project(&state, &company_id, &oid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        list_resource_logs(&state, &company_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    Ok(Json(
        logs.into_iter().filter_map(resource_log_data).collect(),
    ))
}

pub async fn resource_log_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ResourceLogData>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let log = get_resource_log_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    resource_log_data(log)
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/resource_logs/form.html")]
struct ResourceLogFormTemplate {
    action: String,
    id: String,
    project_id: String,
    phase: String,
    resource_id: String,
    started_at: String,
    ended_at: String,
    operator_name: String,
    notes: String,
    projects: Vec<SimpleOption>,
    resources: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceLogForm {
    pub project_id: String,
    pub phase: String,
    pub resource_id: String,
    pub started_at: String,
    pub ended_at: String,
    pub operator_name: String,
    pub notes: String,
}

pub async fn resource_logs_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let projects: Vec<crate::models::Project> = list_projects(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let projects_opts = projects
        .into_iter()
        .map(|p| SimpleOption {
            value: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            label: p.title,
            selected: false,
        })
        .collect();

    let resources = list_resources_state(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let resources_opts = resources
        .into_iter()
        .filter(|r| r.is_active)
        .map(|r| SimpleOption {
            value: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            label: r.name,
            selected: false,
        })
        .collect();

    render(ResourceLogFormTemplate {
        action: "/admin/resource_logs".into(),
        id: String::new(),
        project_id: String::new(),
        phase: String::new(),
        resource_id: String::new(),
        started_at: Utc::now().format("%Y-%m-%dT%H:%M").to_string(),
        ended_at: String::new(),
        operator_name: String::new(),
        notes: String::new(),
        projects: projects_opts,
        resources: resources_opts,
        is_edit: false,
        errors: None,
    })
}

pub async fn resource_logs_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ResourceLogForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let project_id = if form.project_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.project_id).ok()
    };
    let resource_id = if form.resource_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.resource_id).ok()
    };
    let started_at = parse_datetime(&form.started_at).map_err(|_| StatusCode::BAD_REQUEST)?;

    let resource_name = if let Some(ref rid) = resource_id {
        get_resource_by_id_state(&state, rid)
            .await
            .ok()
            .flatten()
            .map(|r| r.name)
    } else {
        None
    };

    let phase = if form.phase.is_empty() {
        None
    } else {
        Some(form.phase)
    };
    let operator_name = if form.operator_name.is_empty() {
        None
    } else {
        Some(form.operator_name)
    };
    let notes = if form.notes.is_empty() {
        None
    } else {
        Some(form.notes)
    };

    create_resource_log(
        &state,
        &company_id,
        project_id,
        phase,
        resource_id,
        resource_name,
        started_at,
        operator_name,
        notes,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/resource_logs"))
}

pub async fn resource_logs_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResourceLogPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_resource_log_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    let id = match create_resource_log(
        &state,
        &company_id,
        parsed.project_id.clone(),
        parsed.phase.clone(),
        parsed.resource_id.clone(),
        parsed.resource_name.clone(),
        parsed.started_at,
        parsed.operator_name.clone(),
        parsed.notes.clone(),
    )
    .await
    {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if parsed.ended_at.is_some()
        && update_resource_log(
            &state,
            &id,
            &company_id,
            parsed.project_id,
            parsed.phase,
            parsed.resource_id,
            parsed.resource_name,
            parsed.started_at,
            parsed.ended_at,
            parsed.operator_name,
            parsed.notes,
        )
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id.to_hex() })),
    )
        .into_response()
}

pub async fn resource_logs_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let log = get_resource_log_by_id(&state, &oid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    ensure_same_company(&log.company_id, &company_id)?;

    let projects: Vec<crate::models::Project> = list_projects(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let projects_opts = projects
        .into_iter()
        .map(|p| SimpleOption {
            value: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            label: p.title,
            selected: false,
        })
        .collect();

    let resources = list_resources_state(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let resources_opts = resources
        .into_iter()
        .filter(|r| r.is_active)
        .map(|r| SimpleOption {
            value: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            label: r.name,
            selected: false,
        })
        .collect();

    render(ResourceLogFormTemplate {
        action: format!("/admin/resource_logs/{}/update", id),
        id: id.clone(),
        project_id: log.project_id.map(|i| i.to_hex()).unwrap_or_default(),
        phase: log.phase.unwrap_or_default(),
        resource_id: log.resource_id.map(|i| i.to_hex()).unwrap_or_default(),
        started_at: format_datetime(&log.started_at),
        ended_at: log
            .ended_at
            .map(|d| format_datetime(&d))
            .unwrap_or_default(),
        operator_name: log.operator_name.unwrap_or_default(),
        notes: log.notes.unwrap_or_default(),
        projects: projects_opts,
        resources: resources_opts,
        is_edit: true,
        errors: None,
    })
}

pub async fn resource_logs_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ResourceLogForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let project_id = if form.project_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.project_id).ok()
    };
    let resource_id = if form.resource_id.is_empty() {
        None
    } else {
        ObjectId::parse_str(&form.resource_id).ok()
    };
    let started_at = parse_datetime(&form.started_at).map_err(|_| StatusCode::BAD_REQUEST)?;
    let ended_at = if form.ended_at.is_empty() {
        None
    } else {
        parse_datetime(&form.ended_at).ok()
    };

    let resource_name = if let Some(ref rid) = resource_id {
        get_resource_by_id_state(&state, rid)
            .await
            .ok()
            .flatten()
            .map(|r| r.name)
    } else {
        None
    };

    let phase = if form.phase.is_empty() {
        None
    } else {
        Some(form.phase)
    };
    let operator_name = if form.operator_name.is_empty() {
        None
    } else {
        Some(form.operator_name)
    };
    let notes = if form.notes.is_empty() {
        None
    } else {
        Some(form.notes)
    };

    update_resource_log(
        &state,
        &oid,
        &company_id,
        project_id,
        phase,
        resource_id,
        resource_name,
        started_at,
        ended_at,
        operator_name,
        notes,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/resource_logs"))
}

pub async fn resource_log_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ResourceLogPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_resource_log_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let parsed = match parse_resource_log_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    match update_resource_log(
        &state,
        &oid,
        &company_id,
        parsed.project_id,
        parsed.phase,
        parsed.resource_id,
        parsed.resource_name,
        parsed.started_at,
        parsed.ended_at,
        parsed.operator_name,
        parsed.notes,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resource_logs_end(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let now = bson::DateTime::from_system_time(std::time::SystemTime::now());

    end_resource_log(&state, &oid, &company_id, now)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/resource_logs"))
}

pub async fn resource_log_end_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ResourceLogEndPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let ended_at = match clean_opt(payload.ended_at) {
        Some(value) => {
            bson::DateTime::parse_rfc3339_str(&value).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        None => bson::DateTime::from_system_time(std::time::SystemTime::now()),
    };
    let duration_hours = end_resource_log(&state, &oid, &company_id, ended_at)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "duration_hours": duration_hours
    }))
    .into_response())
}

pub async fn resource_logs_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    delete_resource_log(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/admin/resource_logs"))
}

pub async fn resource_log_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    get_resource_log_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    delete_resource_log(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

async fn parse_resource_log_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: ResourceLogPayload,
) -> Result<ParsedResourceLogPayload, StatusCode> {
    let project_id = parse_optional_object_id(payload.project_id)?;
    let resource_id = parse_optional_object_id(payload.resource_id)?;
    if let Some(project_id) = &project_id {
        let project = get_project_by_id(state, project_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::BAD_REQUEST)?;
        ensure_same_company(&project.company_id, company_id)?;
    }
    let resource_name = if let Some(resource_id) = &resource_id {
        let resource = get_resource_by_id_state(state, resource_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::BAD_REQUEST)?;
        ensure_same_company(&resource.company_id, company_id)?;
        Some(resource.name)
    } else {
        None
    };
    let started_at = bson::DateTime::parse_rfc3339_str(&payload.started_at)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let ended_at = match clean_opt(payload.ended_at) {
        Some(value) => {
            Some(bson::DateTime::parse_rfc3339_str(&value).map_err(|_| StatusCode::BAD_REQUEST)?)
        }
        None => None,
    };

    Ok(ParsedResourceLogPayload {
        project_id,
        phase: clean_opt(payload.phase),
        resource_id,
        resource_name,
        started_at,
        ended_at,
        operator_name: clean_opt(payload.operator_name),
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

fn format_datetime(dt: &bson::DateTime) -> String {
    let ms = dt.timestamp_millis();
    let secs = ms / 1000;
    let datetime = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
    datetime.format("%Y-%m-%dT%H:%M").to_string()
}

fn parse_datetime(s: &str) -> Result<bson::DateTime, ()> {
    if s.is_empty() {
        return Err(());
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
        .map(|dt| bson::DateTime::from_millis(dt.and_utc().timestamp_millis()))
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_datetime_accepts_html_datetime_input() {
        let parsed = parse_datetime("2026-05-04T10:30").unwrap();

        assert_eq!(parsed.timestamp_millis(), 1_777_890_600_000);
        assert!(parse_datetime("").is_err());
        assert!(parse_datetime("2026-05-04").is_err());
    }

    #[test]
    fn format_datetime_returns_html_datetime_value() {
        let dt = bson::DateTime::from_millis(1_777_890_600_000);

        assert_eq!(format_datetime(&dt), "2026-05-04T10:30");
    }
}
