use std::{collections::HashSet, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::doc;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use slug::slugify;

#[allow(unused_imports)]
use crate::filters;

use super::sat_configs::{SatConfigRow, load_sat_configs_for_company};
use crate::{
    models::UserRole,
    session::SessionUser,
    state::{
        AppState, add_user_to_company, create_company, delete_company, get_company_by_id,
        list_companies, update_company,
    },
};

use super::finance::helpers::require_admin_active;

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/companies/index.html")]
struct CompaniesIndexTemplate {
    companies: Vec<CompanyRow>,
}

struct CompanyRow {
    id: String,
    name: String,
    default_currency: String,
    is_active: bool,
    is_current: bool,
}

#[derive(Serialize)]
struct CompanyUpdateResponse {
    slug: String,
}

#[derive(Serialize)]
pub struct CompanyData {
    id: String,
    name: String,
    slug: String,
    default_currency: String,
    is_active: bool,
    notes: Option<String>,
    is_current: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CompanyPayload {
    name: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    default_currency: Option<String>,
    #[serde(default)]
    is_active: Option<bool>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/companies/form.html")]
struct CompanyFormTemplate {
    action: String,
    name: String,
    slug: String,
    default_currency: String,
    is_active: bool,
    notes: String,
    is_edit: bool,
    errors: Option<String>,
    is_current: bool,
    company_id: String,
    sat_configs: Vec<SatConfigRow>,
}

#[derive(Deserialize)]
pub(crate) struct CompanyFormData {
    name: String,
    #[serde(default)]
    slug: Option<String>,
    default_currency: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    notes: Option<String>,
}

fn has_admin_role_for(session_user: &SessionUser, company_id: &ObjectId) -> bool {
    session_user
        .user()
        .company_ids
        .iter()
        .zip(session_user.user().company_roles.iter())
        .any(|(cid, role)| cid == company_id && role.is_admin())
}

fn company_data(
    session_user: &SessionUser,
    company: crate::models::Company,
) -> Option<CompanyData> {
    let id = company.id?;
    Some(CompanyData {
        id: id.to_hex(),
        name: company.name,
        slug: company.slug,
        default_currency: company.default_currency,
        is_active: company.is_active,
        notes: company.notes,
        is_current: &id == session_user.active_company_id(),
    })
}

fn parse_company_payload(
    payload: CompanyPayload,
) -> Result<(String, String, String, bool, Option<String>), StatusCode> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let slug_raw = payload.slug.unwrap_or_default();
    let slug = slug_raw.trim().to_string();
    validate_slug(&slug).map_err(|_| StatusCode::BAD_REQUEST)?;
    let final_slug = if slug.is_empty() {
        slugify(&name)
    } else {
        slug
    };
    let default_currency = payload
        .default_currency
        .as_deref()
        .map(str::trim)
        .filter(|currency| !currency.is_empty())
        .unwrap_or("MXN")
        .to_string();
    let is_active = payload.is_active.unwrap_or(true);
    let notes = payload.notes.and_then(|notes| {
        let trimmed = notes.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    Ok((name, final_slug, default_currency, is_active, notes))
}

async fn slug_conflicts(
    state: &AppState,
    slug: &str,
    current_id: Option<&ObjectId>,
) -> Result<bool, StatusCode> {
    match state
        .companies
        .find_one(doc! { "slug": slug })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Some(existing) => Ok(existing.id.as_ref() != current_id),
        None => Ok(false),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/companies",
    tag = "admin",
    responses(
        (status = 200, description = "List of companies"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn companies_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CompanyData>>, StatusCode> {
    require_admin_active(&session_user)?;
    let allowed: HashSet<_> = session_user.user().company_ids.iter().cloned().collect();
    let companies = list_companies(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|company| company.id.as_ref().is_some_and(|id| allowed.contains(id)))
        .filter_map(|company| company_data(&session_user, company))
        .collect();
    Ok(Json(companies))
}

#[utoipa::path(
    get,
    path = "/api/admin/companies/{id}",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Company detail"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn company_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<CompanyData>, StatusCode> {
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !has_admin_role_for(&session_user, &object_id) {
        return Err(StatusCode::FORBIDDEN);
    }
    let company = get_company_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    company_data(&session_user, company)
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    post,
    path = "/api/admin/companies",
    tag = "admin",
    request_body = CompanyPayload,
    responses(
        (status = 201, description = "Company created"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn company_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompanyPayload>,
) -> impl IntoResponse {
    if let Err(status) = require_admin_active(&session_user) {
        return status.into_response();
    }
    let (name, slug, default_currency, is_active, notes) = match parse_company_payload(payload) {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    match slug_conflicts(&state, &slug, None).await {
        Ok(true) => return StatusCode::CONFLICT.into_response(),
        Ok(false) => {}
        Err(status) => return status.into_response(),
    }
    match create_company(&state, &name, &slug, &default_currency, is_active, notes).await {
        Ok(company_id) => {
            match add_user_to_company(&state, session_user.user_id(), &company_id, UserRole::Admin)
                .await
            {
                Ok(_) => (
                    StatusCode::CREATED,
                    Json(serde_json::json!({ "id": company_id.to_hex() })),
                )
                    .into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/companies/{id}/update",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    request_body = CompanyPayload,
    responses(
        (status = 200, description = "Company updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn company_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<CompanyPayload>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if get_company_by_id(&state, &object_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (name, slug, default_currency, is_active, notes) = match parse_company_payload(payload) {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    match slug_conflicts(&state, &slug, Some(&object_id)).await {
        Ok(true) => return StatusCode::CONFLICT.into_response(),
        Ok(false) => {}
        Err(status) => return status.into_response(),
    }
    match update_company(
        &state,
        &object_id,
        &name,
        &slug,
        &default_currency,
        is_active,
        notes,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true, "slug": slug })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/companies/{id}/delete",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "Company deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden — not an admin, or the active company"),
        (status = 400, description = "Invalid id")
    ),
    security(("session" = []))
)]
pub async fn company_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    // Mirror the HTML handler: never delete the session's active company.
    if &object_id == session_user.active_company_id() {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match delete_company(&state, &object_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/companies/{id}/cfdis/delete_all",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "All CFDIs deleted; returns the count"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid id")
    ),
    security(("session" = []))
)]
pub async fn company_cfdis_delete_all_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // CFDIs store `company_id` as the hex string (see cfdis insertion).
    match state.cfdis.delete_many(doc! { "company_id": &id }).await {
        Ok(res) => Json(serde_json::json!({ "ok": true, "deleted": res.deleted_count }))
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/companies/{id}/transactions/delete_all",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "All transactions deleted; returns the count"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid id")
    ),
    security(("session" = []))
)]
pub async fn company_transactions_delete_all_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Transactions store `company_id` as an ObjectId (see finance inserts).
    match state
        .transactions
        .delete_many(doc! { "company_id": object_id })
        .await
    {
        Ok(res) => Json(serde_json::json!({ "ok": true, "deleted": res.deleted_count }))
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let allowed: HashSet<_> = session_user.user().company_ids.iter().cloned().collect();
    let active_id = session_user.active_company_id().clone();

    let companies = list_companies(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter_map(|company| {
            if let Some(id) = &company.id {
                if !allowed.contains(id) {
                    return None;
                }
            }
            company.id.map(|id| CompanyRow {
                is_current: &id == &active_id,
                id: id.to_hex(),
                name: company.name,
                default_currency: company.default_currency,
                is_active: company.is_active,
            })
        })
        .collect();

    render(CompaniesIndexTemplate { companies })
}

pub async fn companies_new(
    _session_user: SessionUser,
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    render(CompanyFormTemplate {
        action: "/admin/companies".into(),
        name: String::new(),
        slug: String::new(),
        default_currency: "MXN".into(),
        is_active: true,
        notes: String::new(),
        is_edit: false,
        errors: None,
        is_current: false,
        company_id: String::new(),
        sat_configs: vec![],
    })
}

pub async fn companies_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CompanyFormData>,
) -> impl IntoResponse {
    let name = form.name.trim();
    let is_active = form.is_active;
    let slug_raw = form.slug.unwrap_or_default();
    let slug_val = slug_raw.trim();
    if let Err(msg) = validate_slug(slug_val) {
        return render(CompanyFormTemplate {
            action: "/admin/companies".into(),
            name: name.to_string(),
            slug: slug_val.to_string(),
            default_currency: form.default_currency.clone(),
            is_active,
            notes: form.notes.clone().unwrap_or_default(),
            is_edit: false,
            errors: Some(msg),
            is_current: false,
            company_id: String::new(),
            sat_configs: vec![],
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }
    let default_currency = form.default_currency.trim();
    let notes = form.notes.as_ref().and_then(|n| {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    if name.is_empty() {
        return render(CompanyFormTemplate {
            action: "/admin/companies".into(),
            name: String::new(),
            slug: slug_val.to_string(),
            default_currency: default_currency.to_string(),
            is_active,
            notes: String::new(),
            is_edit: false,
            errors: Some("El nombre es obligatorio".into()),
            is_current: false,
            company_id: String::new(),
            sat_configs: vec![],
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }

    let currency = if default_currency.is_empty() {
        "MXN"
    } else {
        default_currency
    };
    let final_slug = if slug_val.is_empty() {
        slugify(name)
    } else {
        slug_val.to_string()
    };

    // Ensure slug uniqueness
    if let Ok(Some(existing)) = state.companies.find_one(doc! { "slug": &final_slug }).await {
        if existing.id.is_some() {
            return render(CompanyFormTemplate {
                action: "/admin/companies".into(),
                name: name.to_string(),
                slug: slug_val.to_string(),
                default_currency: form.default_currency.clone(),
                is_active,
                notes: form.notes.clone().unwrap_or_default(),
                is_edit: false,
                errors: Some("Ya existe una compañía con ese slug.".into()),
                is_current: false,
                company_id: String::new(),
                sat_configs: vec![],
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    }

    match create_company(
        &state,
        name,
        final_slug.as_str(),
        currency,
        is_active,
        notes,
    )
    .await
    {
        Ok(company_id) => {
            match add_user_to_company(&state, session_user.user_id(), &company_id, UserRole::Admin)
                .await
            {
                Ok(_) => Redirect::to("/admin/companies").into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    if !has_admin_role_for(&session_user, &object_id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let company = get_company_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let sat_configs = load_sat_configs_for_company(&state, &object_id).await;

    render(CompanyFormTemplate {
        action: format!("/admin/companies/{}/update", id),
        name: company.name,
        slug: company.slug,
        default_currency: company.default_currency,
        is_active: company.is_active,
        notes: company.notes.unwrap_or_default(),
        is_edit: true,
        errors: None,
        is_current: company.id.as_ref() == Some(session_user.active_company_id()),
        company_id: id.clone(),
        sat_configs,
    })
}

pub async fn companies_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<CompanyFormData>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let name = form.name.trim();
    let is_active = form.is_active;
    let slug_raw = form.slug.unwrap_or_default();
    let slug_val = slug_raw.trim();
    if let Err(msg) = validate_slug(slug_val) {
        return render(CompanyFormTemplate {
            action: format!("/admin/companies/{}/update", id),
            name: name.to_string(),
            slug: slug_val.to_string(),
            default_currency: form.default_currency.clone(),
            is_active,
            notes: form.notes.clone().unwrap_or_default(),
            is_edit: true,
            errors: Some(msg),
            is_current: &object_id == session_user.active_company_id(),
            company_id: id.clone(),
            sat_configs: load_sat_configs_for_company(&state, &object_id).await,
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }
    let default_currency = form.default_currency.trim();
    let notes = form.notes.as_ref().and_then(|n| {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    if name.is_empty() {
        return render(CompanyFormTemplate {
            action: format!("/admin/companies/{}/update", id),
            name: String::new(),
            slug: slug_val.to_string(),
            default_currency: default_currency.to_string(),
            is_active,
            notes: String::new(),
            is_edit: true,
            errors: Some("El nombre es obligatorio".into()),
            is_current: &object_id == session_user.active_company_id(),
            company_id: id.clone(),
            sat_configs: load_sat_configs_for_company(&state, &object_id).await,
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }

    let currency = if default_currency.is_empty() {
        "MXN"
    } else {
        default_currency
    };
    let final_slug = if slug_val.is_empty() {
        slugify(name)
    } else {
        slug_val.to_string()
    };

    // Ensure slug uniqueness against other companies
    if let Ok(Some(existing)) = state.companies.find_one(doc! { "slug": &final_slug }).await {
        if existing.id != Some(object_id.clone()) {
            return render(CompanyFormTemplate {
                action: format!("/admin/companies/{}/update", id),
                name: name.to_string(),
                slug: slug_val.to_string(),
                default_currency: form.default_currency.clone(),
                is_active,
                notes: form.notes.clone().unwrap_or_default(),
                is_edit: true,
                errors: Some("Ya existe otra compañía con ese slug.".into()),
                is_current: &object_id == session_user.active_company_id(),
                company_id: id.clone(),
                sat_configs: load_sat_configs_for_company(&state, &object_id).await,
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    }

    match update_company(
        &state, &object_id, name, slug_val, currency, is_active, notes,
    )
    .await
    {
        Ok(_) => {
            if &object_id == session_user.active_company_id() {
                return axum::Json(CompanyUpdateResponse { slug: final_slug }).into_response();
            }
            Redirect::to("/admin/companies").into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // Do not allow deleting the currently active company for this session.
    if &object_id == session_user.active_company_id() {
        return StatusCode::FORBIDDEN.into_response();
    }

    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match delete_company(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/companies").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_delete_all_cfdis(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let _ = state
        .cfdis
        .delete_many(bson::doc! { "company_id": &id })
        .await;
    Redirect::to(&format!("/admin/companies/{id}/edit")).into_response()
}

pub async fn companies_delete_all_transactions(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if !has_admin_role_for(&session_user, &object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let _ = state
        .transactions
        .delete_many(bson::doc! { "company_id": object_id })
        .await;
    Redirect::to(&format!("/admin/companies/{id}/edit")).into_response()
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Ok(()); // allow fallback to slugify(name)
    }
    let is_valid = slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && slug.len() <= 64
        && !slug.starts_with('-')
        && !slug.ends_with('-');
    if !is_valid {
        return Err(
            "Slug inválido. Usa solo minúsculas, números y guiones, sin iniciar ni terminar con guion (máx 64 caracteres)."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_slug_accepts_empty_or_safe_slugs() {
        assert!(validate_slug("").is_ok());
        assert!(validate_slug("acme-123").is_ok());
        assert!(validate_slug("mi-empresa").is_ok());
    }

    #[test]
    fn validate_slug_rejects_unsafe_values() {
        assert!(validate_slug("Acme").is_err());
        assert!(validate_slug("-acme").is_err());
        assert!(validate_slug("acme-").is_err());
        assert!(validate_slug("acme_test").is_err());
        assert!(validate_slug(&"a".repeat(65)).is_err());
    }
}
