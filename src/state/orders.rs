use anyhow::{Context, Result};
use bson::{DateTime, doc, oid::ObjectId};
use futures::stream::TryStreamExt;
use std::time::SystemTime;

use crate::models::{OrderItem, OrderStatus, ServiceOrder, TransactionType};
use super::AppState;

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

pub async fn delete_order(state: &AppState, id: &ObjectId) -> Result<()> {
    state.orders.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

/// Mark an order as completed, create an income transaction, link them.
pub async fn complete_order(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    category_id: &ObjectId,
) -> Result<ObjectId> {
    let order = get_order_by_id(state, id)
        .await?
        .context("order not found")?;

    let now = DateTime::from_system_time(SystemTime::now());

    let tx_res = state
        .transactions
        .insert_one(crate::models::Transaction {
            id: None,
            company_id: company_id.clone(),
            date: now,
            description: order.title.clone(),
            transaction_type: TransactionType::Income,
            category_id: category_id.clone(),
            account_from_id: None,
            account_to_id: None,
            amount: order.amount,
            planned_entry_id: None,
            is_confirmed: true,
            created_at: Some(now),
            updated_at: None,
            contact_id: order.contact_id,
            cfdi_uuid: None,
            currency: None,
            cfdi_folio: None,
            notes: None,
        })
        .await?;

    let tx_id = tx_res
        .inserted_id
        .as_object_id()
        .context("transaction insert missing _id")?;

    state
        .orders
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "status": OrderStatus::Completed.as_str(),
                "completed_at": now,
                "transaction_id": tx_id,
                "updated_at": now,
            }},
        )
        .await?;

    Ok(tx_id)
}
