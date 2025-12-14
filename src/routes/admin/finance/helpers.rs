use std::{collections::HashMap, str::FromStr};

use askama::Template;
use axum::{
    http::StatusCode,
    response::Html,
};
use mongodb::bson::{DateTime, oid::ObjectId};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{AccountType, ContactType, FlowType, PlannedStatus, TransactionType},
    session::SessionUser,
    state::{
        AppState, get_account_by_id, get_category_by_id, get_company_by_id, get_contact_by_id,
        get_planned_entry_by_id, get_recurring_plan_by_id, get_user_by_id,
    },
};

pub(super) fn require_admin_active(session_user: &SessionUser) -> Result<ObjectId, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(session_user.active_company_id().clone())
}

pub(super) fn ensure_same_company(
    entity_company: &ObjectId,
    active_company: &ObjectId,
) -> Result<(), StatusCode> {
    if entity_company != active_company {
        Err(StatusCode::FORBIDDEN)
    } else {
        Ok(())
    }
}

pub(super) async fn validate_company_refs(
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

pub(super) async fn validate_recurring_plan_company(
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

pub(super) async fn validate_planned_entry_company(
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

pub(super) async fn validate_user_in_company(
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

pub(super) fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Clone)]
pub(super) struct SimpleOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

pub(super) fn clean_opt(input: Option<String>) -> Option<String> {
    input.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn parse_object_id(value: &str, label: &str) -> Result<ObjectId, String> {
    ObjectId::from_str(value).map_err(|_| format!("{} inválido", label))
}

pub(super) fn parse_f64_field(value: &str, label: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{} debe ser numérico", label))
}

pub(super) fn parse_i32_field(value: &str, label: &str) -> Result<i32, String> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("{} debe ser numérico", label))
}

pub(super) fn parse_optional_i32_field(
    value: Option<String>,
    label: &str,
) -> Result<Option<i32>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_i32_field(&v, label)?)),
        None => Ok(None),
    }
}

pub(super) fn parse_optional_f64_field(
    value: Option<String>,
    label: &str,
) -> Result<Option<f64>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_f64_field(&v, label)?)),
        None => Ok(None),
    }
}

pub(super) fn parse_datetime_field(value: &str, label: &str) -> Result<DateTime, String> {
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

pub(super) fn parse_optional_datetime_field(
    value: Option<String>,
    label: &str,
) -> Result<Option<DateTime>, String> {
    match clean_opt(value) {
        Some(v) => Ok(Some(parse_datetime_field(&v, label)?)),
        None => Ok(None),
    }
}

pub(super) fn parse_flow_type(value: &str) -> Result<FlowType, String> {
    match value.to_lowercase().as_str() {
        "income" => Ok(FlowType::Income),
        "expense" => Ok(FlowType::Expense),
        _ => Err("Tipo de flujo inválido".into()),
    }
}

pub(super) fn parse_account_type(value: &str) -> Result<AccountType, String> {
    match value {
        "bank" => Ok(AccountType::Bank),
        "cash" => Ok(AccountType::Cash),
        "credit_card" => Ok(AccountType::CreditCard),
        "investment" => Ok(AccountType::Investment),
        "other" => Ok(AccountType::Other),
        _ => Err("Tipo de cuenta inválido".into()),
    }
}

pub(super) fn parse_contact_type(value: &str) -> Result<ContactType, String> {
    match value.to_lowercase().as_str() {
        "customer" => Ok(ContactType::Customer),
        "supplier" => Ok(ContactType::Supplier),
        "service" => Ok(ContactType::Service),
        "other" => Ok(ContactType::Other),
        _ => Err("Tipo de contacto inválido".into()),
    }
}

pub(super) fn parse_planned_status(value: &str) -> Result<PlannedStatus, String> {
    match value {
        "planned" => Ok(PlannedStatus::Planned),
        "partially_covered" => Ok(PlannedStatus::PartiallyCovered),
        "covered" => Ok(PlannedStatus::Covered),
        "overdue" => Ok(PlannedStatus::Overdue),
        "cancelled" => Ok(PlannedStatus::Cancelled),
        _ => Err("Estado inválido".into()),
    }
}

pub(super) fn parse_transaction_type(value: &str) -> Result<TransactionType, String> {
    match value {
        "income" => Ok(TransactionType::Income),
        "expense" => Ok(TransactionType::Expense),
        "transfer" => Ok(TransactionType::Transfer),
        _ => Err("Tipo de transacción inválido".into()),
    }
}

pub(super) fn flow_type_value(value: &FlowType) -> &'static str {
    match value {
        FlowType::Income => "income",
        FlowType::Expense => "expense",
    }
}

pub(super) fn account_type_value(value: &AccountType) -> &'static str {
    match value {
        AccountType::Bank => "bank",
        AccountType::Cash => "cash",
        AccountType::CreditCard => "credit_card",
        AccountType::Investment => "investment",
        AccountType::Other => "other",
    }
}

pub(super) fn contact_type_value(value: &ContactType) -> &'static str {
    match value {
        ContactType::Customer => "customer",
        ContactType::Supplier => "supplier",
        ContactType::Service => "service",
        ContactType::Other => "other",
    }
}

pub(super) fn planned_status_value(value: &PlannedStatus) -> &'static str {
    match value {
        PlannedStatus::Planned => "planned",
        PlannedStatus::PartiallyCovered => "partially_covered",
        PlannedStatus::Covered => "covered",
        PlannedStatus::Overdue => "overdue",
        PlannedStatus::Cancelled => "cancelled",
    }
}

pub(super) fn transaction_type_value(value: &TransactionType) -> &'static str {
    match value {
        TransactionType::Income => "income",
        TransactionType::Expense => "expense",
        TransactionType::Transfer => "transfer",
    }
}

pub(super) async fn company_options(
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

pub(super) fn flow_options(selected: &str) -> Vec<SimpleOption> {
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

pub(super) fn account_type_options(selected: &str) -> Vec<SimpleOption> {
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

pub(super) fn contact_type_options(selected: &str) -> Vec<SimpleOption> {
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

pub(super) fn planned_status_options(selected: &str) -> Vec<SimpleOption> {
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

pub(super) fn transaction_type_options(selected: &str) -> Vec<SimpleOption> {
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

pub(super) fn select_from_map(
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

pub(super) fn build_lookup_map(items: Vec<(ObjectId, String)>) -> HashMap<ObjectId, String> {
    let mut map = HashMap::new();
    for (id, name) in items {
        map.insert(id, name);
    }
    map
}

pub(super) fn opt_to_string(opt: &Option<ObjectId>) -> Option<String> {
    opt.as_ref().map(|o| o.to_hex())
}

pub(super) fn datetime_to_string(dt: &DateTime) -> String {
    dt.try_to_rfc3339_string()
        .unwrap_or_else(|_| dt.to_string())
}
