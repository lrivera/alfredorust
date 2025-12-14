use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    session::SessionUser,
    state::{
        AppState, create_planned_entry, delete_planned_entry, get_planned_entry_by_id,
        list_planned_entries, update_planned_entry,
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
    status: String,
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
                status: planned_status_value(&e.status).to_string(),
            })
        })
        .collect();

    render(PlannedEntriesIndexTemplate { entries: rows })
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
        if let Err(status) =
            validate_recurring_plan_company(&state, plan_id, &company_id).await
        {
            return status.into_response();
        }
    }

    match create_planned_entry(
        &state,
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
        status,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/planned_entries").into_response(),
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
    let categories =
        category_options(&state, Some(&entry.category_id), &active_company).await?;
    let accounts =
        account_options(&state, Some(&entry.account_expected_id), &active_company).await?;
    let contacts =
        contact_options(&state, entry.contact_id.as_ref(), &active_company).await?;
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
        Ok(_) => Redirect::to("/admin/planned_entries").into_response(),
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
