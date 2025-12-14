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

    let companies = company_options(&state, &company_id).await.unwrap_or_default();
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
    let accounts =
        account_options(&state, Some(&plan.account_expected_id), session_user.active_company_id())
            .await?;
    let contacts =
        contact_options(&state, plan.contact_id.as_ref(), session_user.active_company_id()).await?;

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
