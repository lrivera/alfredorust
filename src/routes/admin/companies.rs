use std::{collections::HashSet, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
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

use crate::{
    models::UserRole,
    session::SessionUser,
    state::{
        AppState, add_user_to_company, create_company, delete_company, get_company_by_id,
        list_companies, update_company,
    },
};

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
    if let Ok(Some(existing)) = state
        .companies
        .find_one(doc! { "slug": &final_slug })
        .await
    {
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
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    }

    match create_company(&state, name, final_slug.as_str(), currency, is_active, notes).await {
        Ok(company_id) => match add_user_to_company(
            &state,
            session_user.user_id(),
            &company_id,
            UserRole::Admin,
        )
        .await
        {
            Ok(_) => Redirect::to("/admin/companies").into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
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
    if let Ok(Some(existing)) = state
        .companies
        .find_one(doc! { "slug": &final_slug })
        .await
    {
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
