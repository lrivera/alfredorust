use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::RecurringPlan,
    session::SessionUser,
    state::{
        AppState, create_recurring_plan, delete_recurring_plan, get_recurring_plan_by_id,
        list_recurring_plans, regenerate_planned_entries_for_plan_id, update_recurring_plan,
    },
};

use super::helpers::*;
use super::options::{account_options, category_options, contact_options};

#[derive(Template)]
#[template(path = "admin/recurring_plans/index.html")]
struct RecurringPlansIndexTemplate {
    plans: Vec<RecurringPlanRow>,
}

struct RecurringPlanRow {
    id: String,
    name: String,
    company: String,
    flow_type: String,
    amount: f64,
    active: bool,
}

#[derive(Serialize)]
pub struct RecurringPlanData {
    pub id: String,
    pub company_id: String,
    pub company: String,
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub frequency: String,
    pub day_of_month: Option<i32>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
    pub version: i32,
    pub notes: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/recurring_plans/form.html")]
struct RecurringPlanFormTemplate {
    action: String,
    name: String,
    flow_type: String,
    amount_estimated: String,
    frequency: String,
    day_of_month: String,
    start_date: String,
    end_date: String,
    version: String,
    is_active: bool,
    notes: String,
    companies: Vec<SimpleOption>,
    flow_options: Vec<SimpleOption>,
    categories: Vec<SimpleOption>,
    accounts: Vec<SimpleOption>,
    contacts: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct RecurringPlanFormData {
    name: String,
    company_id: String,
    flow_type: String,
    category_id: String,
    account_expected_id: String,
    #[serde(default)]
    contact_id: Option<String>,
    amount_estimated: String,
    frequency: String,
    #[serde(default)]
    day_of_month: Option<String>,
    start_date: String,
    #[serde(default)]
    end_date: Option<String>,
    #[serde(default)]
    is_active: bool,
    version: String,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
pub struct RecurringPlanPayload {
    pub name: String,
    pub flow_type: String,
    pub category_id: String,
    pub account_expected_id: String,
    pub contact_id: Option<String>,
    pub amount_estimated: f64,
    pub frequency: String,
    pub day_of_month: Option<i32>,
    pub start_date: String,
    pub end_date: Option<String>,
    #[serde(default = "default_active")]
    pub is_active: bool,
    #[serde(default = "default_version")]
    pub version: i32,
    pub notes: Option<String>,
}

struct ParsedRecurringPlanPayload {
    name: String,
    flow_type: crate::models::FlowType,
    category_id: ObjectId,
    account_expected_id: ObjectId,
    contact_id: Option<ObjectId>,
    amount_estimated: f64,
    frequency: String,
    day_of_month: Option<i32>,
    start_date: mongodb::bson::DateTime,
    end_date: Option<mongodb::bson::DateTime>,
    is_active: bool,
    version: i32,
    notes: Option<String>,
}

fn default_active() -> bool {
    true
}

fn default_version() -> i32 {
    1
}

pub async fn recurring_plans_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let plans = list_recurring_plans(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|p| p.company_id == active_company)
        .collect::<Vec<_>>();
    let active_name = session_user.user().company_name.clone();

    let rows = plans
        .into_iter()
        .filter_map(|p| {
            p.id.map(|id| RecurringPlanRow {
                id: id.to_hex(),
                name: p.name,
                company: active_name.clone(),
                flow_type: flow_type_value(&p.flow_type).to_string(),
                amount: p.amount_estimated,
                active: p.is_active,
            })
        })
        .collect();

    render(RecurringPlansIndexTemplate { plans: rows })
}

pub async fn recurring_plans_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RecurringPlanData>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let active_name = session_user.user().company_name.clone();
    let plans = list_recurring_plans(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = plans
        .into_iter()
        .filter(|plan| plan.company_id == active_company)
        .filter_map(|plan| recurring_plan_data(plan, active_name.clone()))
        .collect();

    Ok(Json(rows))
}

pub async fn recurring_plan_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RecurringPlanData>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let plan = get_recurring_plan_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&plan.company_id, &active_company)?;

    recurring_plan_data(plan, session_user.user().company_name.clone())
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn recurring_plans_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RecurringPlanPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_recurring_plan_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    match create_recurring_plan(
        &state,
        &company_id,
        &parsed.name,
        parsed.flow_type,
        &parsed.category_id,
        &parsed.account_expected_id,
        parsed.contact_id,
        parsed.amount_estimated,
        &parsed.frequency,
        parsed.day_of_month,
        parsed.start_date,
        parsed.end_date,
        parsed.is_active,
        parsed.version,
        parsed.notes,
    )
    .await
    {
        Ok(id) => {
            let generated_count = count_plan_entries(&state, &id).await.unwrap_or(0);
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": id.to_hex(),
                    "side_effects": { "planned_entries_generated": generated_count }
                })),
            )
                .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plan_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<RecurringPlanPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let existing = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => {
            if let Err(status) = ensure_same_company(&plan.company_id, &company_id) {
                return status.into_response();
            }
            plan
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let parsed = match parse_recurring_plan_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    let before_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);

    match update_recurring_plan(
        &state,
        &object_id,
        &company_id,
        &parsed.name,
        parsed.flow_type,
        &parsed.category_id,
        &parsed.account_expected_id,
        parsed.contact_id,
        parsed.amount_estimated,
        &parsed.frequency,
        parsed.day_of_month,
        parsed.start_date,
        parsed.end_date,
        parsed.is_active,
        parsed.version,
        parsed.notes,
    )
    .await
    {
        Ok(_) => {
            let after_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);
            Json(serde_json::json!({
                "ok": true,
                "side_effects": {
                    "previous_version": existing.version,
                    "planned_entries_before": before_count,
                    "planned_entries_after": after_count
                }
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plan_delete_api(
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
    if let Err(status) = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }
    let before_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);

    match delete_recurring_plan(&state, &object_id).await {
        Ok(_) => {
            let after_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);
            Json(serde_json::json!({
                "ok": true,
                "side_effects": {
                    "planned_entries_before": before_count,
                    "planned_entries_after": after_count
                }
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plan_generate_api(
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
    if let Err(status) = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }
    let before_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);

    match regenerate_planned_entries_for_plan_id(&state, &object_id).await {
        Ok(_) => {
            let after_count = count_plan_entries(&state, &object_id).await.unwrap_or(0);
            Json(serde_json::json!({
                "ok": true,
                "side_effects": {
                    "planned_entries_before": before_count,
                    "planned_entries_after": after_count,
                    "planned_entries_generated": after_count.saturating_sub(before_count)
                }
            }))
            .into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn recurring_plans_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let categories = category_options(&state, None, &active_company).await?;
    let accounts = account_options(&state, None, &active_company).await?;
    let contacts = contact_options(&state, None, &active_company).await?;

    render(RecurringPlanFormTemplate {
        action: "/admin/recurring_plans".into(),
        name: String::new(),
        flow_type: "income".into(),
        amount_estimated: String::from("0"),
        frequency: "monthly".into(),
        day_of_month: String::new(),
        start_date: String::new(),
        end_date: String::new(),
        version: "1".into(),
        is_active: true,
        notes: String::new(),
        companies,
        flow_options: flow_options("income"),
        categories,
        accounts,
        contacts,
        is_edit: false,
        errors: None,
    })
}

pub async fn recurring_plans_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<RecurringPlanFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id)
        .await
        .unwrap_or_default();
    let categories = category_options(&state, None, &company_id)
        .await
        .unwrap_or_default();
    let accounts = account_options(&state, None, &company_id)
        .await
        .unwrap_or_default();
    let contacts = contact_options(&state, None, &company_id)
        .await
        .unwrap_or_default();

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let category_id = match parse_object_id(&form.category_id, "Categoría") {
        Ok(id) => id,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let account_expected_id = match parse_object_id(&form.account_expected_id, "Cuenta esperada") {
        Ok(id) => id,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let contact_id = match form.contact_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(cid) => Some(parse_object_id(&cid, "Contacto").map_err(|msg| {
            render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response())
        })),
        None => None,
    };

    let contact_id = match contact_id {
        Some(Ok(id)) => Some(id),
        Some(Err(resp)) => return resp,
        None => None,
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

    let amount_estimated = match parse_f64_field(&form.amount_estimated, "Monto estimado") {
        Ok(v) => v,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let day_of_month = match parse_optional_i32_field(form.day_of_month.clone(), "Día del mes") {
        Ok(v) => v,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let start_date = match parse_datetime_field(&form.start_date, "Fecha de inicio") {
        Ok(dt) => dt,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let end_date = match parse_optional_datetime_field(form.end_date.clone(), "Fecha fin") {
        Ok(dt) => dt,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies: companies.clone(),
                flow_options: flow_options(&form.flow_type),
                categories: categories.clone(),
                accounts: accounts.clone(),
                contacts: contacts.clone(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let version = match parse_i32_field(&form.version, "Versión") {
        Ok(v) => v,
        Err(msg) => {
            return render(RecurringPlanFormTemplate {
                action: "/admin/recurring_plans".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                amount_estimated: form.amount_estimated.clone(),
                frequency: form.frequency.clone(),
                day_of_month: form.day_of_month.clone().unwrap_or_default(),
                start_date: form.start_date.clone(),
                end_date: form.end_date.clone().unwrap_or_default(),
                version: form.version.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                flow_options: flow_options(&form.flow_type),
                categories,
                accounts,
                contacts,
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let notes = clean_opt(form.notes);

    match create_recurring_plan(
        &state,
        &company_id,
        form.name.trim(),
        flow_type,
        &category_id,
        &account_expected_id,
        contact_id,
        amount_estimated,
        form.frequency.trim(),
        day_of_month,
        start_date,
        end_date,
        form.is_active,
        version,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/recurring_plans").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plans_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let plan = get_recurring_plan_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&plan.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let categories = category_options(&state, Some(&plan.category_id), &active_company).await?;
    let accounts = account_options(
        &state,
        Some(&plan.account_expected_id),
        session_user.active_company_id(),
    )
    .await?;
    let contacts = contact_options(
        &state,
        plan.contact_id.as_ref(),
        session_user.active_company_id(),
    )
    .await?;

    render(RecurringPlanFormTemplate {
        action: format!("/admin/recurring_plans/{}/update", id),
        name: plan.name,
        flow_type: flow_type_value(&plan.flow_type).to_string(),
        amount_estimated: plan.amount_estimated.to_string(),
        frequency: plan.frequency,
        day_of_month: plan.day_of_month.map(|d| d.to_string()).unwrap_or_default(),
        start_date: datetime_to_string(&plan.start_date),
        end_date: plan
            .end_date
            .map(|d| datetime_to_string(&d))
            .unwrap_or_default(),
        version: plan.version.to_string(),
        is_active: plan.is_active,
        notes: plan.notes.unwrap_or_default(),
        companies,
        flow_options: flow_options(flow_type_value(&plan.flow_type)),
        categories,
        accounts,
        contacts,
        is_edit: true,
        errors: None,
    })
}

pub async fn recurring_plans_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<RecurringPlanFormData>,
) -> impl IntoResponse {
    let active_company = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

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

    let amount_estimated = match parse_f64_field(&form.amount_estimated, "Monto estimado") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let day_of_month = match parse_optional_i32_field(form.day_of_month.clone(), "Día del mes") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let start_date = match parse_datetime_field(&form.start_date, "Fecha de inicio") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let end_date = match parse_optional_datetime_field(form.end_date.clone(), "Fecha fin") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let version = match parse_i32_field(&form.version, "Versión") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let notes = clean_opt(form.notes);

    if let Err(status) = validate_company_refs(
        &state,
        &active_company,
        Some(&category_id),
        Some(&account_expected_id),
        contact_id.as_ref(),
    )
    .await
    {
        return status.into_response();
    }

    match update_recurring_plan(
        &state,
        &object_id,
        &active_company,
        form.name.trim(),
        flow_type,
        &category_id,
        &account_expected_id,
        contact_id,
        amount_estimated,
        form.frequency.trim(),
        day_of_month,
        start_date,
        end_date,
        form.is_active,
        version,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/recurring_plans").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plans_delete(
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

    if let Err(status) = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_recurring_plan(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/recurring_plans").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recurring_plans_generate(
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

    if let Err(status) = match get_recurring_plan_by_id(&state, &object_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match regenerate_planned_entries_for_plan_id(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/recurring_plans").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn parse_recurring_plan_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: RecurringPlanPayload,
) -> Result<ParsedRecurringPlanPayload, StatusCode> {
    let name = payload.name.trim().to_string();
    if name.is_empty() || payload.amount_estimated < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let frequency = payload.frequency.trim().to_lowercase();
    if !matches!(frequency.as_str(), "monthly" | "weekly") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(day) = payload.day_of_month {
        if !(1..=31).contains(&day) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let flow_type = parse_flow_type(&payload.flow_type).map_err(|_| StatusCode::BAD_REQUEST)?;
    let category_id = parse_object_id(&payload.category_id, "category_id")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let account_expected_id = parse_object_id(&payload.account_expected_id, "account_expected_id")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let contact_id = parse_optional_object_id(payload.contact_id)?;
    let start_date = parse_datetime_field(&payload.start_date, "start_date")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let end_date = parse_optional_datetime_field(payload.end_date, "end_date")
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if let Some(end_date) = end_date.as_ref() {
        if end_date < &start_date {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    validate_company_refs(
        state,
        company_id,
        Some(&category_id),
        Some(&account_expected_id),
        contact_id.as_ref(),
    )
    .await?;

    Ok(ParsedRecurringPlanPayload {
        name,
        flow_type,
        category_id,
        account_expected_id,
        contact_id,
        amount_estimated: payload.amount_estimated,
        frequency,
        day_of_month: payload.day_of_month,
        start_date,
        end_date,
        is_active: payload.is_active,
        version: payload.version,
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

async fn count_plan_entries(state: &AppState, plan_id: &ObjectId) -> Result<usize, StatusCode> {
    let entries = crate::state::list_planned_entries(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry.recurring_plan_id.as_ref() == Some(plan_id))
        .count())
}

fn recurring_plan_data(plan: RecurringPlan, company: String) -> Option<RecurringPlanData> {
    let id = plan.id?.to_hex();
    Some(RecurringPlanData {
        id,
        company_id: plan.company_id.to_hex(),
        company,
        name: plan.name,
        flow_type: flow_type_value(&plan.flow_type).to_string(),
        category_id: plan.category_id.to_hex(),
        account_expected_id: plan.account_expected_id.to_hex(),
        contact_id: plan.contact_id.map(|id| id.to_hex()),
        amount_estimated: plan.amount_estimated,
        frequency: plan.frequency,
        day_of_month: plan.day_of_month,
        start_date: datetime_to_string(&plan.start_date),
        end_date: plan.end_date.map(|date| datetime_to_string(&date)),
        is_active: plan.is_active,
        version: plan.version,
        notes: plan.notes,
    })
}
