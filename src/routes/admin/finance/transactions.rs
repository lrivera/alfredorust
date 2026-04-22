use std::{collections::HashMap, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use futures::TryStreamExt;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

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

const TX_PER_PAGE: usize = 50;

#[derive(Template)]
#[template(path = "admin/transactions/index.html")]
struct TransactionsIndexTemplate {
    transactions: Vec<TransactionRow>,
    page: usize,
    total_pages: usize,
    total: usize,
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

#[derive(Deserialize)]
pub struct TxPageQuery {
    #[serde(default = "default_tx_page")]
    page: usize,
}
fn default_tx_page() -> usize { 1 }

pub async fn transactions_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Query(q): Query<TxPageQuery>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let all = list_transactions(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let active_name = session_user.user().company_name.clone();

    let mut rows: Vec<TransactionRow> = all
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

    let total = rows.len();
    let total_pages = (total + TX_PER_PAGE - 1) / TX_PER_PAGE;
    let page = q.page.max(1).min(total_pages.max(1));
    let start = (page - 1) * TX_PER_PAGE;
    let page_rows = rows.drain(start..(start + TX_PER_PAGE).min(total)).collect();

    render(TransactionsIndexTemplate { transactions: page_rows, page, total_pages, total })
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
        None,
        None,
        None,
        None,
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

// ── JSON API for the dashboard ────────────────────────────────────────────

#[derive(Serialize)]
pub struct TxApiItem {
    pub id: String,
    pub date: String,
    pub description: String,
    pub tx_type: String,
    pub amount: f64,
    pub category: String,
    pub account_from: String,
    pub account_to: String,
    pub contact: String,
    pub is_confirmed: bool,
    pub cfdi_folio: String,
    pub currency: String,
    pub notes: String,
}

pub async fn transactions_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TxApiItem>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let filter = bson::doc! { "company_id": active_company };

    // Parallel lookup fetches
    let (accs, cats, contacts, txs) = tokio::try_join!(
        async {
            state.accounts.find(filter.clone()).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .try_collect::<Vec<_>>().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        },
        async {
            state.categories.find(filter.clone()).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .try_collect::<Vec<_>>().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        },
        async {
            state.contacts.find(filter.clone()).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .try_collect::<Vec<_>>().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        },
        async {
            let opts = mongodb::options::FindOptions::builder()
                .sort(bson::doc! { "date": -1 })
                .limit(5000_i64)
                .build();
            state.transactions.find(filter.clone()).with_options(opts).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .try_collect::<Vec<_>>().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        },
    )?;

    // Build O(1) lookup maps
    let acc_map: HashMap<String, String> = accs.into_iter()
        .filter_map(|a| a.id.map(|id| (id.to_hex(), a.name)))
        .collect();
    let cat_map: HashMap<String, String> = cats.into_iter()
        .filter_map(|c| c.id.map(|id| (id.to_hex(), c.name)))
        .collect();
    let con_map: HashMap<String, String> = contacts.into_iter()
        .filter_map(|c| c.id.map(|id| (id.to_hex(), c.name)))
        .collect();

    let items = txs.into_iter().filter_map(|tx| {
        let id = tx.id?;
        let date = tx.date.to_chrono().format("%Y-%m-%d").to_string();
        let tx_type = match tx.transaction_type {
            crate::models::TransactionType::Income   => "income",
            crate::models::TransactionType::Expense  => "expense",
            crate::models::TransactionType::Transfer => "transfer",
        }.to_string();
        Some(TxApiItem {
            id: id.to_hex(),
            date,
            description: tx.description,
            tx_type,
            amount: tx.amount,
            category:     cat_map.get(&tx.category_id.to_hex()).cloned().unwrap_or_default(),
            account_from: tx.account_from_id.as_ref().and_then(|id| acc_map.get(&id.to_hex())).cloned().unwrap_or_default(),
            account_to:   tx.account_to_id.as_ref().and_then(|id|  acc_map.get(&id.to_hex())).cloned().unwrap_or_default(),
            contact:      tx.contact_id.as_ref().and_then(|id|      con_map.get(&id.to_hex())).cloned().unwrap_or_default(),
            is_confirmed: tx.is_confirmed,
            cfdi_folio:   tx.cfdi_folio.unwrap_or_default(),
            currency:     tx.currency.unwrap_or_else(|| "MXN".into()),
            notes:        tx.notes.unwrap_or_default(),
        })
    }).collect();

    Ok(Json(items))
}
