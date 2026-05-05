use anyhow::{Context, Result, bail};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use serde::Serialize;
use std::time::SystemTime;

use crate::models::{ConceptStatus, ProjectConcept};

use super::{AppState, get_project_by_id_for_company};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusSummaryItem {
    pub status_id: String,
    pub name: String,
    pub position: i32,
    pub quantity: f64,
    pub unit: Option<String>,
}

pub async fn list_concept_statuses(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Vec<ConceptStatus>> {
    let mut cursor = state
        .concept_statuses
        .find(doc! { "company_id": company_id, "is_active": true })
        .sort(doc! { "position": 1 })
        .await?;
    let mut items = Vec::new();
    while let Some(status) = cursor.try_next().await? {
        items.push(status);
    }
    Ok(items)
}

pub async fn get_concept_status_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ConceptStatus>> {
    state
        .concept_statuses
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn get_initial_concept_status(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Option<ConceptStatus>> {
    if let Some(status) = state
        .concept_statuses
        .find_one(doc! { "company_id": company_id, "is_initial": true, "is_active": true })
        .await?
    {
        return Ok(Some(status));
    }

    state
        .concept_statuses
        .find_one(doc! { "company_id": company_id, "is_active": true })
        .sort(doc! { "position": 1 })
        .await
        .map_err(Into::into)
}

pub async fn create_concept_status(
    state: &AppState,
    company_id: &ObjectId,
    name: &str,
    position: i32,
    color: Option<String>,
    is_initial: bool,
    is_terminal: bool,
    is_cancelled: bool,
    is_active: bool,
) -> Result<ObjectId> {
    let name = name.trim();
    if name.is_empty() {
        bail!("concept status name is required");
    }
    ensure_single_status_marker(is_initial, is_terminal, is_cancelled)?;

    if is_initial {
        state
            .concept_statuses
            .update_many(
                doc! { "company_id": company_id, "is_initial": true },
                doc! { "$set": { "is_initial": false } },
            )
            .await?;
    }

    let res = state
        .concept_statuses
        .insert_one(ConceptStatus {
            id: None,
            company_id: company_id.clone(),
            name: name.to_string(),
            position,
            color,
            is_initial,
            is_terminal,
            is_cancelled,
            is_active,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("concept status insert missing _id")
}

pub async fn update_concept_status(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    name: &str,
    position: i32,
    color: Option<String>,
    is_initial: bool,
    is_terminal: bool,
    is_cancelled: bool,
    is_active: bool,
) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("concept status name is required");
    }
    ensure_single_status_marker(is_initial, is_terminal, is_cancelled)?;

    if is_initial {
        state
            .concept_statuses
            .update_many(
                doc! { "company_id": company_id, "is_initial": true, "_id": { "$ne": id } },
                doc! { "$set": { "is_initial": false } },
            )
            .await?;
    }

    state
        .concept_statuses
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "name": name,
                "position": position,
                "color": color,
                "is_initial": is_initial,
                "is_terminal": is_terminal,
                "is_cancelled": is_cancelled,
                "is_active": is_active,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

fn ensure_single_status_marker(
    is_initial: bool,
    is_terminal: bool,
    is_cancelled: bool,
) -> Result<()> {
    let marker_count = [is_initial, is_terminal, is_cancelled]
        .into_iter()
        .filter(|selected| *selected)
        .count();
    if marker_count > 1 {
        bail!("a status can only be initial, terminal, cancelled, or none");
    }
    Ok(())
}

pub async fn delete_concept_status(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    let in_use = state
        .project_concepts
        .find_one(doc! { "company_id": company_id, "status_id": id })
        .await?
        .is_some();
    if in_use {
        state
            .concept_statuses
            .update_one(
                doc! { "_id": id, "company_id": company_id },
                doc! { "$set": { "is_active": false, "updated_at": DateTime::from_system_time(SystemTime::now()) } },
            )
            .await?;
    } else {
        state
            .concept_statuses
            .delete_one(doc! { "_id": id, "company_id": company_id })
            .await?;
    }
    Ok(())
}

pub async fn list_project_concepts(
    state: &AppState,
    company_id: &ObjectId,
    project_id: &ObjectId,
) -> Result<Vec<ProjectConcept>> {
    let mut cursor = state
        .project_concepts
        .find(doc! { "company_id": company_id, "project_id": project_id })
        .sort(doc! { "position": 1, "created_at": 1 })
        .await?;
    let mut items = Vec::new();
    while let Some(concept) = cursor.try_next().await? {
        items.push(concept);
    }
    Ok(items)
}

pub async fn get_project_concept_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ProjectConcept>> {
    state
        .project_concepts
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn create_project_concept(
    state: &AppState,
    company_id: &ObjectId,
    project_id: &ObjectId,
    status_id: Option<ObjectId>,
    name: &str,
    quantity: f64,
    unit: Option<String>,
    description: Option<String>,
    estimated_hours: Option<f64>,
    estimated_cost: Option<f64>,
    notes: Option<String>,
    position: i32,
) -> Result<ObjectId> {
    let name = name.trim();
    if name.is_empty() {
        bail!("project concept name is required");
    }
    if quantity <= 0.0 {
        bail!("project concept quantity must be greater than zero");
    }
    get_project_by_id_for_company(state, project_id, company_id)
        .await?
        .context("project not found for company")?;

    let status_id = match status_id {
        Some(id) => {
            get_concept_status_by_id_for_company(state, &id, company_id)
                .await?
                .context("concept status not found for company")?;
            id
        }
        None => get_initial_concept_status(state, company_id)
            .await?
            .context("initial concept status not configured")?
            .id
            .context("initial concept status missing _id")?,
    };

    let res = state
        .project_concepts
        .insert_one(ProjectConcept {
            id: None,
            company_id: company_id.clone(),
            project_id: project_id.clone(),
            status_id,
            name: name.to_string(),
            quantity,
            unit,
            description,
            estimated_hours,
            estimated_cost,
            notes,
            position,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("project concept insert missing _id")
}

pub async fn update_project_concept(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    status_id: &ObjectId,
    name: &str,
    quantity: f64,
    unit: Option<String>,
    description: Option<String>,
    estimated_hours: Option<f64>,
    estimated_cost: Option<f64>,
    notes: Option<String>,
    position: i32,
) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("project concept name is required");
    }
    if quantity <= 0.0 {
        bail!("project concept quantity must be greater than zero");
    }
    get_concept_status_by_id_for_company(state, status_id, company_id)
        .await?
        .context("concept status not found for company")?;

    state
        .project_concepts
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "status_id": status_id,
                "name": name,
                "quantity": quantity,
                "unit": unit,
                "description": description,
                "estimated_hours": estimated_hours,
                "estimated_cost": estimated_cost,
                "notes": notes,
                "position": position,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    Ok(())
}

pub async fn advance_project_concept_status(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ConceptStatus>> {
    let concept = match get_project_concept_by_id_for_company(state, id, company_id).await? {
        Some(concept) => concept,
        None => return Ok(None),
    };
    let current = get_concept_status_by_id_for_company(state, &concept.status_id, company_id)
        .await?
        .context("current concept status not found")?;
    let next = state
        .concept_statuses
        .find_one(doc! {
            "company_id": company_id,
            "is_active": true,
            "position": { "$gt": current.position }
        })
        .sort(doc! { "position": 1 })
        .await?;
    let next = match next {
        Some(status) => status,
        None => return Ok(Some(current)),
    };
    let next_id = next.id.clone().context("next concept status missing _id")?;
    state
        .project_concepts
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": { "status_id": next_id, "updated_at": DateTime::from_system_time(SystemTime::now()) } },
        )
        .await?;
    Ok(Some(next))
}

pub async fn delete_project_concept(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    let has_allocations = state
        .resource_usage_allocations
        .find_one(doc! { "company_id": company_id, "concept_id": id })
        .await?
        .is_some();
    if has_allocations {
        bail!("cannot delete concept with resource usage allocations");
    }
    state
        .project_concepts
        .delete_one(doc! { "_id": id, "company_id": company_id })
        .await?;
    Ok(())
}

pub async fn project_status_summary_by_quantity(
    state: &AppState,
    company_id: &ObjectId,
    project_id: &ObjectId,
) -> Result<Vec<ProjectStatusSummaryItem>> {
    let statuses = list_concept_statuses(state, company_id).await?;
    let concepts = list_project_concepts(state, company_id, project_id).await?;
    let mut out = Vec::new();
    for status in statuses {
        let Some(status_id) = status.id.clone() else {
            continue;
        };
        let matching: Vec<&ProjectConcept> = concepts
            .iter()
            .filter(|concept| concept.status_id == status_id)
            .collect();
        let quantity: f64 = matching.iter().map(|concept| concept.quantity).sum();
        if quantity > 0.0 {
            let unit = matching.iter().find_map(|concept| concept.unit.clone());
            out.push(ProjectStatusSummaryItem {
                status_id: status_id.to_hex(),
                name: status.name,
                position: status.position,
                quantity,
                unit,
            });
        }
    }
    Ok(out)
}
