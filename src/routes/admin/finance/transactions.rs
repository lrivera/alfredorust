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
        AppState, create_transaction, delete_transaction, get_transaction_by_id, list_transactions,
        update_transaction,
    },
};

use super::helpers::*;
use super::options::{account_options, category_options, planned_entry_options};

#[derive(Template)]
#[template(path = "admin/transactions/index.html")]
struct TransactionsIndexTemplate {
    transactions: Vec<TransactionRow>,
}

struct TransactionRow {
    id: String,
    description: String,
    company: String,
    amount: f64,
    transaction_type: String,
}

#[derive(Template)]
#[template(path = "admin/transactions/form.html")]
struct TransactionFormTemplate {
    action: String,
    description: String,
    amount: String,
    transaction_type: String,
    date: String,
    notes: String,
    is_confirmed: bool,
    companies: Vec<SimpleOption>,
    categories: Vec<SimpleOption>,
    accounts: Vec<SimpleOption>,
    planned_entries: Vec<SimpleOption>,
    transaction_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct TransactionFormData {
    #[serde(default)]
    company_id: String,
    date: String,
    description: String,
    transaction_type: String,
    category_id: String,
    #[serde(default)]
    account_from_id: Option<String>,
    #[serde(default)]
    account_to_id: Option<String>,
    amount: String,
    #[serde(default)]
    planned_entry_id: Option<String>,
    #[serde(default)]
    is_confirmed: bool,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn transactions_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let transactions = list_transactions(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_name = session_user.user().company_name.clone();

    let rows = transactions
        .into_iter()
        .filter(|t| t.company_id == active_company)
        .filter_map(|t| {
            t.id.map(|id| TransactionRow {
                id: id.to_hex(),
                description: t.description,
                company: active_name.clone(),
                amount: t.amount,
                transaction_type: transaction_type_value(&t.transaction_type).to_string(),
            })
        })
        .collect();

    render(TransactionsIndexTemplate { transactions: rows })
}

pub async fn transactions_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let categories = category_options(&state, None, &active_company).await?;
    let accounts = account_options(&state, None, &active_company).await?;
    let planned_entries = planned_entry_options(&state, None, &active_company).await?;

    render(TransactionFormTemplate {
        action: "/admin/transactions".into(),
        description: String::new(),
        amount: "0".into(),
        transaction_type: "expense".into(),
        date: String::new(),
        notes: String::new(),
        is_confirmed: true,
        companies,
        categories,
        accounts,
        planned_entries,
        transaction_options: transaction_type_options("expense"),
        is_edit: false,
        errors: None,
    })
}

pub async fn transactions_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<TransactionFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let transaction_type = match parse_transaction_type(&form.transaction_type) {
        Ok(t) => t,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let category_id = match parse_object_id(&form.category_id, "Categoría") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let account_from_id = match form.account_from_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Cuenta origen") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let account_to_id = match form.account_to_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Cuenta destino") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let planned_entry_id = match form.planned_entry_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Compromiso planificado") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let amount = match parse_f64_field(&form.amount, "Monto") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let date = match parse_datetime_field(&form.date, "Fecha") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let notes = clean_opt(form.notes);

    if let Err(status) =
        validate_company_refs(&state, &company_id, Some(&category_id), account_from_id.as_ref(), None)
            .await
    {
        return status.into_response();
    }

    if let Some(ref account_to) = account_to_id {
        if let Err(status) =
            validate_company_refs(&state, &company_id, Some(&category_id), Some(account_to), None)
                .await
        {
            return status.into_response();
        }
    }

    if let Some(ref entry_id) = planned_entry_id {
        if let Err(status) = validate_planned_entry_company(&state, entry_id, &company_id).await {
            return status.into_response();
        }
    }

    match create_transaction(
        &state,
        &company_id,
        date,
        form.description.trim(),
        transaction_type,
        &category_id,
        account_from_id,
        account_to_id,
        amount,
        planned_entry_id,
        form.is_confirmed,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/transactions").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn transactions_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let transaction = get_transaction_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&transaction.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let categories =
        category_options(&state, Some(&transaction.category_id), &active_company).await?;
    let accounts = account_options(
        &state,
        transaction
            .account_from_id
            .as_ref()
            .or(transaction.account_to_id.as_ref()),
        &active_company,
    )
    .await?;
    let planned_entries = planned_entry_options(
        &state,
        transaction.planned_entry_id.as_ref(),
        &active_company,
    )
    .await?;

    render(TransactionFormTemplate {
        action: format!("/admin/transactions/{}/update", id),
        description: transaction.description,
        amount: transaction.amount.to_string(),
        transaction_type: transaction_type_value(&transaction.transaction_type).to_string(),
        date: datetime_to_string(&transaction.date),
        notes: transaction.notes.unwrap_or_default(),
        is_confirmed: transaction.is_confirmed,
        companies,
        categories,
        accounts,
        planned_entries,
        transaction_options: transaction_type_options(transaction_type_value(
            &transaction.transaction_type,
        )),
        is_edit: true,
        errors: None,
    })
}

pub async fn transactions_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<TransactionFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = match get_transaction_by_id(&state, &object_id).await {
        Ok(Some(tx)) => ensure_same_company(&tx.company_id, &company_id),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    let transaction_type = match parse_transaction_type(&form.transaction_type) {
        Ok(t) => t,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let category_id = match parse_object_id(&form.category_id, "Categoría") {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let account_from_id = match form.account_from_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Cuenta origen") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let account_to_id = match form.account_to_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Cuenta destino") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let planned_entry_id = match form.planned_entry_id.clone().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(val) => match parse_object_id(&val, "Compromiso planificado") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    let amount = match parse_f64_field(&form.amount, "Monto") {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let date = match parse_datetime_field(&form.date, "Fecha") {
        Ok(dt) => dt,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let notes = clean_opt(form.notes);

    if let Err(status) =
        validate_company_refs(&state, &company_id, Some(&category_id), account_from_id.as_ref(), None)
            .await
    {
        return status.into_response();
    }

    if let Some(ref account_to) = account_to_id {
        if let Err(status) =
            validate_company_refs(&state, &company_id, Some(&category_id), Some(account_to), None)
                .await
        {
            return status.into_response();
        }
    }

    if let Some(ref entry_id) = planned_entry_id {
        if let Err(status) = validate_planned_entry_company(&state, entry_id, &company_id).await {
            return status.into_response();
        }
    }

    match update_transaction(
        &state,
        &object_id,
        &company_id,
        date,
        form.description.trim(),
        transaction_type,
        &category_id,
        account_from_id,
        account_to_id,
        amount,
        planned_entry_id,
        form.is_confirmed,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/transactions").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn transactions_delete(
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

    if let Err(status) = match get_transaction_by_id(&state, &object_id).await {
        Ok(Some(tx)) => ensure_same_company(&tx.company_id, &active_company),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    } {
        return status.into_response();
    }

    match delete_transaction(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/transactions").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
