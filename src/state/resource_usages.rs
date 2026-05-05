use anyhow::{Context, Result, bail};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};

use crate::models::{ProjectConcept, ProjectStatus, ResourceUsage, ResourceUsageAllocation};

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

pub async fn list_active_project_concepts_for_status(
    state: &AppState,
    company_id: &ObjectId,
    status_id: &ObjectId,
) -> Result<Vec<ProjectConcept>> {
    list_active_project_concepts_for_scope(state, company_id, Some(status_id)).await
}

pub async fn list_active_project_concepts(
    state: &AppState,
    company_id: &ObjectId,
) -> Result<Vec<ProjectConcept>> {
    list_active_project_concepts_for_scope(state, company_id, None).await
}

async fn list_active_project_concepts_for_scope(
    state: &AppState,
    company_id: &ObjectId,
    status_id: Option<&ObjectId>,
) -> Result<Vec<ProjectConcept>> {
    let mut filter = doc! { "company_id": company_id };
    if let Some(status_id) = status_id {
        filter.insert("status_id", status_id);
    }
    let mut cursor = state
        .project_concepts
        .find(filter)
        .sort(doc! { "status_id": 1, "position": 1, "created_at": 1 })
        .await?;
    let mut items = Vec::new();
    while let Some(concept) = cursor.try_next().await? {
        let project = get_project_by_id_for_company(state, &concept.project_id, company_id).await?;
        let status = state
            .concept_statuses
            .find_one(doc! { "_id": &concept.status_id, "company_id": company_id })
            .await?;
        if project
            .as_ref()
            .is_some_and(|project| project.status != ProjectStatus::Cancelado)
            && status
                .as_ref()
                .is_some_and(|status| !status.is_terminal && !status.is_cancelled)
        {
            items.push(concept);
        }
    }
    Ok(items)
}

pub async fn replace_hourly_resource_usage_grid(
    state: &AppState,
    company_id: &ObjectId,
    status_id: Option<&ObjectId>,
    date_start: DateTime,
    start_hour: i32,
    end_hour: i32,
    selections: &[(ObjectId, i32, ObjectId)],
) -> Result<()> {
    let target_concepts =
        list_active_project_concepts_for_scope(state, company_id, status_id).await?;
    let target_concept_ids: HashSet<ObjectId> = target_concepts
        .iter()
        .filter_map(|concept| concept.id.clone())
        .collect();
    let target_concepts_by_id: HashMap<ObjectId, ProjectConcept> = target_concepts
        .into_iter()
        .filter_map(|concept| {
            let id = concept.id.clone()?;
            Some((id, concept))
        })
        .collect();

    let mut desired: HashMap<(i32, ObjectId), HashSet<ObjectId>> = HashMap::new();
    for (concept_id, hour, resource_id) in selections {
        if *hour < start_hour || *hour >= end_hour || !target_concept_ids.contains(concept_id) {
            continue;
        }
        let Some(concept) = target_concepts_by_id.get(concept_id) else {
            continue;
        };
        let Some(resource) = get_resource_by_id_for_company(state, resource_id, company_id).await?
        else {
            continue;
        };
        if !resource
            .allowed_status_ids
            .iter()
            .any(|allowed_status_id| allowed_status_id == &concept.status_id)
        {
            continue;
        }
        desired
            .entry((*hour, resource_id.clone()))
            .or_default()
            .insert(concept_id.clone());
    }

    for hour in start_hour..end_hour {
        let started_at = add_hours(date_start, hour)?;
        let ended_at = add_hours(date_start, hour + 1)?;
        let mut resources_to_process: HashSet<ObjectId> = desired
            .keys()
            .filter(|(desired_hour, _)| *desired_hour == hour)
            .map(|(_, resource_id)| resource_id.clone())
            .collect();

        let existing_usages =
            list_resource_usages_for_slot(state, company_id, started_at, ended_at).await?;
        for usage in &existing_usages {
            let Some(usage_id) = usage.id.clone() else {
                continue;
            };
            let allocations = list_resource_usage_allocations(state, company_id, &usage_id).await?;
            if allocations
                .iter()
                .any(|allocation| target_concept_ids.contains(&allocation.concept_id))
            {
                resources_to_process.insert(usage.resource_id.clone());
            }
        }

        for resource_id in resources_to_process {
            let Some(resource) =
                get_resource_by_id_for_company(state, &resource_id, company_id).await?
            else {
                continue;
            };
            let existing_usage =
                get_resource_usage_for_slot(state, company_id, &resource_id, started_at, ended_at)
                    .await?;

            let mut final_concepts: HashSet<ObjectId> = HashSet::new();
            let existing_usage_id = existing_usage.as_ref().and_then(|usage| usage.id.clone());
            if let Some(usage_id) = &existing_usage_id {
                for allocation in
                    list_resource_usage_allocations(state, company_id, usage_id).await?
                {
                    let resource_is_editable_for_concept = target_concepts_by_id
                        .get(&allocation.concept_id)
                        .is_some_and(|concept| {
                            resource
                                .allowed_status_ids
                                .iter()
                                .any(|allowed_status_id| allowed_status_id == &concept.status_id)
                        });
                    if !target_concept_ids.contains(&allocation.concept_id)
                        || !resource_is_editable_for_concept
                    {
                        final_concepts.insert(allocation.concept_id);
                    }
                }
            }

            if let Some(selected) = desired.get(&(hour, resource_id.clone())) {
                final_concepts.extend(selected.iter().cloned());
            }

            if final_concepts.is_empty() {
                if let Some(usage_id) = existing_usage_id {
                    delete_resource_usage(state, &usage_id, company_id).await?;
                }
                continue;
            }

            let usage_id = if let Some(usage_id) = existing_usage_id {
                usage_id
            } else {
                create_resource_usage(
                    state,
                    company_id,
                    &resource_id,
                    started_at,
                    Some(ended_at),
                    None,
                    Some("Registro horario".to_string()),
                )
                .await?
            };

            let concept_ids: Vec<ObjectId> = final_concepts.into_iter().collect();
            replace_resource_usage_allocations_equal(state, company_id, &usage_id, &concept_ids)
                .await?;
        }
    }

    Ok(())
}

pub async fn list_resource_usages_for_slot(
    state: &AppState,
    company_id: &ObjectId,
    started_at: DateTime,
    ended_at: DateTime,
) -> Result<Vec<ResourceUsage>> {
    let mut cursor = state
        .resource_usages
        .find(doc! {
            "company_id": company_id,
            "started_at": started_at,
            "ended_at": ended_at,
        })
        .await?;
    let mut items = Vec::new();
    while let Some(usage) = cursor.try_next().await? {
        items.push(usage);
    }
    Ok(items)
}

pub async fn get_resource_usage_for_slot(
    state: &AppState,
    company_id: &ObjectId,
    resource_id: &ObjectId,
    started_at: DateTime,
    ended_at: DateTime,
) -> Result<Option<ResourceUsage>> {
    state
        .resource_usages
        .find_one(doc! {
            "company_id": company_id,
            "resource_id": resource_id,
            "started_at": started_at,
            "ended_at": ended_at,
        })
        .await
        .map_err(Into::into)
}

fn add_hours(date_start: DateTime, hour: i32) -> Result<DateTime> {
    if !(0..=24).contains(&hour) {
        bail!("hour must be between 0 and 24");
    }
    Ok(DateTime::from_millis(
        date_start.timestamp_millis() + i64::from(hour) * 3_600_000,
    ))
}
