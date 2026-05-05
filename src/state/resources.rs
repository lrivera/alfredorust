use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::time::SystemTime;

use crate::models::{Resource, ResourceType};

use super::AppState;

pub async fn list_resources(state: &AppState, company_id: &ObjectId) -> Result<Vec<Resource>> {
    let mut cursor = state
        .resources
        .find(doc! { "company_id": company_id })
        .await?;
    let mut items = Vec::new();
    while let Some(resource) = cursor.try_next().await? {
        items.push(resource);
    }
    Ok(items)
}

pub async fn list_active_resources(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Vec<Resource>> {
    let mut cursor = state
        .resources
        .find(doc! { "company_id": company_id, "is_active": true })
        .await?;
    let mut items = Vec::new();
    while let Some(resource) = cursor.try_next().await? {
        items.push(resource);
    }
    Ok(items)
}

pub async fn get_resource_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Resource>> {
    state
        .resources
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn get_resource_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<Resource>> {
    state
        .resources
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn create_resource(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    resource_type: ResourceType,
    is_active: bool,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .resources
        .insert_one(Resource {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            resource_type,
            is_active,
            hourly_cost: 0.0,
            currency: "MXN".to_string(),
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("resource insert missing _id")
}

pub async fn create_resource_with_cost(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    resource_type: ResourceType,
    is_active: bool,
    hourly_cost: f64,
    currency: &str,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .resources
        .insert_one(Resource {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            resource_type,
            is_active,
            hourly_cost: hourly_cost.max(0.0),
            currency: if currency.trim().is_empty() {
                "MXN".to_string()
            } else {
                currency.trim().to_string()
            },
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("resource insert missing _id")
}

pub async fn update_resource(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    resource_type: ResourceType,
    is_active: bool,
    notes: Option<String>,
) -> Result<()> {
    state
        .resources
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "name": name,
                "resource_type": resource_type.as_str(),
                "is_active": is_active,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn update_resource_cost(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    hourly_cost: f64,
    currency: &str,
) -> Result<()> {
    state
        .resources
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "hourly_cost": hourly_cost.max(0.0),
                "currency": if currency.trim().is_empty() { "MXN" } else { currency.trim() },
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn delete_resource(state: &AppState, id: &ObjectId, company_id: &ObjectId) -> Result<()> {
    state
        .resources
        .delete_one(doc! { "_id": id, "company_id": company_id })
        .await?;
    Ok(())
}
