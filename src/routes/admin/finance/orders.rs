use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{OrderItem, OrderStatus},
    session::SessionUser,
    state::{
        AppState, complete_order, create_order, delete_order, get_order_by_id,
        get_or_create_category, list_orders, update_order,
    },
};
use crate::models::FlowType;

use super::helpers::{require_admin_active, ensure_same_company, SimpleOption};
use super::options::contact_options;
use crate::state::get_contact_by_id;

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render().map(Html).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── Index ──────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin/orders/index.html")]
struct OrdersIndexTemplate {
    orders: Vec<OrderRow>,
}

struct OrderRow {
    id: String,
    title: String,
    contact_name: String,
    status: String,
    status_label: String,
    amount: f64,
    scheduled_at: String,
    has_transaction: bool,
}

pub async fn orders_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let orders = list_orders(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut rows = Vec::new();
    for o in orders {
        let contact_name = if let Some(cid) = &o.contact_id {
            get_contact_by_id(&state, cid)
                .await
                .ok()
                .flatten()
                .map(|c| c.name)
                .unwrap_or_default()
        } else {
            String::new()
        };

        rows.push(OrderRow {
            id: o.id.map(|i| i.to_hex()).unwrap_or_default(),
            title: o.title,
            contact_name,
            status: o.status.as_str().to_string(),
            status_label: o.status.label().to_string(),
            amount: o.amount,
            scheduled_at: o.scheduled_at
                .map(|d| {
                    let ms = d.timestamp_millis();
                    let secs = ms / 1000;
                    let dt = chrono::DateTime::from_timestamp(secs, 0)
                        .unwrap_or_default();
                    dt.format("%d/%m/%Y").to_string()
                })
                .unwrap_or_default(),
            has_transaction: o.transaction_id.is_some(),
        });
    }

    render(OrdersIndexTemplate { orders: rows })
}

// ── Form ───────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin/orders/form.html")]
struct OrderFormTemplate {
    action: String,
    title: String,
    status: String,
    amount: String,
    scheduled_at: String,
    notes: String,
    items_json: String,
    contacts: Vec<SimpleOption>,
    status_options: Vec<(String, String)>,
    is_edit: bool,
    errors: Option<String>,
}

fn status_options() -> Vec<(String, String)> {
    vec![
        ("pending".into(),     "Pendiente".into()),
        ("confirmed".into(),   "Confirmada".into()),
        ("in_progress".into(), "En proceso".into()),
        ("completed".into(),   "Completada".into()),
        ("cancelled".into(),   "Cancelada".into()),
    ]
}

fn parse_status(s: &str) -> OrderStatus {
    match s {
        "confirmed"   => OrderStatus::Confirmed,
        "in_progress" => OrderStatus::InProgress,
        "completed"   => OrderStatus::Completed,
        "cancelled"   => OrderStatus::Cancelled,
        _             => OrderStatus::Pending,
    }
}

#[derive(Deserialize)]
pub struct OrderFormData {
    title: String,
    #[serde(default)]
    contact_id: Option<String>,
    #[serde(default = "default_pending")]
    status: String,
    amount: String,
    #[serde(default)]
    scheduled_at: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    /// JSON array: [{"description":"...","quantity":1,"unit_price":100}]
    #[serde(default)]
    items_json: Option<String>,
}

fn default_pending() -> String { "pending".to_string() }

fn parse_items(json: &str) -> Vec<OrderItem> {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            Some(OrderItem {
                description: v.get("description")?.as_str()?.to_string(),
                quantity: v.get("quantity")?.as_f64()?,
                unit_price: v.get("unit_price")?.as_f64()?,
            })
        })
        .filter(|i| !i.description.is_empty() && i.quantity > 0.0)
        .collect()
}

fn parse_scheduled_at(s: &str) -> Option<bson::DateTime> {
    let s = s.trim();
    if s.is_empty() { return None; }
    let dt = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    let dt = dt.and_hms_opt(0, 0, 0)?;
    let utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
    Some(bson::DateTime::from_millis(utc.timestamp_millis()))
}

pub async fn orders_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let contacts = contact_options(&state, None, &company_id).await?;

    render(OrderFormTemplate {
        action: "/admin/orders".into(),
        title: String::new(),
        status: "pending".into(),
        amount: String::new(),
        scheduled_at: String::new(),
        notes: String::new(),
        items_json: "[]".into(),
        contacts,
        status_options: status_options(),
        is_edit: false,
        errors: None,
    })
}

pub async fn orders_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<OrderFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };

    let amount: f64 = form.amount.trim().parse().unwrap_or(0.0);
    let contact_id = form.contact_id.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| ObjectId::from_str(s).ok());
    let scheduled_at = form.scheduled_at.as_deref()
        .and_then(|s| parse_scheduled_at(s));
    let items = parse_items(form.items_json.as_deref().unwrap_or("[]"));
    let notes = form.notes.filter(|s| !s.trim().is_empty());
    let status = parse_status(&form.status);

    match create_order(&state, &company_id, contact_id, form.title.trim(), status, amount, scheduled_at, items, notes).await {
        Ok(_) => Redirect::to("/admin/orders").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn orders_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let order = get_order_by_id(&state, &oid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&order.company_id, &company_id)?;

    let contacts = contact_options(&state, None, &company_id).await?;

    let items_json = serde_json::to_string(&order.items.iter().map(|i| serde_json::json!({
        "description": i.description,
        "quantity": i.quantity,
        "unit_price": i.unit_price,
    })).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());

    let scheduled_at = order.scheduled_at.map(|d| {
        let ms = d.timestamp_millis();
        chrono::DateTime::from_timestamp(ms / 1000, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string()
    }).unwrap_or_default();

    render(OrderFormTemplate {
        action: format!("/admin/orders/{}/update", id),
        title: order.title,
        status: order.status.as_str().to_string(),
        amount: order.amount.to_string(),
        scheduled_at,
        notes: order.notes.unwrap_or_default(),
        items_json,
        contacts,
        status_options: status_options(),
        is_edit: true,
        errors: None,
    })
}

pub async fn orders_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<OrderFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_order_by_id(&state, &oid).await {
        Ok(Some(o)) => { if let Err(s) = ensure_same_company(&o.company_id, &company_id) { return s.into_response(); } }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let amount: f64 = form.amount.trim().parse().unwrap_or(0.0);
    let contact_id = form.contact_id.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| ObjectId::from_str(s).ok());
    let scheduled_at = form.scheduled_at.as_deref().and_then(|s| parse_scheduled_at(s));
    let items = parse_items(form.items_json.as_deref().unwrap_or("[]"));
    let notes = form.notes.filter(|s| !s.trim().is_empty());
    let status = parse_status(&form.status);

    match update_order(&state, &oid, contact_id, form.title.trim(), status, amount, scheduled_at, items, notes).await {
        Ok(_) => Redirect::to("/admin/orders").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn orders_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_order_by_id(&state, &oid).await {
        Ok(Some(o)) => { if let Err(s) = ensure_same_company(&o.company_id, &company_id) { return s.into_response(); } }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match delete_order(&state, &oid).await {
        Ok(_) => Redirect::to("/admin/orders").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn orders_complete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_order_by_id(&state, &oid).await {
        Ok(Some(o)) => { if let Err(s) = ensure_same_company(&o.company_id, &company_id) { return s.into_response(); } }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let category_id = match get_or_create_category(&state, &company_id, "Órdenes de servicio", FlowType::Income).await {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match complete_order(&state, &oid, &company_id, &category_id).await {
        Ok(_) => Redirect::to("/admin/orders").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
