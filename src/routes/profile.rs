use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use mongodb::bson::doc;
use serde::Serialize;

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
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<CompanySummary>>, StatusCode> {
    let active_slug = session.active_company_slug().to_string();
    let companies = session
        .user()
        .company_ids
        .iter()
        .enumerate()
        .map(|(idx, _)| CompanySummary {
            id: session.user().company_ids[idx].to_hex(),
            name: session
                .user()
                .company_names
                .get(idx)
                .cloned()
                .unwrap_or_default(),
            slug: session
                .user()
                .company_slugs
                .get(idx)
                .cloned()
                .unwrap_or_default(),
            active: session
                .user()
                .company_slugs
                .get(idx)
                .map(|s| s.eq_ignore_ascii_case(&active_slug))
                .unwrap_or(false),
        })
        .collect();

    Ok(Json(companies))
}
