use std::{collections::HashMap, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::{DateTime, oid::ObjectId};
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{AccountType, ContactType, FlowType, PlannedStatus, TransactionType},
    session::SessionUser,
    state::{
        AppState, create_account, create_category, create_contact, create_forecast,
        create_planned_entry, create_recurring_plan, create_transaction, delete_account,
        delete_category, delete_contact, delete_forecast, delete_planned_entry,
        delete_recurring_plan, delete_transaction, get_account_by_id, get_category_by_id,
        get_company_by_id, get_contact_by_id, get_forecast_by_id, get_planned_entry_by_id,
        get_recurring_plan_by_id, get_transaction_by_id, get_user_by_id, list_accounts,
        list_categories, list_contacts, list_forecasts, list_planned_entries, list_recurring_plans,
        list_transactions, list_users, regenerate_planned_entries_for_plan_id, update_account,
        update_category, update_contact, update_forecast, update_planned_entry,
        update_recurring_plan, update_transaction,
    },
};

fn require_admin_active(session_user: &SessionUser) -> Result<ObjectId, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(session_user.active_company_id().clone())
}

fn ensure_same_company(entity_company: &ObjectId, active_company: &ObjectId) -> Result<(), StatusCode> {
    if entity_company != active_company {
        Err(StatusCode::FORBIDDEN)
    } else {
        Ok(())
    }
}

async fn validate_company_refs(
    state: &AppState,
    active_company: &ObjectId,
    category: Option<&ObjectId>,
    account: Option<&ObjectId>,
    contact: Option<&ObjectId>,
) -> Result<(), StatusCode> {
    if let Some(cid) = category {
        match get_category_by_id(state, cid).await {
            Ok(Some(cat)) => ensure_same_company(&cat.company_id, active_company)?,
            Ok(None) => return Err(StatusCode::BAD_REQUEST),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    if let Some(aid) = account {
        match get_account_by_id(state, aid).await {
            Ok(Some(acc)) => ensure_same_company(&acc.company_id, active_company)?,
            Ok(None) => return Err(StatusCode::BAD_REQUEST),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    if let Some(cid) = contact {
        match get_contact_by_id(state, cid).await {
            Ok(Some(c)) => ensure_same_company(&c.company_id, active_company)?,
            Ok(None) => return Err(StatusCode::BAD_REQUEST),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    Ok(())
}

async fn validate_recurring_plan_company(
    state: &AppState,
    plan_id: &ObjectId,
    active_company: &ObjectId,
) -> Result<(), StatusCode> {
    match get_recurring_plan_by_id(state, plan_id).await {
        Ok(Some(plan)) => ensure_same_company(&plan.company_id, active_company),
        Ok(None) => Err(StatusCode::BAD_REQUEST),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn validate_planned_entry_company(
    state: &AppState,
    entry_id: &ObjectId,
    active_company: &ObjectId,
) -> Result<(), StatusCode> {
    match get_planned_entry_by_id(state, entry_id).await {
        Ok(Some(entry)) => ensure_same_company(&entry.company_id, active_company),
        Ok(None) => Err(StatusCode::BAD_REQUEST),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn validate_user_in_company(
    state: &AppState,
    user_id: &ObjectId,
    active_company: &ObjectId,
) -> Result<(), StatusCode> {
    match get_user_by_id(state, user_id).await {
        Ok(Some(user)) => {
            if user.company_ids.contains(active_company) {
                Ok(())
            } else {
                Err(StatusCode::FORBIDDEN)
            }
        }
        Ok(None) => Err(StatusCode::BAD_REQUEST),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Clone)]
struct SimpleOption {
    value: String,
    label: String,
    selected: bool,
}

fn clean_opt(input: Option<String>) -> Option<String> {
    input.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_object_id(value: &str, label: &str) -> Result<ObjectId, String> {
    ObjectId::from_str(value).map_err(|_| format!("{} inválido", label))
}

fn parse_f64_field(value: &str, label: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{} debe ser numérico", label))
}

fn parse_i32_field(value: &str, label: &str) -> Result<i32, String> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("{} debe ser numérico", label))
}

fn parse_optional_i32_field(value: Option<String>, label: &str) -> Result<Option<i32>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_i32_field(&v, label)?)),
        None => Ok(None),
    }
}

fn parse_optional_f64_field(value: Option<String>, label: &str) -> Result<Option<f64>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_f64_field(&v, label)?)),
        None => Ok(None),
    }
}

fn parse_datetime_field(value: &str, label: &str) -> Result<DateTime, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "{} es obligatorio (usa RFC3339: 2024-01-01T00:00:00Z)",
            label
        ));
    }
    DateTime::parse_rfc3339_str(trimmed)
        .map_err(|_| format!("Formato de fecha/hora inválido para {}", label))
}

fn parse_optional_datetime_field(
    value: Option<String>,
    label: &str,
) -> Result<Option<DateTime>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_datetime_field(&v, label)?)),
        None => Ok(None),
    }
}

fn parse_flow_type(value: &str) -> Result<FlowType, String> {
    match value.to_lowercase().as_str() {
        "income" => Ok(FlowType::Income),
        "expense" => Ok(FlowType::Expense),
        _ => Err("Tipo de flujo inválido".into()),
    }
}

fn parse_account_type(value: &str) -> Result<AccountType, String> {
    match value {
        "bank" => Ok(AccountType::Bank),
        "cash" => Ok(AccountType::Cash),
        "credit_card" => Ok(AccountType::CreditCard),
        "investment" => Ok(AccountType::Investment),
        "other" => Ok(AccountType::Other),
        _ => Err("Tipo de cuenta inválido".into()),
    }
}

fn parse_contact_type(value: &str) -> Result<ContactType, String> {
    match value.to_lowercase().as_str() {
        "customer" => Ok(ContactType::Customer),
        "supplier" => Ok(ContactType::Supplier),
        "service" => Ok(ContactType::Service),
        "other" => Ok(ContactType::Other),
        _ => Err("Tipo de contacto inválido".into()),
    }
}

fn parse_planned_status(value: &str) -> Result<PlannedStatus, String> {
    match value {
        "planned" => Ok(PlannedStatus::Planned),
        "partially_covered" => Ok(PlannedStatus::PartiallyCovered),
        "covered" => Ok(PlannedStatus::Covered),
        "overdue" => Ok(PlannedStatus::Overdue),
        "cancelled" => Ok(PlannedStatus::Cancelled),
        _ => Err("Estado inválido".into()),
    }
}

fn parse_transaction_type(value: &str) -> Result<TransactionType, String> {
    match value {
        "income" => Ok(TransactionType::Income),
        "expense" => Ok(TransactionType::Expense),
        "transfer" => Ok(TransactionType::Transfer),
        _ => Err("Tipo de transacción inválido".into()),
    }
}

fn flow_type_value(value: &FlowType) -> &'static str {
    match value {
        FlowType::Income => "income",
        FlowType::Expense => "expense",
    }
}

fn account_type_value(value: &AccountType) -> &'static str {
    match value {
        AccountType::Bank => "bank",
        AccountType::Cash => "cash",
        AccountType::CreditCard => "credit_card",
        AccountType::Investment => "investment",
        AccountType::Other => "other",
    }
}

fn contact_type_value(value: &ContactType) -> &'static str {
    match value {
        ContactType::Customer => "customer",
        ContactType::Supplier => "supplier",
        ContactType::Service => "service",
        ContactType::Other => "other",
    }
}

fn planned_status_value(value: &PlannedStatus) -> &'static str {
    match value {
        PlannedStatus::Planned => "planned",
        PlannedStatus::PartiallyCovered => "partially_covered",
        PlannedStatus::Covered => "covered",
        PlannedStatus::Overdue => "overdue",
        PlannedStatus::Cancelled => "cancelled",
    }
}

fn transaction_type_value(value: &TransactionType) -> &'static str {
    match value {
        TransactionType::Income => "income",
        TransactionType::Expense => "expense",
        TransactionType::Transfer => "transfer",
    }
}

async fn company_options(
    state: &AppState,
    active: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let company = get_company_by_id(state, active)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut opts = Vec::new();
    if let Some(c) = company {
        opts.push(SimpleOption {
            value: active.to_hex(),
            label: c.name,
            selected: true,
        });
    }
    Ok(opts)
}

fn flow_options(selected: &str) -> Vec<SimpleOption> {
    vec![
        SimpleOption {
            value: "income".into(),
            label: "Ingreso".into(),
            selected: selected == "income",
        },
        SimpleOption {
            value: "expense".into(),
            label: "Gasto".into(),
            selected: selected == "expense",
        },
    ]
}

fn account_type_options(selected: &str) -> Vec<SimpleOption> {
    vec![
        SimpleOption {
            value: "bank".into(),
            label: "Banco".into(),
            selected: selected == "bank",
        },
        SimpleOption {
            value: "cash".into(),
            label: "Efectivo".into(),
            selected: selected == "cash",
        },
        SimpleOption {
            value: "credit_card".into(),
            label: "Tarjeta de crédito".into(),
            selected: selected == "credit_card",
        },
        SimpleOption {
            value: "investment".into(),
            label: "Inversión".into(),
            selected: selected == "investment",
        },
        SimpleOption {
            value: "other".into(),
            label: "Otra".into(),
            selected: selected == "other",
        },
    ]
}

fn contact_type_options(selected: &str) -> Vec<SimpleOption> {
    vec![
        SimpleOption {
            value: "customer".into(),
            label: "Cliente".into(),
            selected: selected == "customer",
        },
        SimpleOption {
            value: "supplier".into(),
            label: "Proveedor".into(),
            selected: selected == "supplier",
        },
        SimpleOption {
            value: "service".into(),
            label: "Servicio".into(),
            selected: selected == "service",
        },
        SimpleOption {
            value: "other".into(),
            label: "Otro".into(),
            selected: selected == "other",
        },
    ]
}

fn planned_status_options(selected: &str) -> Vec<SimpleOption> {
    vec![
        SimpleOption {
            value: "planned".into(),
            label: "Planeado".into(),
            selected: selected == "planned",
        },
        SimpleOption {
            value: "partially_covered".into(),
            label: "Parcial".into(),
            selected: selected == "partially_covered",
        },
        SimpleOption {
            value: "covered".into(),
            label: "Cubierto".into(),
            selected: selected == "covered",
        },
        SimpleOption {
            value: "overdue".into(),
            label: "Vencido".into(),
            selected: selected == "overdue",
        },
        SimpleOption {
            value: "cancelled".into(),
            label: "Cancelado".into(),
            selected: selected == "cancelled",
        },
    ]
}

fn transaction_type_options(selected: &str) -> Vec<SimpleOption> {
    vec![
        SimpleOption {
            value: "income".into(),
            label: "Ingreso".into(),
            selected: selected == "income",
        },
        SimpleOption {
            value: "expense".into(),
            label: "Gasto".into(),
            selected: selected == "expense",
        },
        SimpleOption {
            value: "transfer".into(),
            label: "Transferencia".into(),
            selected: selected == "transfer",
        },
    ]
}

fn select_from_map(
    map: &HashMap<ObjectId, String>,
    selected: Option<&ObjectId>,
) -> Vec<SimpleOption> {
    map.iter()
        .map(|(id, name)| SimpleOption {
            value: id.to_hex(),
            label: name.clone(),
            selected: selected.map(|s| *s == *id).unwrap_or(false),
        })
        .collect()
}

fn build_lookup_map(items: Vec<(ObjectId, String)>) -> HashMap<ObjectId, String> {
    let mut map = HashMap::new();
    for (id, name) in items {
        map.insert(id, name);
    }
    map
}

fn opt_to_string(opt: &Option<ObjectId>) -> Option<String> {
    opt.as_ref().map(|o| o.to_hex())
}

fn datetime_to_string(dt: &DateTime) -> String {
    dt.try_to_rfc3339_string()
        .unwrap_or_else(|_| dt.to_string())
}

/// ---------- ACCOUNTS ----------

#[derive(Template)]
#[template(path = "admin/accounts/index.html")]
struct AccountsIndexTemplate {
    accounts: Vec<AccountRow>,
}

struct AccountRow {
    id: String,
    name: String,
    company: String,
    account_type: String,
    currency: String,
    is_active: bool,
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

    let companies = company_options(&state, &company_id).await.unwrap_or_default();

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

/// ---------- CATEGORIES ----------

#[derive(Template)]
#[template(path = "admin/categories/index.html")]
struct CategoriesIndexTemplate {
    categories: Vec<CategoryRow>,
}

struct CategoryRow {
    id: String,
    name: String,
    company: String,
    flow_type: String,
    parent: String,
}

#[derive(Template)]
#[template(path = "admin/categories/form.html")]
struct CategoryFormTemplate {
    action: String,
    name: String,
    flow_type: String,
    parent_id: Option<String>,
    companies: Vec<SimpleOption>,
    flow_options: Vec<SimpleOption>,
    parent_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct CategoryFormData {
    name: String,
    company_id: String,
    flow_type: String,
    #[serde(default)]
    parent_id: Option<String>,
}

pub async fn categories_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let categories = list_categories(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .collect::<Vec<_>>();
    let category_map = build_lookup_map(
        categories
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c.name.clone())))
            .collect(),
    );
    let active_company = session_user.active_company_id().clone();
    let active_name = session_user.user().company_name.clone();

    let rows = categories
        .into_iter()
        .filter(|cat| cat.company_id == active_company)
        .filter_map(|cat| {
            cat.id.map(|id| CategoryRow {
                id: id.to_hex(),
                name: cat.name,
                company: active_name.clone(),
                flow_type: flow_type_value(&cat.flow_type).to_string(),
                parent: cat
                    .parent_id
                    .and_then(|pid| category_map.get(&pid).cloned())
                    .unwrap_or_else(|| "-".into()),
            })
        })
        .collect();

    render(CategoriesIndexTemplate { categories: rows })
}

pub async fn categories_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let parents = category_parent_options(&state, None, &active_company).await?;

    render(CategoryFormTemplate {
        action: "/admin/categories".into(),
        name: String::new(),
        flow_type: "income".into(),
        parent_id: None,
        companies,
        flow_options: flow_options("income"),
        parent_options: parents,
        is_edit: false,
        errors: None,
    })
}

pub async fn categories_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CategoryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id).await.unwrap_or_default();
    let parents = category_parent_options(&state, None, &company_id)
        .await
        .unwrap_or_default();

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(msg) => {
            return render(CategoryFormTemplate {
                action: "/admin/categories".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                parent_id: form.parent_id.clone(),
                companies,
                flow_options: flow_options(&form.flow_type),
                        parent_options: category_parent_options(&state, None, &company_id)
                    .await
                    .unwrap_or_default(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let parent_id = match form.parent_id.clone().and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(pid) => Some(
            parse_object_id(&pid, "Categoría padre")
                .map_err(|e| e)
                .map_err(|msg| {
                    render(CategoryFormTemplate {
                        action: "/admin/categories".into(),
                        name: form.name.clone(),
                        flow_type: form.flow_type.clone(),
                        parent_id: Some(pid.clone()),
                        companies: companies.clone(),
                        flow_options: flow_options(&form.flow_type),
                        parent_options: parents.clone(),
                        is_edit: false,
                        errors: Some(msg),
                    })
                    .map(IntoResponse::into_response)
                    .unwrap_or_else(|status| status.into_response())
                }),
        ),
        None => None,
    };

    let parent_id = match parent_id {
        Some(Ok(id)) => Some(id),
        Some(Err(resp)) => return resp,
        None => None,
    };

    if let Some(pid) = parent_id {
        match get_category_by_id(&state, &pid).await {
            Ok(Some(cat)) => {
                if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                    return status.into_response();
                }
            }
            Ok(None) => return StatusCode::BAD_REQUEST.into_response(),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

    match create_category(
        &state,
        &company_id,
        form.name.trim(),
        flow_type,
        parent_id,
        None,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn categories_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let category = get_category_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&category.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let parents =
        category_parent_options(&state, category.parent_id.as_ref(), &active_company).await?;

    render(CategoryFormTemplate {
        action: format!("/admin/categories/{}/update", id),
        name: category.name,
        flow_type: flow_type_value(&category.flow_type).to_string(),
        parent_id: opt_to_string(&category.parent_id),
        companies,
        flow_options: flow_options(flow_type_value(&category.flow_type)),
        parent_options: parents,
        is_edit: true,
        errors: None,
    })
}

pub async fn categories_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<CategoryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_category_by_id(&state, &object_id).await {
        Ok(Some(cat)) => {
            if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(msg) => {
            let companies = company_options(&state, &company_id)
                .await
                .unwrap_or_default();
            let parents = category_parent_options(&state, None, &company_id)
                .await
                .unwrap_or_default();
            return render(CategoryFormTemplate {
                action: format!("/admin/categories/{}/update", id),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                parent_id: form.parent_id.clone(),
                companies,
                flow_options: flow_options(&form.flow_type),
                parent_options: parents,
                is_edit: true,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let parent_id = match form.parent_id.clone().and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(pid) => match parse_object_id(&pid, "Categoría padre") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    if let Some(pid) = parent_id {
        match get_category_by_id(&state, &pid).await {
            Ok(Some(cat)) => {
                if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                    return status.into_response();
                }
            }
            Ok(None) => return StatusCode::BAD_REQUEST.into_response(),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

    match update_category(
        &state,
        &object_id,
        &company_id,
        form.name.trim(),
        flow_type,
        parent_id,
        None,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn categories_delete(
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

    match get_category_by_id(&state, &object_id).await {
        Ok(Some(cat)) => {
            if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match delete_category(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn category_parent_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let categories = list_categories(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == *company_id)
        .collect::<Vec<_>>();
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin padre".into(),
        selected: selected.is_none(),
    });
    options.extend(categories.into_iter().filter_map(|c| {
        c.id.map(|id| SimpleOption {
            value: id.to_hex(),
            label: c.name,
            selected: selected.map(|s| *s == id).unwrap_or(false),
        })
    }));
    Ok(options)
}

/// ---------- CONTACTS ----------

#[derive(Template)]
#[template(path = "admin/contacts/index.html")]
struct ContactsIndexTemplate {
    contacts: Vec<ContactRow>,
}

struct ContactRow {
    id: String,
    name: String,
    company: String,
    kind: String,
    email: String,
}

#[derive(Template)]
#[template(path = "admin/contacts/form.html")]
struct ContactFormTemplate {
    action: String,
    name: String,
    contact_type: String,
    email: String,
    phone: String,
    notes: String,
    companies: Vec<SimpleOption>,
    contact_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct ContactFormData {
    name: String,
    company_id: String,
    contact_type: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn contacts_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let contacts = list_contacts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .collect::<Vec<_>>();
    let active_name = session_user.user().company_name.clone();

    let rows = contacts
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .filter_map(|c| {
            c.id.map(|id| ContactRow {
                id: id.to_hex(),
                name: c.name,
                company: active_name.clone(),
                kind: contact_type_value(&c.contact_type).to_string(),
                email: c.email.unwrap_or_else(|| "-".into()),
            })
        })
        .collect();

    render(ContactsIndexTemplate { contacts: rows })
}

pub async fn contacts_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let companies = company_options(&state, &active_company).await?;

    render(ContactFormTemplate {
        action: "/admin/contacts".into(),
        name: String::new(),
        contact_type: "customer".into(),
        email: String::new(),
        phone: String::new(),
        notes: String::new(),
        companies,
        contact_options: contact_type_options("customer"),
        is_edit: false,
        errors: None,
    })
}

pub async fn contacts_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ContactFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id).await.unwrap_or_default();

    let contact_type = match parse_contact_type(&form.contact_type) {
        Ok(c) => c,
        Err(msg) => {
            return render(ContactFormTemplate {
                action: "/admin/contacts".into(),
                name: form.name.clone(),
                contact_type: form.contact_type.clone(),
                email: form.email.clone().unwrap_or_default(),
                phone: form.phone.clone().unwrap_or_default(),
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                contact_options: contact_type_options(&form.contact_type),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let email = clean_opt(form.email);
    let phone = clean_opt(form.phone);
    let notes = clean_opt(form.notes);

    match create_contact(
        &state,
        &company_id,
        form.name.trim(),
        contact_type,
        email,
        phone,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn contacts_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let contact = get_contact_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&contact.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;

    render(ContactFormTemplate {
        action: format!("/admin/contacts/{}/update", id),
        name: contact.name,
        contact_type: contact_type_value(&contact.contact_type).to_string(),
        email: contact.email.unwrap_or_default(),
        phone: contact.phone.unwrap_or_default(),
        notes: contact.notes.unwrap_or_default(),
        companies,
        contact_options: contact_type_options(contact_type_value(&contact.contact_type)),
        is_edit: true,
        errors: None,
    })
}

pub async fn contacts_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ContactFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_contact_by_id(&state, &object_id).await {
        Ok(Some(contact)) => {
            if let Err(status) = ensure_same_company(&contact.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let contact_type = match parse_contact_type(&form.contact_type) {
        Ok(c) => c,
        Err(msg) => {
            let companies = company_options(&state, session_user.active_company_id())
                .await
                .unwrap_or_default();
            return render(ContactFormTemplate {
                action: format!("/admin/contacts/{}/update", id),
                name: form.name.clone(),
                contact_type: form.contact_type.clone(),
                email: form.email.clone().unwrap_or_default(),
                phone: form.phone.clone().unwrap_or_default(),
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                contact_options: contact_type_options(&form.contact_type),
                is_edit: true,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let email = clean_opt(form.email);
    let phone = clean_opt(form.phone);
    let notes = clean_opt(form.notes);

    match update_contact(
        &state,
        &object_id,
        &company_id,
        form.name.trim(),
        contact_type,
        email,
        phone,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn contacts_delete(
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

    match get_contact_by_id(&state, &object_id).await {
        Ok(Some(contact)) => {
            if let Err(status) = ensure_same_company(&contact.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match delete_contact(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// ---------- RECURRING PLANS ----------

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

    // Validate related entities belong to the active company
    if let Err(status) =
        validate_company_refs(&state, &company_id, Some(&category_id), Some(&account_expected_id), contact_id.as_ref())
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

    let contact_id = contact_id;

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

/// ---------- PLANNED ENTRIES ----------

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

/// ---------- TRANSACTIONS ----------

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

/// ---------- FORECASTS ----------

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

/// ---------- OPTION HELPERS ----------

async fn category_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let categories = list_categories(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(categories
        .into_iter()
        .filter(|c| c.company_id == *company_id)
        .filter_map(|c| {
            c.id.map(|id| SimpleOption {
                value: id.to_hex(),
                label: c.name,
                selected: selected.map(|s| *s == id).unwrap_or(false),
            })
        })
        .collect())
}

async fn account_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let accounts = list_accounts(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(accounts
        .into_iter()
        .filter(|a| a.company_id == *company_id)
        .filter_map(|a| {
            a.id.map(|id| SimpleOption {
                value: id.to_hex(),
                label: format!("{} ({})", a.name, a.currency),
                selected: selected.map(|s| *s == id).unwrap_or(false),
            })
        })
        .collect())
}

async fn contact_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let contacts = list_contacts(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin contacto".into(),
        selected: selected.is_none(),
    });
    options.extend(
        contacts
            .into_iter()
            .filter(|c| c.company_id == *company_id)
            .filter_map(|c| {
                c.id.map(|id| SimpleOption {
                    value: id.to_hex(),
                    label: c.name,
                    selected: selected.map(|s| *s == id).unwrap_or(false),
                })
            }),
    );
    Ok(options)
}

async fn recurring_plan_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let plans = list_recurring_plans(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin plan".into(),
        selected: selected.is_none(),
    });
    options.extend(
        plans
            .into_iter()
            .filter(|p| p.company_id == *company_id)
            .filter_map(|p| {
                p.id.map(|id| SimpleOption {
                    value: id.to_hex(),
                    label: p.name,
                    selected: selected.map(|s| *s == id).unwrap_or(false),
                })
            }),
    );
    Ok(options)
}

async fn planned_entry_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let entries = list_planned_entries(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin enlace".into(),
        selected: selected.is_none(),
    });
    options.extend(
        entries
            .into_iter()
            .filter(|e| e.company_id == *company_id)
            .filter_map(|e| {
                e.id.map(|id| SimpleOption {
                    value: id.to_hex(),
                    label: e.name,
                    selected: selected.map(|s| *s == id).unwrap_or(false),
                })
            }),
    );
    Ok(options)
}

async fn user_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let users = list_users(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin usuario".into(),
        selected: selected.is_none(),
    });
    options.extend(
        users
            .into_iter()
            .filter(|u| u.company_ids.contains(company_id))
            .map(|u| SimpleOption {
                value: u.id.to_hex(),
                label: u.email,
                selected: selected.map(|s| s == &u.id).unwrap_or(false),
            }),
    );
    Ok(options)
}
