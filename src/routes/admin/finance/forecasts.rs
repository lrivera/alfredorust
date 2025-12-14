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
        AppState, create_forecast, delete_forecast, get_forecast_by_id, list_forecasts,
        update_forecast,
    },
};

use super::helpers::*;
use super::options::user_options;

#[derive(Template)]
#[template(path = "admin/forecasts/index.html")]
struct ForecastsIndexTemplate {
    forecasts: Vec<ForecastRow>,
}

struct ForecastRow {
    id: String,
    company: String,
    currency: String,
    projected_net: f64,
}

#[derive(Template)]
#[template(path = "admin/forecasts/form.html")]
struct ForecastFormTemplate {
    action: String,
    currency: String,
    projected_income_total: String,
    projected_expense_total: String,
    projected_net: String,
    initial_balance: String,
    final_balance: String,
    generated_at: String,
    start_date: String,
    end_date: String,
    generated_by_user_id: String,
    details: String,
    scenario_name: String,
    notes: String,
    companies: Vec<SimpleOption>,
    users: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct ForecastFormData {
    company_id: String,
    currency: String,
    projected_income_total: String,
    projected_expense_total: String,
    projected_net: String,
    #[serde(default)]
    initial_balance: Option<String>,
    #[serde(default)]
    final_balance: Option<String>,
    generated_at: String,
    start_date: String,
    end_date: String,
    #[serde(default)]
    generated_by_user_id: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    scenario_name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn forecasts_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let forecasts = list_forecasts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_name = session_user.user().company_name.clone();

    let rows = forecasts
        .into_iter()
        .filter(|f| f.company_id == active_company)
        .filter_map(|f| {
            f.id.map(|id| ForecastRow {
                id: id.to_hex(),
                company: active_name.clone(),
                currency: f.currency,
                projected_net: f.projected_net,
            })
        })
        .collect();

    render(ForecastsIndexTemplate { forecasts: rows })
}

pub async fn forecasts_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let users = user_options(&state, None, &active_company).await?;

    render(ForecastFormTemplate {
        action: "/admin/forecasts".into(),
        currency: "MXN".into(),
        projected_income_total: "0".into(),
        projected_expense_total: "0".into(),
        projected_net: "0".into(),
        initial_balance: String::new(),
        final_balance: String::new(),
        generated_at: String::new(),
        start_date: String::new(),
        end_date: String::new(),
        generated_by_user_id: String::new(),
        details: String::new(),
        scenario_name: String::new(),
        notes: String::new(),
        companies,
        users,
        is_edit: false,
        errors: None,
    })
}

pub async fn forecasts_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ForecastFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let projected_income_total =
        match parse_f64_field(&form.projected_income_total, "Ingreso proyectado") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let projected_expense_total =
        match parse_f64_field(&form.projected_expense_total, "Gasto proyectado") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let projected_net = match parse_f64_field(&form.projected_net, "Neto proyectado") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let initial_balance =
        match parse_optional_f64_field(form.initial_balance.clone(), "Saldo inicial") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let final_balance = match parse_optional_f64_field(form.final_balance.clone(), "Saldo final") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let generated_at = match parse_datetime_field(&form.generated_at, "Fecha de generación") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let start_date = match parse_datetime_field(&form.start_date, "Fecha inicio") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let end_date = match parse_datetime_field(&form.end_date, "Fecha fin") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let generated_by_user_id = match form
        .generated_by_user_id
        .clone()
        .and_then(|v| if v.trim().is_empty() { None } else { Some(v) })
    {
        Some(val) => match parse_object_id(&val, "Usuario") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let details = clean_opt(form.details);
    let scenario_name = clean_opt(form.scenario_name);
    let notes = clean_opt(form.notes);

    if let Some(ref uid) = generated_by_user_id {
        if let Err(status) = validate_user_in_company(&state, uid, &company_id).await {
            return status.into_response();
        }
    }

    match create_forecast(
        &state,
        &company_id,
        generated_at,
        generated_by_user_id,
        start_date,
        end_date,
        form.currency.trim(),
        projected_income_total,
        projected_expense_total,
        projected_net,
        initial_balance,
        final_balance,
        details,
        scenario_name,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/forecasts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn forecasts_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let forecast = get_forecast_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&forecast.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let users =
        user_options(&state, forecast.generated_by_user_id.as_ref(), &active_company).await?;

    render(ForecastFormTemplate {
        action: format!("/admin/forecasts/{}/update", id),
        currency: forecast.currency,
        projected_income_total: forecast.projected_income_total.to_string(),
        projected_expense_total: forecast.projected_expense_total.to_string(),
        projected_net: forecast.projected_net.to_string(),
        initial_balance: forecast
            .initial_balance
            .map(|v| v.to_string())
            .unwrap_or_default(),
        final_balance: forecast
            .final_balance
            .map(|v| v.to_string())
            .unwrap_or_default(),
        generated_at: datetime_to_string(&forecast.generated_at),
        start_date: datetime_to_string(&forecast.start_date),
        end_date: datetime_to_string(&forecast.end_date),
        generated_by_user_id: forecast
            .generated_by_user_id
            .map(|id| id.to_hex())
            .unwrap_or_default(),
        details: forecast.details.unwrap_or_default(),
        scenario_name: forecast.scenario_name.unwrap_or_default(),
        notes: forecast.notes.unwrap_or_default(),
        companies,
        users,
        is_edit: true,
        errors: None,
    })
}

pub async fn forecasts_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ForecastFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = match get_forecast_by_id(&state, &object_id).await {
        Ok(Some(forecast)) => ensure_same_company(&forecast.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    let projected_income_total =
        match parse_f64_field(&form.projected_income_total, "Ingreso proyectado") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let projected_expense_total =
        match parse_f64_field(&form.projected_expense_total, "Gasto proyectado") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let projected_net = match parse_f64_field(&form.projected_net, "Neto proyectado") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let initial_balance =
        match parse_optional_f64_field(form.initial_balance.clone(), "Saldo inicial") {
            Ok(v) => v,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    let final_balance = match parse_optional_f64_field(form.final_balance.clone(), "Saldo final") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let generated_at = match parse_datetime_field(&form.generated_at, "Fecha de generación") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let start_date = match parse_datetime_field(&form.start_date, "Fecha inicio") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let end_date = match parse_datetime_field(&form.end_date, "Fecha fin") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let generated_by_user_id = match form
        .generated_by_user_id
        .clone()
        .and_then(|v| if v.trim().is_empty() { None } else { Some(v) })
    {
        Some(val) => match parse_object_id(&val, "Usuario") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let details = clean_opt(form.details);
    let scenario_name = clean_opt(form.scenario_name);
    let notes = clean_opt(form.notes);

    if let Some(ref uid) = generated_by_user_id {
        if let Err(status) = validate_user_in_company(&state, uid, &company_id).await {
            return status.into_response();
        }
    }

    match update_forecast(
        &state,
        &object_id,
        &company_id,
        generated_at,
        generated_by_user_id,
        start_date,
        end_date,
        form.currency.trim(),
        projected_income_total,
        projected_expense_total,
        projected_net,
        initial_balance,
        final_balance,
        details,
        scenario_name,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/forecasts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn forecasts_delete(
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

    if let Err(status) = match get_forecast_by_id(&state, &object_id).await {
        Ok(Some(forecast)) => ensure_same_company(&forecast.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_forecast(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/forecasts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
