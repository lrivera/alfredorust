use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::time::SystemTime;

use crate::models::ResourceLog;

use super::AppState;

pub async fn list_resource_logs(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Vec<ResourceLog>> {
    let mut cursor = state
        .resource_logs
        .find(doc! { "company_id": company_id })
        .await?;
    let mut items = Vec::new();
    while let Some(log) = cursor.try_next().await? {
        items.push(log);
    }
    Ok(items)
}

pub async fn list_resource_logs_for_project(
    state: &AppState,
    company_id: &ObjectId,
    project_id: &ObjectId,
) -> Result<Vec<ResourceLog>> {
    let mut cursor = state
        .resource_logs
        .find(doc! { "company_id": company_id, "project_id": project_id })
        .await?;
    let mut items = Vec::new();
    while let Some(log) = cursor.try_next().await? {
        items.push(log);
    }
    Ok(items)
}

pub async fn get_resource_log_by_id(
    state: &AppState,
    id: &ObjectId,
) -> Result<Option<ResourceLog>> {
    state
        .resource_logs
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn get_resource_log_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ResourceLog>> {
    state
        .resource_logs
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn create_resource_log(
    state: &AppState,
    company_id: &ObjectId,
    project_id: Option<ObjectId>,
    phase: Option<String>,
    resource_id: Option<ObjectId>,
    resource_name: Option<String>,
    started_at: DateTime,
    operator_name: Option<String>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .resource_logs
        .insert_one(ResourceLog {
            id: None,
            company_id: company_id.clone(),
            project_id,
            phase,
            resource_id,
            resource_name,
            started_at,
            ended_at: None,
            duration_hours: None,
            operator_name,
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("resource log insert missing _id")
}

pub async fn end_resource_log(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    ended_at: DateTime,
) -> Result<Option<f64>> {
    let log = match get_resource_log_by_id_for_company(state, id, company_id).await? {
        Some(l) => l,
        None => return Ok(None),
    };

    let duration_hours = if log.started_at <= ended_at {
        let duration = ended_at
            .to_chrono()
            .signed_duration_since(log.started_at.to_chrono());
        Some(duration.num_milliseconds() as f64 / 3_600_000.0)
    } else {
        None
    };

    state
        .resource_logs
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "ended_at": ended_at,
                "duration_hours": duration_hours,
            } },
        )
        .await?;

    Ok(duration_hours)
}

pub async fn update_resource_log(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    project_id: Option<ObjectId>,
    phase: Option<String>,
    resource_id: Option<ObjectId>,
    resource_name: Option<String>,
    started_at: DateTime,
    ended_at: Option<DateTime>,
    operator_name: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let duration_hours = if let Some(ended) = ended_at {
        if started_at <= ended {
            let duration = ended
                .to_chrono()
                .signed_duration_since(started_at.to_chrono());
            Some(duration.num_milliseconds() as f64 / 3_600_000.0)
        } else {
            None
        }
    } else {
        None
    };

    state
        .resource_logs
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "project_id": project_id,
                "phase": phase,
                "resource_id": resource_id,
                "resource_name": resource_name,
                "started_at": started_at,
                "ended_at": ended_at,
                "duration_hours": duration_hours,
                "operator_name": operator_name,
                "notes": notes,
            } },
        )
        .await?;

    Ok(())
}

pub async fn delete_resource_log(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    state
        .resource_logs
        .delete_one(doc! { "_id": id, "company_id": company_id })
        .await?;
    Ok(())
}
