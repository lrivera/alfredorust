use std::sync::Arc;

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use chrono::Utc;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    session::SessionUser,
    state::{
        AppState, create_resource_log, delete_resource_log, end_resource_log, get_project_by_id,
        get_resource_by_id as get_resource_by_id_state, get_resource_log_by_id, list_projects,
        list_resource_logs, list_resource_logs_for_project, list_resources as list_resources_state,
        update_resource_log,
    },
};

use super::finance::helpers::{SimpleOption, ensure_same_company, require_admin_active};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
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
