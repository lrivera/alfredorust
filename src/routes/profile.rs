use std::{collections::HashSet, sync::Arc};

use axum::{Json, extract::State, http::StatusCode};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use serde::Serialize;
use slug::slugify;

use crate::{session::SessionUser, state::AppState};

#[derive(Serialize, utoipa::ToSchema)]
pub struct CompanySummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub active: bool,
}

/// Consolidated bootstrap payload: profile + active-tenant role/permissions +
/// the companies the user belongs to. Never includes the TOTP secret.
#[derive(Serialize, utoipa::ToSchema)]
pub struct MeResponse {
    pub email: String,
    pub company: String,
    pub company_slug: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub companies: Vec<CompanySummary>,
}

/// Collects the companies the session user belongs to, marking the active one
/// (resolved from the request host by the session middleware). Shared by
/// `GET /api/me/companies` and `GET /api/me`.
async fn collect_companies(
    session: &SessionUser,
    state: &AppState,
) -> Result<Vec<CompanySummary>, StatusCode> {
    let active_company = session.active_company_id().clone();

    // Always fetch fresh memberships to reflect changes done after login.
    let mut company_ids: HashSet<ObjectId> = session.user().company_ids.iter().cloned().collect();

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
        }
    }

    if let Some(primary) = session.user().company_ids.first() {
        company_ids.insert(primary.clone());
    }

    let ids: Vec<ObjectId> = company_ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(vec![]);
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

    Ok(companies)
}

#[utoipa::path(
    get,
    path = "/api/me/companies",
    tag = "auth",
    responses(
        (status = 200, description = "Returns the companies the user belongs to"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn me_companies(
    session: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CompanySummary>>, StatusCode> {
    let companies = collect_companies(&session, &state).await?;
    Ok(Json(companies))
}

/// Single bootstrap call for the SPA: current profile, active-tenant role and
/// permissions, and company memberships. The TOTP secret is never returned.
#[utoipa::path(
    get,
    path = "/api/me",
    tag = "auth",
    responses(
        (status = 200, description = "Returns the current user's profile, permissions, and companies", body = MeResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn me(
    session: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<MeResponse>, StatusCode> {
    let companies = collect_companies(&session, &state).await?;
    let current = session.user();
    let permissions = current
        .permissions
        .iter()
        .map(|permission| permission.as_str().to_string())
        .collect::<Vec<_>>();

    Ok(Json(MeResponse {
        email: current.email.clone(),
        company: current.company_name.clone(),
        company_slug: current.company_slug.clone(),
        role: current.role.as_str().to_string(),
        permissions,
        companies,
    }))
}
