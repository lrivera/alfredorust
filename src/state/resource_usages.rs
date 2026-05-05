use anyhow::{Context, Result, bail};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::time::SystemTime;

use crate::models::{ResourceUsage, ResourceUsageAllocation};

use super::{
    AppState, get_project_by_id_for_company, get_project_concept_by_id_for_company,
    get_resource_by_id_for_company,
};

fn usage_totals(
    started_at: DateTime,
    ended_at: Option<DateTime>,
    hourly_cost: f64,
) -> Result<(Option<f64>, Option<f64>)> {
    let Some(ended_at) = ended_at else {
        return Ok((None, None));
    };
    if ended_at < started_at {
        bail!("ended_at cannot be before started_at");
    }
    let duration = ended_at
        .to_chrono()
        .signed_duration_since(started_at.to_chrono());
    let hours = duration.num_milliseconds() as f64 / 3_600_000.0;
    Ok((Some(hours), Some(hours * hourly_cost)))
}

fn allocation_totals(usage: &ResourceUsage, ratio: f64) -> (Option<f64>, Option<f64>) {
    (
        usage.duration_hours.map(|hours| hours * ratio),
        usage.total_cost.map(|cost| cost * ratio),
    )
}

pub async fn list_resource_usages(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Vec<ResourceUsage>> {
    let mut cursor = state
        .resource_usages
        .find(doc! { "company_id": company_id })
        .sort(doc! { "started_at": -1 })
        .await?;
    let mut items = Vec::new();
    while let Some(usage) = cursor.try_next().await? {
        items.push(usage);
    }
    Ok(items)
}

pub async fn get_resource_usage_by_id_for_company(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<Option<ResourceUsage>> {
    state
        .resource_usages
        .find_one(doc! { "_id": id, "company_id": company_id })
        .await
        .map_err(Into::into)
}

pub async fn create_resource_usage(
    state: &AppState,
    company_id: &ObjectId,
    resource_id: &ObjectId,
    started_at: DateTime,
    ended_at: Option<DateTime>,
    operator_name: Option<String>,
    notes: Option<String>,
) -> Result<ObjectId> {
    let resource = get_resource_by_id_for_company(state, resource_id, company_id)
        .await?
        .context("resource not found for company")?;
    let (duration_hours, total_cost) = usage_totals(started_at, ended_at, resource.hourly_cost)?;
    let res = state
        .resource_usages
        .insert_one(ResourceUsage {
            id: None,
            company_id: company_id.clone(),
            resource_id: resource_id.clone(),
            resource_name_snapshot: resource.name,
            started_at,
            ended_at,
            duration_hours,
            hourly_cost_snapshot: resource.hourly_cost,
            total_cost,
            currency: resource.currency,
            operator_name,
            notes,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("resource usage insert missing _id")
}

pub async fn update_resource_usage(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
    started_at: DateTime,
    ended_at: Option<DateTime>,
    hourly_cost_snapshot: f64,
    operator_name: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let (duration_hours, total_cost) = usage_totals(started_at, ended_at, hourly_cost_snapshot)?;
    state
        .resource_usages
        .update_one(
            doc! { "_id": id, "company_id": company_id },
            doc! { "$set": {
                "started_at": started_at,
                "ended_at": ended_at,
                "duration_hours": duration_hours,
                "hourly_cost_snapshot": hourly_cost_snapshot.max(0.0),
                "total_cost": total_cost,
                "operator_name": operator_name,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now()),
            } },
        )
        .await?;
    recalculate_resource_usage_allocations(state, id, company_id).await?;
    Ok(())
}

pub async fn list_resource_usage_allocations(
    state: &AppState,
    company_id: &ObjectId,
    usage_id: &ObjectId,
) -> Result<Vec<ResourceUsageAllocation>> {
    let mut cursor = state
        .resource_usage_allocations
        .find(doc! { "company_id": company_id, "usage_id": usage_id })
        .await?;
    let mut items = Vec::new();
    while let Some(allocation) = cursor.try_next().await? {
        items.push(allocation);
    }
    Ok(items)
}

pub async fn list_resource_usage_allocations_for_concept(
    state: &AppState,
    company_id: &ObjectId,
    concept_id: &ObjectId,
) -> Result<Vec<ResourceUsageAllocation>> {
    let mut cursor = state
        .resource_usage_allocations
        .find(doc! { "company_id": company_id, "concept_id": concept_id })
        .await?;
    let mut items = Vec::new();
    while let Some(allocation) = cursor.try_next().await? {
        items.push(allocation);
    }
    Ok(items)
}

pub async fn replace_resource_usage_allocations_equal(
    state: &AppState,
    company_id: &ObjectId,
    usage_id: &ObjectId,
    concept_ids: &[ObjectId],
) -> Result<Vec<ObjectId>> {
    if concept_ids.is_empty() {
        bail!("at least one concept allocation is required");
    }
    let ratio = 1.0 / concept_ids.len() as f64;
    let allocations: Vec<(ObjectId, f64, Option<String>)> = concept_ids
        .iter()
        .cloned()
        .map(|concept_id| (concept_id, ratio, None))
        .collect();
    replace_resource_usage_allocations(state, company_id, usage_id, &allocations).await
}

pub async fn replace_resource_usage_allocations(
    state: &AppState,
    company_id: &ObjectId,
    usage_id: &ObjectId,
    allocations: &[(ObjectId, f64, Option<String>)],
) -> Result<Vec<ObjectId>> {
    if allocations.is_empty() {
        bail!("at least one allocation is required");
    }
    let ratio_sum: f64 = allocations.iter().map(|(_, ratio, _)| *ratio).sum();
    if (ratio_sum - 1.0).abs() > 0.0001 {
        bail!("allocation ratios must sum to 1.0");
    }
    if allocations.iter().any(|(_, ratio, _)| *ratio <= 0.0) {
        bail!("allocation ratios must be greater than zero");
    }

    let usage = get_resource_usage_by_id_for_company(state, usage_id, company_id)
        .await?
        .context("resource usage not found for company")?;

    let mut docs = Vec::new();
    for (concept_id, ratio, notes) in allocations {
        let concept = get_project_concept_by_id_for_company(state, concept_id, company_id)
            .await?
            .context("project concept not found for company")?;
        get_project_by_id_for_company(state, &concept.project_id, company_id)
            .await?
            .context("project not found for company")?;
        let (allocated_hours, allocated_cost) = allocation_totals(&usage, *ratio);
        docs.push(ResourceUsageAllocation {
            id: None,
            company_id: company_id.clone(),
            usage_id: usage_id.clone(),
            project_id: concept.project_id,
            concept_id: concept_id.clone(),
            allocation_ratio: *ratio,
            allocated_hours,
            allocated_cost,
            notes: notes.clone(),
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
        });
    }

    state
        .resource_usage_allocations
        .delete_many(doc! { "company_id": company_id, "usage_id": usage_id })
        .await?;

    let mut ids = Vec::new();
    for allocation in docs {
        let res = state
            .resource_usage_allocations
            .insert_one(allocation)
            .await?;
        ids.push(
            res.inserted_id
                .as_object_id()
                .context("resource usage allocation insert missing _id")?,
        );
    }
    Ok(ids)
}

pub async fn recalculate_resource_usage_allocations(
    state: &AppState,
    usage_id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    let usage = get_resource_usage_by_id_for_company(state, usage_id, company_id)
        .await?
        .context("resource usage not found for company")?;
    let allocations = list_resource_usage_allocations(state, company_id, usage_id).await?;
    for allocation in allocations {
        let Some(allocation_id) = allocation.id else {
            continue;
        };
        let (allocated_hours, allocated_cost) =
            allocation_totals(&usage, allocation.allocation_ratio);
        state
            .resource_usage_allocations
            .update_one(
                doc! { "_id": allocation_id, "company_id": company_id },
                doc! { "$set": {
                    "allocated_hours": allocated_hours,
                    "allocated_cost": allocated_cost,
                    "updated_at": DateTime::from_system_time(SystemTime::now()),
                } },
            )
            .await?;
    }
    Ok(())
}

pub async fn delete_resource_usage(
    state: &AppState,
    id: &ObjectId,
    company_id: &ObjectId,
) -> Result<()> {
    state
        .resource_usage_allocations
        .delete_many(doc! { "company_id": company_id, "usage_id": id })
        .await?;
    state
        .resource_usages
        .delete_one(doc! { "_id": id, "company_id": company_id })
        .await?;
    Ok(())
}
