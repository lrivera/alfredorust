use anyhow::{Context, Result};
use bson::{DateTime, doc, oid::ObjectId};
use futures::stream::TryStreamExt;
use std::time::SystemTime;

use crate::models::{FlowType, OrderItem, OrderStatus, PlannedStatus, ServiceOrder};
use super::AppState;
use super::finance::create_planned_entry;

pub async fn list_orders(state: &AppState, company_id: &ObjectId) -> Result<Vec<ServiceOrder>> {
    let mut cursor = state
        .orders
        .find(doc! { "company_id": company_id })
        .await?;
    let mut items = Vec::new();
    while let Some(o) = cursor.try_next().await? {
        items.push(o);
    }
    Ok(items)
}

pub async fn get_order_by_id(state: &AppState, id: &ObjectId) -> Result<Option<ServiceOrder>> {
    state.orders.find_one(doc! { "_id": id }).await.map_err(Into::into)
}

pub async fn create_order(
    state: &AppState,
    company_id: &ObjectId,
    contact_id: Option<ObjectId>,
    category_id: Option<ObjectId>,
    account_id: Option<ObjectId>,
    title: &str,
    status: OrderStatus,
    amount: f64,
    scheduled_at: Option<DateTime>,
    items: Vec<OrderItem>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .orders
        .insert_one(ServiceOrder {
            id: None,
            company_id: company_id.clone(),
            contact_id,
            category_id,
            account_id,
            planned_entry_id: None,
            title: title.to_string(),
            status,
            amount,
            scheduled_at,
            completed_at: None,
            transaction_id: None,
            items,
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id.as_object_id().context("order insert missing _id")
}

pub async fn update_order(
    state: &AppState,
    id: &ObjectId,
    contact_id: Option<ObjectId>,
    category_id: Option<ObjectId>,
    account_id: Option<ObjectId>,
    title: &str,
    status: OrderStatus,
    amount: f64,
    scheduled_at: Option<DateTime>,
    items: Vec<OrderItem>,
    notes: Option<String>,
) -> Result<()> {
    let items_bson: Vec<bson::Document> = items
        .iter()
        .map(|i| doc! {
            "description": &i.description,
            "quantity": i.quantity,
            "unit_price": i.unit_price,
        })
        .collect();

    state
        .orders
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "contact_id": contact_id,
                "category_id": category_id,
                "account_id": account_id,
                "title": title,
                "status": status.as_str(),
                "amount": amount,
                "scheduled_at": scheduled_at,
                "items": items_bson,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            }},
        )
        .await?;
    Ok(())
}

/// When an order is confirmed: create a PlannedEntry (income commitment) linked to it.
/// Idempotent — does nothing if the order already has a planned_entry_id.
pub async fn confirm_order(
    state: &AppState,
    order: &ServiceOrder,
    company_id: &ObjectId,
) -> Result<()> {
    if order.planned_entry_id.is_some() {
        return Ok(());
    }
    let (Some(category_id), Some(account_id)) = (&order.category_id, &order.account_id) else {
        return Ok(());
    };
    let order_id = order.id.as_ref().context("order missing _id")?;

    let due_date = order.scheduled_at.unwrap_or_else(|| DateTime::from_system_time(SystemTime::now()));

    let planned_id = create_planned_entry(
        state,
        company_id,
        None,
        None,
        Some(order_id.clone()),
        &order.title,
        FlowType::Income,
        category_id,
        account_id,
        order.contact_id.clone(),
        order.amount,
        due_date,
        PlannedStatus::Planned,
        None,
    )
    .await?;

    state
        .orders
        .update_one(
            doc! { "_id": order_id },
            doc! { "$set": { "planned_entry_id": planned_id } },
        )
        .await?;

    Ok(())
}

/// Mark an order as completed. Payment is handled separately via the linked PlannedEntry.
pub async fn complete_order(state: &AppState, id: &ObjectId) -> Result<()> {
    let now = DateTime::from_system_time(SystemTime::now());
    state
        .orders
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "status": OrderStatus::Completed.as_str(),
                "completed_at": now,
                "updated_at": now,
            }},
        )
        .await?;
    Ok(())
}

pub async fn delete_order(state: &AppState, id: &ObjectId) -> Result<()> {
    state.orders.delete_one(doc! { "_id": id }).await?;
    Ok(())
}
