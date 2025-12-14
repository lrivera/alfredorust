use axum::http::StatusCode;
use mongodb::bson::oid::ObjectId;

use crate::state::{
    AppState, list_accounts, list_categories, list_contacts, list_planned_entries,
    list_recurring_plans, list_users,
};

use super::helpers::SimpleOption;

pub(super) async fn category_options(
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

pub(super) async fn account_options(
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

pub(super) async fn contact_options(
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

pub(super) async fn recurring_plan_options(
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

pub(super) async fn planned_entry_options(
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

pub(super) async fn user_options(
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

