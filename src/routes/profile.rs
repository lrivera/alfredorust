use std::{collections::HashSet, sync::Arc};

use axum::{extract::State, http::StatusCode, Json};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use serde::Serialize;
use slug::slugify;

use crate::{session::SessionUser, state::AppState};

#[derive(Serialize)]
pub struct CompanySummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub active: bool,
}

pub async fn me_companies(
    session: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CompanySummary>>, StatusCode> {
    let active_company = session.active_company_id().clone();

    // Always fetch fresh memberships to reflect changes done after login.
    let mut company_ids: HashSet<ObjectId> =
        session.user().company_ids.iter().cloned().collect();

    // print user_id
    println!("User ID: {}", session.user_id());

    if let Ok(mut memberships) = state
        .user_companies
        .find(doc! { "user_id": session.user_id() })
        .await
    {
        while let Some(m) = memberships
            .try_next()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            company_ids.insert(m.company_id);
            println!("Found membership for company ID: {}", m.company_id);
        }
    }

    if let Some(primary) = session.user().company_ids.first() {
        company_ids.insert(primary.clone());
    }

    let ids: Vec<ObjectId> = company_ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(Json(vec![]));
    }

    let mut cursor = state
        .companies
        .find(doc! { "_id": { "$in": &ids } })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut companies = Vec::new();
    while let Some(company) = cursor
        .try_next()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let slug = if company.slug.is_empty() {
            slugify(&company.name)
        } else {
            company.slug.clone()
        };
        let active = company
            .id
            .as_ref()
            .map(|cid| cid == &active_company)
            .unwrap_or(false);
        companies.push(CompanySummary {
            id: company.id.unwrap().to_hex(),
            name: company.name,
            slug,
            active,
        });
    }

    // Sort by name for stable UX
    companies.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(Json(companies))
}
