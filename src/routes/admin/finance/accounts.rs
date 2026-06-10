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
    session::SessionUser,
    state::{
        AppState, create_account, delete_account, get_account_by_id, list_accounts, update_account,
    },
};

use super::helpers::*;

#[derive(Template)]
#[template(path = "admin/accounts/index.html")]
struct AccountsIndexTemplate {
    accounts: Vec<AccountRow>,
}

#[derive(Serialize)]
pub struct AccountRow {
    pub id: String,
    pub name: String,
    pub company: String,
    pub account_type: String,
    pub currency: String,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct AccountDetail {
    pub id: String,
    pub name: String,
    pub company_id: String,
    pub company: String,
    pub account_type: String,
    pub currency: String,
    pub is_active: bool,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountCreatePayload {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    #[serde(default = "default_true_payload")]
    pub is_active: bool,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountUpdatePayload {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    #[serde(default = "default_true_payload")]
    pub is_active: bool,
    pub notes: Option<String>,
}

fn default_true_payload() -> bool {
    true
}

pub async fn accounts_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AccountRow>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let active_name = session_user.user().company_name.clone();
    let accounts = list_accounts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = accounts
        .into_iter()
        .filter(|acc| acc.company_id == active_company)
        .filter_map(|acc| {
            acc.id.map(|id| AccountRow {
                id: id.to_hex(),
                name: acc.name,
                company: active_name.clone(),
                account_type: account_type_value(&acc.account_type).to_string(),
                currency: acc.currency,
                is_active: acc.is_active,
            })
        })
        .collect();

    Ok(Json(rows))
}

pub async fn accounts_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AccountCreatePayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let account_type = match parse_account_type(&payload.account_type) {
        Ok(value) => value,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": message })),
            )
                .into_response();
        }
    };
    let name = payload.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "name is required" })),
        )
            .into_response();
    }
    let currency = payload
        .currency
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("MXN")
        .to_string();

    match create_account(
        &state,
        &company_id,
        name,
        account_type,
        &currency,
        payload.is_active,
        clean_opt(payload.notes),
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

pub async fn account_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AccountDetail>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let account = get_account_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&account.company_id, &active_company)?;

    Ok(Json(AccountDetail {
        id,
        name: account.name,
        company_id: account.company_id.to_hex(),
        company: session_user.user().company_name.clone(),
        account_type: account_type_value(&account.account_type).to_string(),
        currency: account.currency,
        is_active: account.is_active,
        notes: account.notes,
    }))
}

pub async fn account_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<AccountUpdatePayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_account_by_id(&state, &object_id).await {
        Ok(Some(account)) => {
            if let Err(status) = ensure_same_company(&account.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let account_type = match parse_account_type(&payload.account_type) {
        Ok(value) => value,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": message })),
            )
                .into_response();
        }
    };
    let name = payload.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "name is required" })),
        )
            .into_response();
    }
    let currency = payload
        .currency
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("MXN")
        .to_string();

    match update_account(
        &state,
        &object_id,
        &company_id,
        name,
        account_type,
        &currency,
        payload.is_active,
        clean_opt(payload.notes),
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn account_delete_api(
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
    match get_account_by_id(&state, &object_id).await {
        Ok(Some(account)) => {
            if let Err(status) = ensure_same_company(&account.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match delete_account(&state, &object_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Template)]
#[template(path = "admin/accounts/form.html")]
struct AccountFormTemplate {
    action: String,
    name: String,
    currency: String,
    account_type: String,
    is_active: bool,
    notes: String,
    companies: Vec<SimpleOption>,
    account_type_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountFormData {
    name: String,
    company_id: String,
    account_type: String,
    currency: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn accounts_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    let accounts = list_accounts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_company = session_user.active_company_id().clone();
    let active_name = session_user.user().company_name.clone();

    let rows = accounts
        .into_iter()
        .filter(|acc| acc.company_id == active_company)
        .filter_map(|acc| {
            acc.id.map(|id| AccountRow {
                id: id.to_hex(),
                name: acc.name,
                company: active_name.clone(),
                account_type: account_type_value(&acc.account_type).to_string(),
                currency: acc.currency,
                is_active: acc.is_active,
            })
        })
        .collect();

    render(AccountsIndexTemplate { accounts: rows })
}

pub async fn accounts_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let companies = company_options(&state, &active_company).await?;

    render(AccountFormTemplate {
        action: "/admin/accounts".into(),
        name: String::new(),
        currency: "MXN".into(),
        account_type: "bank".into(),
        is_active: true,
        notes: String::new(),
        companies,
        account_type_options: account_type_options("bank"),
        is_edit: false,
        errors: None,
    })
}

pub async fn accounts_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<AccountFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id)
        .await
        .unwrap_or_default();

    let account_type = match parse_account_type(&form.account_type) {
        Ok(t) => t,
        Err(msg) => {
            return render(AccountFormTemplate {
                action: "/admin/accounts".into(),
                name: form.name.clone(),
                currency: form.currency.clone(),
                account_type: form.account_type.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                account_type_options: account_type_options(&form.account_type),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let currency = if form.currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        form.currency.trim().to_string()
    };

    let notes = clean_opt(form.notes);

    match create_account(
        &state,
        &company_id,
        form.name.trim(),
        account_type,
        &currency,
        form.is_active,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/accounts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn accounts_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let account = get_account_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&account.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;

    render(AccountFormTemplate {
        action: format!("/admin/accounts/{}/update", id),
        name: account.name,
        currency: account.currency,
        account_type: account_type_value(&account.account_type).to_string(),
        is_active: account.is_active,
        notes: account.notes.unwrap_or_default(),
        companies,
        account_type_options: account_type_options(account_type_value(&account.account_type)),
        is_edit: true,
        errors: None,
    })
}

pub async fn accounts_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<AccountFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_account_by_id(&state, &object_id).await {
        Ok(Some(acc)) => {
            if let Err(status) = ensure_same_company(&acc.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let account_type = match parse_account_type(&form.account_type) {
        Ok(t) => t,
        Err(msg) => {
            let companies = company_options(&state, session_user.active_company_id())
                .await
                .unwrap_or_default();
            return render(AccountFormTemplate {
                action: format!("/admin/accounts/{}/update", id),
                name: form.name.clone(),
                currency: form.currency.clone(),
                account_type: form.account_type.clone(),
                is_active: form.is_active,
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                account_type_options: account_type_options(&form.account_type),
                is_edit: true,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let currency = if form.currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        form.currency.trim().to_string()
    };

    let notes = clean_opt(form.notes);

    match update_account(
        &state,
        &object_id,
        &company_id,
        form.name.trim(),
        account_type,
        &currency,
        form.is_active,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/accounts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn accounts_delete(
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

    match get_account_by_id(&state, &object_id).await {
        Ok(Some(acc)) => {
            if let Err(status) = ensure_same_company(&acc.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match delete_account(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/accounts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
