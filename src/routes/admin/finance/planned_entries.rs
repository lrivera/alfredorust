use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::PlannedEntry,
    session::SessionUser,
    state::{
        AppState, create_planned_entry, delete_planned_entry, get_planned_entry_by_id,
        get_project_by_id_for_company, list_planned_entries, list_projects,
        pay_planned_entry_with_project, update_planned_entry, update_planned_entry_project_links,
    },
};

use super::helpers::*;
use super::options::{account_options, category_options, contact_options, recurring_plan_options};

#[derive(Template)]
#[template(path = "admin/planned_entries/index.html")]
struct PlannedEntriesIndexTemplate {
    entries: Vec<PlannedEntryRow>,
}

struct PlannedEntryRow {
    id: String,
    name: String,
    company: String,
    flow_type: String,
    amount: f64,
    original_amount: f64,
    status: String,
    status_label: String,
}

#[derive(Serialize)]
pub struct PlannedEntryData {
    pub id: String,
    pub company_id: String,
    pub company: String,
    pub recurring_plan_id: Option<String>,
    pub recurring_plan_version: Option<i32>,
    pub service_order_id: Option<String>,
    pub project_id: Option<String>,
    pub parent_planned_entry_id: Option<String>,
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub original_amount_estimated: Option<f64>,
    pub due_date: String,
    pub original_due_date: Option<String>,
    pub status: String,
    pub status_label: String,
    pub notes: Option<String>,
    pub cfdi_uuid: Option<String>,
    pub currency: Option<String>,
    pub cfdi_folio: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/planned_entries/form.html")]
struct PlannedEntryFormTemplate {
    action: String,
    name: String,
    flow_type: String,
    amount_estimated: String,
    due_date: String,
    status: String,
    notes: String,
    companies: Vec<SimpleOption>,
    flow_options: Vec<SimpleOption>,
    status_options: Vec<SimpleOption>,
    categories: Vec<SimpleOption>,
    accounts: Vec<SimpleOption>,
    contacts: Vec<SimpleOption>,
    projects: Vec<SimpleOption>,
    recurring_plans: Vec<SimpleOption>,
    recurring_plan_version: String,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct PlannedEntryFormData {
    name: String,
    company_id: String,
    flow_type: String,
    category_id: String,
    account_expected_id: String,
    #[serde(default)]
    contact_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    amount_estimated: String,
    due_date: String,
    status: String,
    #[serde(default)]
    recurring_plan_id: Option<String>,
    #[serde(default)]
    recurring_plan_version: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PlannedEntryPayload {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub project_id: Option<String>,
    pub amount_estimated: f64,
    pub due_date: String,
    pub status: String,
    pub recurring_plan_id: Option<String>,
    pub recurring_plan_version: Option<i32>,
    pub notes: Option<String>,
}

struct ParsedPlannedEntryPayload {
    name: String,
    flow_type: crate::models::FlowType,
    category_id: ObjectId,
    account_expected_id: ObjectId,
    contact_id: Option<ObjectId>,
    project_id: Option<ObjectId>,
    amount_estimated: f64,
    due_date: mongodb::bson::DateTime,
    status: crate::models::PlannedStatus,
    recurring_plan_id: Option<ObjectId>,
    recurring_plan_version: Option<i32>,
    notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PlannedEntryPayPayload {
    pub paid_at: String,
    pub amount: f64,
    pub account_id: String,
    pub project_id: Option<String>,
    pub parent_planned_entry_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PlannedEntryBulkPayPayload {
    pub entry_ids: Vec<String>,
    pub paid_at: String,
    pub account_id: String,
    pub project_id: Option<String>,
    pub parent_planned_entry_id: Option<String>,
    pub notes: Option<String>,
}

pub async fn planned_entries_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let entries = list_planned_entries(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_name = session_user.user().company_name.clone();

    let rows = entries
        .into_iter()
        .filter(|e| e.company_id == active_company)
        .filter_map(|e| {
            e.id.map(|id| PlannedEntryRow {
                id: id.to_hex(),
                name: e.name,
                company: active_name.clone(),
                flow_type: flow_type_value(&e.flow_type).to_string(),
                amount: e.amount_estimated,
                original_amount: e.original_amount_estimated.unwrap_or(0.0),
                status: planned_status_value(&e.status).to_string(),
                status_label: planned_status_label(&e.status).to_string(),
            })
        })
        .collect();

    render(PlannedEntriesIndexTemplate { entries: rows })
}

#[utoipa::path(
    get,
    path = "/api/admin/planned-entries",
    tag = "finance",
    responses(
        (status = 200, description = "List planned entries"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn planned_entries_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PlannedEntryData>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let active_name = session_user.user().company_name.clone();
    let entries = list_planned_entries(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = entries
        .into_iter()
        .filter(|entry| entry.company_id == active_company)
        .filter_map(|entry| planned_entry_data(entry, active_name.clone()))
        .collect();

    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/api/admin/planned-entries/{id}",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Get planned entry"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn planned_entry_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<PlannedEntryData>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let entry = get_planned_entry_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&entry.company_id, &active_company)?;

    planned_entry_data(entry, session_user.user().company_name.clone())
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    post,
    path = "/api/admin/planned-entries",
    tag = "finance",
    request_body = PlannedEntryPayload,
    responses(
        (status = 201, description = "Create planned entry"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn planned_entries_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlannedEntryPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_planned_entry_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    match create_planned_entry(
        &state,
        &company_id,
        parsed.recurring_plan_id,
        parsed.recurring_plan_version,
        None,
        &parsed.name,
        parsed.flow_type,
        &parsed.category_id,
        &parsed.account_expected_id,
        parsed.contact_id,
        parsed.amount_estimated,
        parsed.due_date,
        parsed.status,
        parsed.notes,
    )
    .await
    {
        Ok(id) => {
            if parsed.project_id.is_some() {
                if update_planned_entry_project_links(
                    &state,
                    &id,
                    &company_id,
                    parsed.project_id,
                    None,
                )
                .await
                .is_err()
                {
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            }
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": id.to_hex(),
                    "side_effects": { "project_link_updated": true }
                })),
            )
                .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/planned-entries/{id}/update",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    request_body = PlannedEntryPayload,
    responses(
        (status = 200, description = "Update planned entry"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn planned_entry_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<PlannedEntryPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(status) = match get_planned_entry_by_id(&state, &object_id).await {
        Ok(Some(entry)) => ensure_same_company(&entry.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }
    let parsed = match parse_planned_entry_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    match update_planned_entry(
        &state,
        &object_id,
        &company_id,
        parsed.recurring_plan_id,
        parsed.recurring_plan_version,
        &parsed.name,
        parsed.flow_type,
        &parsed.category_id,
        &parsed.account_expected_id,
        parsed.contact_id,
        parsed.amount_estimated,
        parsed.due_date,
        parsed.status,
        parsed.notes,
    )
    .await
    {
        Ok(_) => {
            if update_planned_entry_project_links(
                &state,
                &object_id,
                &company_id,
                parsed.project_id,
                None,
            )
            .await
            .is_err()
            {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            Json(serde_json::json!({
                "ok": true,
                "side_effects": { "planned_entry_recalculated": object_id.to_hex() }
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/planned-entries/{id}/delete",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Delete planned entry"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn planned_entry_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(status) = match get_planned_entry_by_id(&state, &object_id).await {
        Ok(Some(entry)) => ensure_same_company(&entry.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_planned_entry(&state, &object_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn planned_entries_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let categories = category_options(&state, None, &active_company).await?;
    let accounts = account_options(&state, None, &active_company).await?;
    let contacts = contact_options(&state, None, &active_company).await?;
    let projects = project_options(&state, &active_company, None).await?;
    let recurring_plans = recurring_plan_options(&state, None, &active_company).await?;

    render(PlannedEntryFormTemplate {
        action: "/admin/planned_entries".into(),
        name: String::new(),
        flow_type: "expense".into(),
        amount_estimated: "0".into(),
        due_date: String::new(),
        status: "planned".into(),
        notes: String::new(),
        companies,
        flow_options: flow_options("expense"),
        status_options: planned_status_options("planned"),
        categories,
        accounts,
        contacts,
        projects,
        recurring_plans,
        recurring_plan_version: String::new(),
        is_edit: false,
        errors: None,
    })
}

pub async fn planned_entries_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<PlannedEntryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let category_id = match parse_object_id(&form.category_id, "Categoría") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let account_expected_id = match parse_object_id(&form.account_expected_id, "Cuenta esperada") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let contact_id = match form.contact_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(cid) => match parse_object_id(&cid, "Contacto") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let recurring_plan_id = match form.recurring_plan_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(rpid) => match parse_object_id(&rpid, "Plan recurrente") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let recurring_plan_version = match form
        .recurring_plan_version
        .clone()
        .and_then(|v| if v.trim().is_empty() { None } else { Some(v) })
    {
        Some(v) => match parse_i32_field(&v, "Versión del plan") {
            Ok(num) => Some(num),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let amount_estimated = match parse_f64_field(&form.amount_estimated, "Monto estimado") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let due_date = match parse_datetime_field(&form.due_date, "Fecha de vencimiento") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let status = match parse_planned_status(&form.status) {
        Ok(s) => s,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let notes = clean_opt(form.notes);
    let project_id =
        parse_optional_project_id(&state, &company_id, form.project_id.as_deref()).await;
    let project_id = match project_id {
        Ok(project_id) => project_id,
        Err(status) => return status.into_response(),
    };

    if let Err(status) = validate_company_refs(
        &state,
        &company_id,
        Some(&category_id),
        Some(&account_expected_id),
        contact_id.as_ref(),
    )
    .await
    {
        return status.into_response();
    }

    if let Some(ref plan_id) = recurring_plan_id {
        if let Err(status) = validate_recurring_plan_company(&state, plan_id, &company_id).await {
            return status.into_response();
        }
    }

    match create_planned_entry(
        &state,
        &company_id,
        recurring_plan_id,
        recurring_plan_version,
        None,
        form.name.trim(),
        flow_type,
        &category_id,
        &account_expected_id,
        contact_id,
        amount_estimated,
        due_date,
        status,
        notes,
    )
    .await
    {
        Ok(planned_entry_id) => {
            if project_id.is_some() {
                let _ = update_planned_entry_project_links(
                    &state,
                    &planned_entry_id,
                    &company_id,
                    project_id,
                    None,
                )
                .await;
            }
            Redirect::to("/admin/planned_entries").into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn planned_entries_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let entry = get_planned_entry_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&entry.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let categories = category_options(&state, Some(&entry.category_id), &active_company).await?;
    let accounts =
        account_options(&state, Some(&entry.account_expected_id), &active_company).await?;
    let contacts = contact_options(&state, entry.contact_id.as_ref(), &active_company).await?;
    let projects = project_options(&state, &active_company, entry.project_id.as_ref()).await?;
    let recurring_plans =
        recurring_plan_options(&state, entry.recurring_plan_id.as_ref(), &active_company).await?;

    render(PlannedEntryFormTemplate {
        action: format!("/admin/planned_entries/{}/update", id),
        name: entry.name,
        flow_type: flow_type_value(&entry.flow_type).to_string(),
        amount_estimated: entry.amount_estimated.to_string(),
        due_date: datetime_to_string(&entry.due_date),
        status: planned_status_value(&entry.status).to_string(),
        notes: entry.notes.unwrap_or_default(),
        companies,
        flow_options: flow_options(flow_type_value(&entry.flow_type)),
        status_options: planned_status_options(planned_status_value(&entry.status)),
        categories,
        accounts,
        contacts,
        projects,
        recurring_plans,
        recurring_plan_version: entry
            .recurring_plan_version
            .map(|v| v.to_string())
            .unwrap_or_default(),
        is_edit: true,
        errors: None,
    })
}

pub async fn planned_entries_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<PlannedEntryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = match get_planned_entry_by_id(&state, &object_id).await {
        Ok(Some(entry)) => ensure_same_company(&entry.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }
    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let category_id = match parse_object_id(&form.category_id, "Categoría") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let account_expected_id = match parse_object_id(&form.account_expected_id, "Cuenta esperada") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let contact_id = match form.contact_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(cid) => match parse_object_id(&cid, "Contacto") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let recurring_plan_id = match form.recurring_plan_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(rpid) => match parse_object_id(&rpid, "Plan recurrente") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let recurring_plan_version = match form
        .recurring_plan_version
        .clone()
        .and_then(|v| if v.trim().is_empty() { None } else { Some(v) })
    {
        Some(v) => match parse_i32_field(&v, "Versión del plan") {
            Ok(num) => Some(num),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let amount_estimated = match parse_f64_field(&form.amount_estimated, "Monto estimado") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let due_date = match parse_datetime_field(&form.due_date, "Fecha de vencimiento") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let status_enum = match parse_planned_status(&form.status) {
        Ok(s) => s,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let notes = clean_opt(form.notes);
    let project_id =
        parse_optional_project_id(&state, &company_id, form.project_id.as_deref()).await;
    let project_id = match project_id {
        Ok(project_id) => project_id,
        Err(status) => return status.into_response(),
    };

    if let Err(status) = validate_company_refs(
        &state,
        &company_id,
        Some(&category_id),
        Some(&account_expected_id),
        contact_id.as_ref(),
    )
    .await
    {
        return status.into_response();
    }

    if let Some(ref plan_id) = recurring_plan_id {
        if let Err(status) = validate_recurring_plan_company(&state, plan_id, &company_id).await {
            return status.into_response();
        }
    }

    match update_planned_entry(
        &state,
        &object_id,
        &company_id,
        recurring_plan_id,
        recurring_plan_version,
        form.name.trim(),
        flow_type,
        &category_id,
        &account_expected_id,
        contact_id,
        amount_estimated,
        due_date,
        status_enum,
        notes,
    )
    .await
    {
        Ok(_) => {
            let _ = update_planned_entry_project_links(
                &state,
                &object_id,
                &company_id,
                project_id,
                None,
            )
            .await;
            Redirect::to("/admin/planned_entries").into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn planned_entries_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let active_company = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = match get_planned_entry_by_id(&state, &object_id).await {
        Ok(Some(entry)) => ensure_same_company(&entry.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_planned_entry(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/planned_entries").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ── Pay ────────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin/planned_entries/pay.html")]
struct PayFormTemplate {
    entry_id: String,
    entry_name: String,
    paid_at: String,
    amount: String,
    original_amount: f64,
    accounts: Vec<SimpleOption>,
    projects: Vec<SimpleOption>,
    parent_entries: Vec<SimpleOption>,
    errors: Option<String>,
    return_to: String,
}

#[derive(Template)]
#[template(path = "admin/planned_entries/bulk_pay.html")]
struct BulkPayFormTemplate {
    entries: Vec<BulkPayEntryRow>,
    entry_ids: String,
    paid_at: String,
    total_amount: f64,
    accounts: Vec<SimpleOption>,
    projects: Vec<SimpleOption>,
    parent_entries: Vec<SimpleOption>,
    errors: Option<String>,
    return_to: String,
}

struct BulkPayEntryRow {
    name: String,
    amount: f64,
}

#[derive(Deserialize)]
pub struct PayQuery {
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkPayQuery {
    ids: String,
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkPayFormData {
    entry_ids: String,
    paid_at: String,
    account_id: String,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    parent_planned_entry_id: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    return_to: Option<String>,
}

pub async fn planned_entries_bulk_pay_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<BulkPayQuery>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let entry_ids = parse_entry_ids(&query.ids).map_err(|_| StatusCode::BAD_REQUEST)?;
    let entries = load_payable_entries(&state, &company_id, &entry_ids).await?;
    let accounts = account_options(&state, None, &company_id).await?;
    let projects = project_options(&state, &company_id, None).await?;
    let parent_entries = parent_entry_options(&state, &company_id, None, None).await?;
    let total_amount = entries.iter().map(|entry| entry.amount_estimated).sum();
    let rows = entries
        .into_iter()
        .map(|entry| BulkPayEntryRow {
            name: entry.name,
            amount: entry.amount_estimated,
        })
        .collect();

    render(BulkPayFormTemplate {
        entries: rows,
        entry_ids: query.ids,
        paid_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        total_amount,
        accounts,
        projects,
        parent_entries,
        errors: None,
        return_to: safe_return_to(query.return_to.as_deref())
            .unwrap_or("/admin/planned_entries")
            .to_string(),
    })
}

pub async fn planned_entries_bulk_pay(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<BulkPayFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let entry_ids = match parse_entry_ids(&form.entry_ids) {
        Ok(ids) => ids,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let entries = match load_payable_entries(&state, &company_id, &entry_ids).await {
        Ok(entries) => entries,
        Err(status) => return status.into_response(),
    };
    let account_id = match parse_object_id(&form.account_id, "Cuenta") {
        Ok(id) => id,
        Err(_) => {
            return render_bulk_pay_form_error(
                &state,
                &company_id,
                &form,
                &entries,
                "Selecciona una cuenta válida",
            )
            .await
            .into_response();
        }
    };
    let paid_date = match parse_date_field(&form.paid_at) {
        Some(date) => date,
        None => {
            return render_bulk_pay_form_error(
                &state,
                &company_id,
                &form,
                &entries,
                "Captura una fecha de pago válida",
            )
            .await
            .into_response();
        }
    };
    let project_id =
        match parse_optional_project_id(&state, &company_id, form.project_id.as_deref()).await {
            Ok(project_id) => project_id,
            Err(_) => {
                return render_bulk_pay_form_error(
                    &state,
                    &company_id,
                    &form,
                    &entries,
                    "Selecciona un proyecto válido",
                )
                .await
                .into_response();
            }
        };
    let parent_planned_entry_id = match parse_optional_parent_entry_id(
        &state,
        &company_id,
        form.parent_planned_entry_id.as_deref(),
        project_id.as_ref(),
    )
    .await
    {
        Ok(parent_id) => parent_id,
        Err(_) => {
            return render_bulk_pay_form_error(
                &state,
                &company_id,
                &form,
                &entries,
                "Selecciona un compromiso estimado válido",
            )
            .await
            .into_response();
        }
    };
    let notes = clean_opt(form.notes.clone());

    for entry in entries {
        let Some(entry_id) = entry.id else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        if let Err(err) = pay_planned_entry_with_project(
            &state,
            &entry_id,
            &company_id,
            &account_id,
            entry.amount_estimated,
            paid_date,
            notes.clone(),
            project_id.clone(),
            parent_planned_entry_id.clone(),
        )
        .await
        {
            eprintln!("[planned_entries] bulk pay failed for {entry_id}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }

    Redirect::to(safe_return_to(form.return_to.as_deref()).unwrap_or("/admin/planned_entries"))
        .into_response()
}

pub async fn planned_entries_pay_form(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PayQuery>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let entry = get_planned_entry_by_id(&state, &oid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&entry.company_id, &company_id)?;

    let accounts = account_options(&state, None, &company_id).await?;
    let projects = project_options(&state, &company_id, entry.project_id.as_ref()).await?;
    let parent_entries = parent_entry_options(
        &state,
        &company_id,
        entry.id.as_ref(),
        entry.parent_planned_entry_id.as_ref(),
    )
    .await?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    render(PayFormTemplate {
        entry_id: id,
        entry_name: entry.name,
        paid_at: today,
        amount: entry.amount_estimated.to_string(),
        original_amount: entry.original_amount_estimated.unwrap_or(0.0),
        accounts,
        projects,
        parent_entries,
        errors: None,
        return_to: safe_return_to(query.return_to.as_deref())
            .unwrap_or("/admin/planned_entries")
            .to_string(),
    })
}

#[derive(Deserialize)]
pub struct PayFormData {
    paid_at: String,
    amount: String,
    account_id: String,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    parent_planned_entry_id: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    return_to: Option<String>,
}

pub async fn planned_entries_pay(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<PayFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let entry = match get_planned_entry_by_id(&state, &oid).await {
        Ok(Some(entry)) => {
            if let Err(status) = ensure_same_company(&entry.company_id, &company_id) {
                return status.into_response();
            }
            entry
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let account_id = match parse_object_id(&form.account_id, "Cuenta") {
        Ok(id) => id,
        Err(_) => {
            return render_pay_form_error(
                &state,
                &company_id,
                id,
                &entry.name,
                &form,
                entry.original_amount_estimated.unwrap_or(0.0),
                "Selecciona una cuenta válida",
            )
            .await
            .into_response();
        }
    };
    let paid_amount = match parse_f64_field(&form.amount, "Monto") {
        Ok(v) => v,
        Err(_) => {
            return render_pay_form_error(
                &state,
                &company_id,
                id,
                &entry.name,
                &form,
                entry.original_amount_estimated.unwrap_or(0.0),
                "Captura un monto válido",
            )
            .await
            .into_response();
        }
    };
    let paid_date = match parse_date_field(&form.paid_at) {
        Some(d) => d,
        None => {
            return render_pay_form_error(
                &state,
                &company_id,
                id,
                &entry.name,
                &form,
                entry.original_amount_estimated.unwrap_or(0.0),
                "Captura una fecha de pago válida",
            )
            .await
            .into_response();
        }
    };
    let notes = clean_opt(form.notes.clone());
    let project_id = match clean_opt(form.project_id.clone()) {
        Some(value) => match parse_object_id(&value, "Proyecto") {
            Ok(project_oid) => {
                match get_project_by_id_for_company(&state, &project_oid, &company_id).await {
                    Ok(Some(_)) => Some(project_oid),
                    Ok(None) => {
                        return render_pay_form_error(
                            &state,
                            &company_id,
                            id,
                            &entry.name,
                            &form,
                            entry.original_amount_estimated.unwrap_or(0.0),
                            "Selecciona un proyecto válido",
                        )
                        .await
                        .into_response();
                    }
                    Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                }
            }
            Err(_) => {
                return render_pay_form_error(
                    &state,
                    &company_id,
                    id,
                    &entry.name,
                    &form,
                    entry.original_amount_estimated.unwrap_or(0.0),
                    "Selecciona un proyecto válido",
                )
                .await
                .into_response();
            }
        },
        None => entry.project_id.clone(),
    };
    let parent_planned_entry_id = match clean_opt(form.parent_planned_entry_id.clone()) {
        Some(value) => match parse_object_id(&value, "Compromiso estimado") {
            Ok(parent_id) => match get_planned_entry_by_id(&state, &parent_id).await {
                Ok(Some(parent)) => {
                    if parent.company_id != company_id || parent.id.as_ref() == Some(&oid) {
                        return render_pay_form_error(
                            &state,
                            &company_id,
                            id,
                            &entry.name,
                            &form,
                            entry.original_amount_estimated.unwrap_or(0.0),
                            "Selecciona un compromiso estimado válido",
                        )
                        .await
                        .into_response();
                    }
                    if let (Some(parent_project), Some(project)) = (&parent.project_id, &project_id)
                    {
                        if parent_project != project {
                            return render_pay_form_error(
                                &state,
                                &company_id,
                                id,
                                &entry.name,
                                &form,
                                entry.original_amount_estimated.unwrap_or(0.0),
                                "El compromiso estimado debe pertenecer al proyecto seleccionado",
                            )
                            .await
                            .into_response();
                        }
                    }
                    Some(parent_id)
                }
                Ok(None) => {
                    return render_pay_form_error(
                        &state,
                        &company_id,
                        id,
                        &entry.name,
                        &form,
                        entry.original_amount_estimated.unwrap_or(0.0),
                        "Selecciona un compromiso estimado válido",
                    )
                    .await
                    .into_response();
                }
                Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            },
            Err(_) => {
                return render_pay_form_error(
                    &state,
                    &company_id,
                    id,
                    &entry.name,
                    &form,
                    entry.original_amount_estimated.unwrap_or(0.0),
                    "Selecciona un compromiso estimado válido",
                )
                .await
                .into_response();
            }
        },
        None => entry.parent_planned_entry_id.clone(),
    };

    match pay_planned_entry_with_project(
        &state,
        &oid,
        &company_id,
        &account_id,
        paid_amount,
        paid_date,
        notes,
        project_id,
        parent_planned_entry_id,
    )
    .await
    {
        Ok(_) => Redirect::to(
            safe_return_to(form.return_to.as_deref()).unwrap_or("/admin/planned_entries"),
        )
        .into_response(),
        Err(err) => {
            eprintln!("[planned_entries] pay failed for {id}: {err}");
            let msg = format!("No se pudo registrar el pago: {err}");
            render_pay_form_error(
                &state,
                &company_id,
                id,
                &entry.name,
                &form,
                entry.original_amount_estimated.unwrap_or(0.0),
                &msg,
            )
            .await
            .into_response()
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/planned-entries/{id}/pay",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    request_body = PlannedEntryPayPayload,
    responses(
        (status = 200, description = "Pay planned entry"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn planned_entry_pay_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<PlannedEntryPayPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let entry = match get_planned_entry_by_id(&state, &object_id).await {
        Ok(Some(entry)) => {
            if let Err(status) = ensure_same_company(&entry.company_id, &company_id) {
                return status.into_response();
            }
            entry
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if matches!(
        entry.status,
        crate::models::PlannedStatus::Covered | crate::models::PlannedStatus::Cancelled
    ) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let payment =
        match parse_planned_entry_payment_payload(&state, &company_id, &entry, &object_id, payload)
            .await
        {
            Ok(payment) => payment,
            Err(status) => return status.into_response(),
        };

    match pay_planned_entry_with_project(
        &state,
        &object_id,
        &company_id,
        &payment.account_id,
        payment.amount,
        payment.paid_at,
        payment.notes,
        payment.project_id,
        payment.parent_planned_entry_id,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "side_effects": {
                "transaction_created": true,
                "planned_entry_recalculated": object_id.to_hex()
            }
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/planned-entries/bulk-pay",
    tag = "finance",
    request_body = PlannedEntryBulkPayPayload,
    responses(
        (status = 200, description = "Bulk pay planned entries"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn planned_entries_bulk_pay_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlannedEntryBulkPayPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let entry_ids = match parse_entry_ids(&payload.entry_ids.join(",")) {
        Ok(ids) => ids,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let entries = match load_payable_entries(&state, &company_id, &entry_ids).await {
        Ok(entries) => entries,
        Err(status) => return status.into_response(),
    };
    let account_id = match parse_object_id(&payload.account_id, "account_id") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(status) =
        validate_company_refs(&state, &company_id, None, Some(&account_id), None).await
    {
        return status.into_response();
    }
    let paid_at = match parse_datetime_field(&payload.paid_at, "paid_at") {
        Ok(date) => date,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let project_id =
        match parse_optional_project_id(&state, &company_id, payload.project_id.as_deref()).await {
            Ok(project_id) => project_id,
            Err(status) => return status.into_response(),
        };
    let parent_planned_entry_id = match parse_optional_parent_entry_id(
        &state,
        &company_id,
        payload.parent_planned_entry_id.as_deref(),
        project_id.as_ref(),
    )
    .await
    {
        Ok(parent_id) => parent_id,
        Err(status) => return status.into_response(),
    };
    let notes = clean_opt(payload.notes);

    let mut paid_ids = Vec::new();
    for entry in entries {
        let Some(entry_id) = entry.id else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        if pay_planned_entry_with_project(
            &state,
            &entry_id,
            &company_id,
            &account_id,
            entry.amount_estimated,
            paid_at,
            notes.clone(),
            project_id.clone(),
            parent_planned_entry_id.clone(),
        )
        .await
        .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        paid_ids.push(entry_id.to_hex());
    }

    Json(serde_json::json!({
        "ok": true,
        "paid_entry_ids": paid_ids,
        "side_effects": { "transactions_created": paid_ids.len() }
    }))
    .into_response()
}

struct ParsedPaymentPayload {
    paid_at: mongodb::bson::DateTime,
    amount: f64,
    account_id: ObjectId,
    project_id: Option<ObjectId>,
    parent_planned_entry_id: Option<ObjectId>,
    notes: Option<String>,
}

async fn render_pay_form_error(
    state: &AppState,
    company_id: &ObjectId,
    entry_id: String,
    entry_name: &str,
    form: &PayFormData,
    original_amount: f64,
    message: &str,
) -> Result<Html<String>, StatusCode> {
    let accounts = account_options(state, None, company_id).await?;
    let project_id = clean_opt(form.project_id.clone()).unwrap_or_default();
    let parent_planned_entry_id =
        clean_opt(form.parent_planned_entry_id.clone()).unwrap_or_default();
    let selected_project = ObjectId::from_str(&project_id).ok();
    let selected_parent = ObjectId::from_str(&parent_planned_entry_id).ok();
    let projects = project_options(state, company_id, selected_project.as_ref()).await?;
    let parent_entries =
        parent_entry_options(state, company_id, None, selected_parent.as_ref()).await?;
    render(PayFormTemplate {
        entry_id,
        entry_name: entry_name.to_string(),
        paid_at: form.paid_at.clone(),
        amount: form.amount.clone(),
        original_amount,
        accounts,
        projects,
        parent_entries,
        errors: Some(message.to_string()),
        return_to: safe_return_to(form.return_to.as_deref())
            .unwrap_or("/admin/planned_entries")
            .to_string(),
    })
}

async fn render_bulk_pay_form_error(
    state: &AppState,
    company_id: &ObjectId,
    form: &BulkPayFormData,
    entries: &[crate::models::PlannedEntry],
    message: &str,
) -> Result<Html<String>, StatusCode> {
    let selected_project =
        clean_opt(form.project_id.clone()).and_then(|id| ObjectId::from_str(&id).ok());
    let selected_parent =
        clean_opt(form.parent_planned_entry_id.clone()).and_then(|id| ObjectId::from_str(&id).ok());
    let accounts = account_options(state, None, company_id).await?;
    let projects = project_options(state, company_id, selected_project.as_ref()).await?;
    let parent_entries =
        parent_entry_options(state, company_id, None, selected_parent.as_ref()).await?;
    let total_amount = entries.iter().map(|entry| entry.amount_estimated).sum();
    let rows = entries
        .iter()
        .map(|entry| BulkPayEntryRow {
            name: entry.name.clone(),
            amount: entry.amount_estimated,
        })
        .collect();

    render(BulkPayFormTemplate {
        entries: rows,
        entry_ids: form.entry_ids.clone(),
        paid_at: form.paid_at.clone(),
        total_amount,
        accounts,
        projects,
        parent_entries,
        errors: Some(message.to_string()),
        return_to: safe_return_to(form.return_to.as_deref())
            .unwrap_or("/admin/planned_entries")
            .to_string(),
    })
}

fn parse_entry_ids(value: &str) -> Result<Vec<ObjectId>, ()> {
    let mut ids = Vec::new();
    for raw in value.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id = ObjectId::from_str(trimmed).map_err(|_| ())?;
        if !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(());
    }
    Ok(ids)
}

async fn parse_planned_entry_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: PlannedEntryPayload,
) -> Result<ParsedPlannedEntryPayload, StatusCode> {
    let name = payload.name.trim().to_string();
    if name.is_empty() || payload.amount_estimated < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let flow_type = parse_flow_type(&payload.flow_type).map_err(|_| StatusCode::BAD_REQUEST)?;
    let category_id = parse_object_id(&payload.category_id, "category_id")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let account_expected_id = parse_object_id(&payload.account_expected_id, "account_expected_id")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let contact_id = parse_optional_object_id(payload.contact_id)?;
    let project_id =
        parse_optional_project_id(state, company_id, payload.project_id.as_deref()).await?;
    let due_date =
        parse_datetime_field(&payload.due_date, "due_date").map_err(|_| StatusCode::BAD_REQUEST)?;
    let status = parse_planned_status(&payload.status).map_err(|_| StatusCode::BAD_REQUEST)?;
    let recurring_plan_id = parse_optional_object_id(payload.recurring_plan_id)?;

    validate_company_refs(
        state,
        company_id,
        Some(&category_id),
        Some(&account_expected_id),
        contact_id.as_ref(),
    )
    .await?;
    if let Some(ref plan_id) = recurring_plan_id {
        validate_recurring_plan_company(state, plan_id, company_id).await?;
    }

    Ok(ParsedPlannedEntryPayload {
        name,
        flow_type,
        category_id,
        account_expected_id,
        contact_id,
        project_id,
        amount_estimated: payload.amount_estimated,
        due_date,
        status,
        recurring_plan_id,
        recurring_plan_version: payload.recurring_plan_version,
        notes: clean_opt(payload.notes),
    })
}

async fn parse_planned_entry_payment_payload(
    state: &AppState,
    company_id: &ObjectId,
    entry: &crate::models::PlannedEntry,
    entry_id: &ObjectId,
    payload: PlannedEntryPayPayload,
) -> Result<ParsedPaymentPayload, StatusCode> {
    if payload.amount < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let account_id =
        parse_object_id(&payload.account_id, "account_id").map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_company_refs(state, company_id, None, Some(&account_id), None).await?;
    let paid_at =
        parse_datetime_field(&payload.paid_at, "paid_at").map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_id = match payload.project_id.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            parse_optional_project_id(state, company_id, Some(value)).await?
        }
        _ => entry.project_id.clone(),
    };
    let parent_planned_entry_id = match payload.parent_planned_entry_id.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            let parent_id =
                parse_optional_parent_entry_id(state, company_id, Some(value), project_id.as_ref())
                    .await?;
            if parent_id.as_ref() == Some(entry_id) {
                return Err(StatusCode::BAD_REQUEST);
            }
            parent_id
        }
        _ => entry.parent_planned_entry_id.clone(),
    };

    Ok(ParsedPaymentPayload {
        paid_at,
        amount: payload.amount,
        account_id,
        project_id,
        parent_planned_entry_id,
        notes: clean_opt(payload.notes),
    })
}

fn parse_optional_object_id(value: Option<String>) -> Result<Option<ObjectId>, StatusCode> {
    match clean_opt(value) {
        Some(value) => ObjectId::from_str(&value)
            .map(Some)
            .map_err(|_| StatusCode::BAD_REQUEST),
        None => Ok(None),
    }
}

async fn load_payable_entries(
    state: &AppState,
    company_id: &ObjectId,
    entry_ids: &[ObjectId],
) -> Result<Vec<crate::models::PlannedEntry>, StatusCode> {
    let mut entries = Vec::new();
    for entry_id in entry_ids {
        let entry = get_planned_entry_by_id(state, entry_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        ensure_same_company(&entry.company_id, company_id)?;
        match entry.status {
            crate::models::PlannedStatus::Covered | crate::models::PlannedStatus::Cancelled => {
                return Err(StatusCode::BAD_REQUEST);
            }
            _ => entries.push(entry),
        }
    }
    Ok(entries)
}

async fn project_options(
    state: &AppState,
    company_id: &ObjectId,
    selected: Option<&ObjectId>,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let mut projects = list_projects(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    projects.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(projects
        .into_iter()
        .filter_map(|project| {
            let id = project.id?;
            Some(SimpleOption {
                value: id.to_hex(),
                label: project.title,
                selected: selected == Some(&id),
            })
        })
        .collect())
}

async fn parse_optional_project_id(
    state: &AppState,
    company_id: &ObjectId,
    value: Option<&str>,
) -> Result<Option<ObjectId>, StatusCode> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let project_id = ObjectId::from_str(value).map_err(|_| StatusCode::BAD_REQUEST)?;
    match get_project_by_id_for_company(state, &project_id, company_id).await {
        Ok(Some(_)) => Ok(Some(project_id)),
        Ok(None) => Err(StatusCode::BAD_REQUEST),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn parse_optional_parent_entry_id(
    state: &AppState,
    company_id: &ObjectId,
    value: Option<&str>,
    project_id: Option<&ObjectId>,
) -> Result<Option<ObjectId>, StatusCode> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parent_id = ObjectId::from_str(value).map_err(|_| StatusCode::BAD_REQUEST)?;
    let parent = get_planned_entry_by_id(state, &parent_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;
    if parent.company_id != *company_id || parent.cfdi_uuid.is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let (Some(parent_project), Some(project_id)) = (parent.project_id.as_ref(), project_id) {
        if parent_project != project_id {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(Some(parent_id))
}

async fn parent_entry_options(
    state: &AppState,
    company_id: &ObjectId,
    current_entry_id: Option<&ObjectId>,
    selected: Option<&ObjectId>,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let mut entries = list_planned_entries(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries
        .into_iter()
        .filter(|entry| entry.company_id == *company_id)
        .filter(|entry| entry.project_id.is_some())
        .filter(|entry| entry.cfdi_uuid.is_none())
        .filter_map(|entry| {
            let id = entry.id?;
            if current_entry_id == Some(&id) {
                return None;
            }
            Some(SimpleOption {
                value: id.to_hex(),
                label: format!("{} (${:.2})", entry.name, entry.amount_estimated),
                selected: selected == Some(&id),
            })
        })
        .collect())
}

fn safe_return_to(value: Option<&str>) -> Option<&str> {
    match value {
        Some("/tiempo") => Some("/tiempo"),
        Some("/admin/planned_entries") => Some("/admin/planned_entries"),
        _ => None,
    }
}

fn planned_entry_data(entry: PlannedEntry, company: String) -> Option<PlannedEntryData> {
    let id = entry.id?.to_hex();
    Some(PlannedEntryData {
        id,
        company_id: entry.company_id.to_hex(),
        company,
        recurring_plan_id: entry.recurring_plan_id.map(|id| id.to_hex()),
        recurring_plan_version: entry.recurring_plan_version,
        service_order_id: entry.service_order_id.map(|id| id.to_hex()),
        project_id: entry.project_id.map(|id| id.to_hex()),
        parent_planned_entry_id: entry.parent_planned_entry_id.map(|id| id.to_hex()),
        name: entry.name,
        flow_type: flow_type_value(&entry.flow_type).to_string(),
        category_id: entry.category_id.to_hex(),
        account_expected_id: entry.account_expected_id.to_hex(),
        contact_id: entry.contact_id.map(|id| id.to_hex()),
        amount_estimated: entry.amount_estimated,
        original_amount_estimated: entry.original_amount_estimated,
        due_date: datetime_to_string(&entry.due_date),
        original_due_date: entry
            .original_due_date
            .map(|date| datetime_to_string(&date)),
        status: planned_status_value(&entry.status).to_string(),
        status_label: planned_status_label(&entry.status).to_string(),
        notes: entry.notes,
        cfdi_uuid: entry.cfdi_uuid,
        currency: entry.currency,
        cfdi_folio: entry.cfdi_folio,
    })
}
