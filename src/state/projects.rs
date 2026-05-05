use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::time::SystemTime;

use crate::models::{Project, ProjectPriority, ProjectStatus};

use super::AppState;

pub async fn list_projects(state: &AppState, company_id: &ObjectId) -> Result<Vec<Project>> {
    let mut cursor = state
        .projects
        .find(doc! { "company_id": company_id })
        .await?;
    let mut items = Vec::new();
    while let Some(project) = cursor.try_next().await? {
        items.push(project);
    }
    Ok(items)
}

pub async fn get_project_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Project>> {
    state
        .projects
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn get_project_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<Project>> {
    state
        .projects
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn create_project(
    state: &AppState,
    company_id: &ObjectId,
    title: &str,
    contact_id: Option<ObjectId>,
    category_id: Option<ObjectId>,
    description: Option<String>,
    priority: ProjectPriority,
    total_budget: Option<f64>,
    scheduled_at: Option<DateTime>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let res = state
        .projects
        .insert_one(Project {
            id: None,
            company_id: company_id.clone(),
            contact_id,
            category_id,
            title: title.to_string(),
            description,
            status: ProjectStatus::Pedidos,
            priority,
            total_budget,
            scheduled_at,
            completed_at: None,
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("project insert missing _id")
}

pub async fn update_project(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    title: &str,
    contact_id: Option<ObjectId>,
    category_id: Option<ObjectId>,
    description: Option<String>,
    priority: ProjectPriority,
    total_budget: Option<f64>,
    scheduled_at: Option<DateTime>,
    notes: Option<String>,
) -> Result<()> {
    state
        .projects
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "title": title,
                "contact_id": contact_id,
                "category_id": category_id,
                "description": description,
                "priority": priority.as_str(),
                "total_budget": total_budget,
                "scheduled_at": scheduled_at,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn advance_project_phase(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ProjectStatus>> {
    let project = match get_project_by_id_for_company(state, id, company_id).await? {
        Some(p) => p,
        None => return Ok(None),
    };

    let next_phase = match project.status.next() {
        Some(s) => s,
        None => return Ok(Some(project.status)),
    };

    let now = DateTime::from_system_time(SystemTime::now());
    let completed_at =
        if next_phase == ProjectStatus::Entregado || next_phase == ProjectStatus::Cancelado {
            Some(now)
        } else {
            None
        };

    state
        .projects
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "status": next_phase.as_str(),
                "completed_at": completed_at,
                "updated_at": now,
            } },
        )
        .await?;

    Ok(Some(next_phase))
}

pub async fn delete_project(state: &AppState, id: &ObjectId, company_id: &ObjectId) -> Result<()> {
    state
        .projects
        .delete_one(doc! { "_id": id, "company_id": company_id })
        .await?;
    Ok(())
}
