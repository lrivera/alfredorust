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
    models::Forecast,
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

#[derive(Serialize)]
pub struct ForecastRow {
    pub id: String,
    pub company: String,
    pub currency: String,
    pub projected_net: f64,
    pub start_date: String,
    pub end_date: String,
    pub scenario_name: Option<String>,
}

#[derive(Serialize)]
pub struct ForecastDetail {
    pub id: String,
    pub company_id: String,
    pub company: String,
    pub generated_at: String,
    pub generated_by_user_id: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub currency: String,
    pub projected_income_total: f64,
    pub projected_expense_total: f64,
    pub projected_net: f64,
    pub initial_balance: Option<f64>,
    pub final_balance: Option<f64>,
    pub details: Option<String>,
    pub scenario_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ForecastPayload {
    pub generated_at: String,
    pub generated_by_user_id: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub currency: String,
    pub projected_income_total: f64,
    pub projected_expense_total: f64,
    pub projected_net: f64,
    pub initial_balance: Option<f64>,
    pub final_balance: Option<f64>,
    pub details: Option<String>,
    pub scenario_name: Option<String>,
    pub notes: Option<String>,
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
                start_date: datetime_to_string(&f.start_date),
                end_date: datetime_to_string(&f.end_date),
                scenario_name: f.scenario_name,
            })
        })
        .collect();

    render(ForecastsIndexTemplate { forecasts: rows })
}

#[utoipa::path(
    get,
    path = "/api/admin/forecasts",
    tag = "finance",
    responses(
        (status = 200, description = "List forecasts"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn forecasts_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ForecastRow>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let active_name = session_user.user().company_name.clone();
    let forecasts = list_forecasts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = forecasts
        .into_iter()
        .filter(|forecast| forecast.company_id == active_company)
        .filter_map(|forecast| {
            forecast.id.map(|id| ForecastRow {
                id: id.to_hex(),
                company: active_name.clone(),
                currency: forecast.currency,
                projected_net: forecast.projected_net,
                start_date: datetime_to_string(&forecast.start_date),
                end_date: datetime_to_string(&forecast.end_date),
                scenario_name: forecast.scenario_name,
            })
        })
        .collect();

    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/api/admin/forecasts/{id}",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Forecast detail"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn forecast_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ForecastDetail>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let forecast = get_forecast_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&forecast.company_id, &active_company)?;

    Ok(Json(forecast_detail(
        id,
        forecast,
        session_user.user().company_name.clone(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/admin/forecasts",
    tag = "finance",
    request_body = ForecastPayload,
    responses(
        (status = 201, description = "Forecast created"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn forecasts_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ForecastPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_forecast_payload(&state, &company_id, payload).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    match create_forecast(
        &state,
        &company_id,
        parsed.generated_at,
        parsed.generated_by_user_id,
        parsed.start_date,
        parsed.end_date,
        &parsed.currency,
        parsed.projected_income_total,
        parsed.projected_expense_total,
        parsed.projected_net,
        parsed.initial_balance,
        parsed.final_balance,
        parsed.details,
        parsed.scenario_name,
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

#[utoipa::path(
    post,
    path = "/api/admin/forecasts/{id}/update",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    request_body = ForecastPayload,
    responses(
        (status = 200, description = "Forecast updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn forecast_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ForecastPayload>,
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
    let parsed = match parse_forecast_payload(&state, &company_id, payload).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    match update_forecast(
        &state,
        &object_id,
        &company_id,
        parsed.generated_at,
        parsed.generated_by_user_id,
        parsed.start_date,
        parsed.end_date,
        &parsed.currency,
        parsed.projected_income_total,
        parsed.projected_expense_total,
        parsed.projected_net,
        parsed.initial_balance,
        parsed.final_balance,
        parsed.details,
        parsed.scenario_name,
        parsed.notes,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/forecasts/{id}/delete",
    tag = "finance",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Forecast deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn forecast_delete_api(
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
    if let Err(status) = match get_forecast_by_id(&state, &object_id).await {
        Ok(Some(forecast)) => ensure_same_company(&forecast.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_forecast(&state, &object_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

struct ParsedForecastPayload {
    generated_at: mongodb::bson::DateTime,
    generated_by_user_id: Option<ObjectId>,
    start_date: mongodb::bson::DateTime,
    end_date: mongodb::bson::DateTime,
    currency: String,
    projected_income_total: f64,
    projected_expense_total: f64,
    projected_net: f64,
    initial_balance: Option<f64>,
    final_balance: Option<f64>,
    details: Option<String>,
    scenario_name: Option<String>,
    notes: Option<String>,
}

async fn parse_forecast_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: ForecastPayload,
) -> Result<ParsedForecastPayload, axum::response::Response> {
    let generated_at = parse_datetime_field(&payload.generated_at, "generated_at")
        .map_err(|message| json_bad_request(&message))?;
    let start_date = parse_datetime_field(&payload.start_date, "start_date")
        .map_err(|message| json_bad_request(&message))?;
    let end_date = parse_datetime_field(&payload.end_date, "end_date")
        .map_err(|message| json_bad_request(&message))?;
    let currency = payload.currency.trim().to_string();
    if currency.is_empty() {
        return Err(json_bad_request("currency is required"));
    }
    let generated_by_user_id = match clean_opt(payload.generated_by_user_id) {
        Some(value) => match ObjectId::from_str(&value) {
            Ok(id) => {
                if let Err(status) = validate_user_in_company(state, &id, company_id).await {
                    return Err(status.into_response());
                }
                Some(id)
            }
            Err(_) => return Err(json_bad_request("generated_by_user_id is invalid")),
        },
        None => None,
    };

    Ok(ParsedForecastPayload {
        generated_at,
        generated_by_user_id,
        start_date,
        end_date,
        currency,
        projected_income_total: payload.projected_income_total,
        projected_expense_total: payload.projected_expense_total,
        projected_net: payload.projected_net,
        initial_balance: payload.initial_balance,
        final_balance: payload.final_balance,
        details: clean_opt(payload.details),
        scenario_name: clean_opt(payload.scenario_name),
        notes: clean_opt(payload.notes),
    })
}

fn forecast_detail(id: String, forecast: Forecast, company: String) -> ForecastDetail {
    ForecastDetail {
        id,
        company_id: forecast.company_id.to_hex(),
        company,
        generated_at: datetime_to_string(&forecast.generated_at),
        generated_by_user_id: forecast.generated_by_user_id.map(|id| id.to_hex()),
        start_date: datetime_to_string(&forecast.start_date),
        end_date: datetime_to_string(&forecast.end_date),
        currency: forecast.currency,
        projected_income_total: forecast.projected_income_total,
        projected_expense_total: forecast.projected_expense_total,
        projected_net: forecast.projected_net,
        initial_balance: forecast.initial_balance,
        final_balance: forecast.final_balance,
        details: forecast.details,
        scenario_name: forecast.scenario_name,
        notes: forecast.notes,
    }
}

fn json_bad_request(message: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
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
    let users = user_options(
        &state,
        forecast.generated_by_user_id.as_ref(),
        &active_company,
    )
    .await?;

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
