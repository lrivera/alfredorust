use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    session::SessionUser,
    state::{
        AppState, create_company, delete_company, get_company_by_id, list_companies, update_company,
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
}

#[derive(Template)]
#[template(path = "admin/companies/form.html")]
struct CompanyFormTemplate {
    action: String,
    name: String,
    default_currency: String,
    is_active: bool,
    notes: String,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct CompanyFormData {
    name: String,
    default_currency: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn companies_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    let companies = list_companies(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter_map(|company| {
            company.id.map(|id| CompanyRow {
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
    session_user: SessionUser,
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    render(CompanyFormTemplate {
        action: "/admin/companies".into(),
        name: String::new(),
        default_currency: "MXN".into(),
        is_active: true,
        notes: String::new(),
        is_edit: false,
        errors: None,
    })
}

pub async fn companies_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CompanyFormData>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    let name = form.name.trim();
    let default_currency = form.default_currency.trim();
    let is_active = form.is_active;
    let notes = form
        .notes
        .and_then(|n| {
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
            default_currency: default_currency.to_string(),
            is_active,
            notes: String::new(),
            is_edit: false,
            errors: Some("El nombre es obligatorio".into()),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }

    let currency = if default_currency.is_empty() {
        "MXN"
    } else {
        default_currency
    };

    match create_company(&state, name, currency, is_active, notes).await {
        Ok(_) => Redirect::to("/admin/companies").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let company = get_company_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    render(CompanyFormTemplate {
        action: format!("/admin/companies/{}/update", id),
        name: company.name,
        default_currency: company.default_currency,
        is_active: company.is_active,
        notes: company.notes.unwrap_or_default(),
        is_edit: true,
        errors: None,
    })
}

pub async fn companies_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<CompanyFormData>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let name = form.name.trim();
    let default_currency = form.default_currency.trim();
    let is_active = form.is_active;
    let notes = form
        .notes
        .and_then(|n| {
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
            default_currency: default_currency.to_string(),
            is_active,
            notes: String::new(),
            is_edit: true,
            errors: Some("El nombre es obligatorio".into()),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }

    let currency = if default_currency.is_empty() {
        "MXN"
    } else {
        default_currency
    };

    match update_company(&state, &object_id, name, currency, is_active, notes).await {
        Ok(_) => Redirect::to("/admin/companies").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn companies_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match delete_company(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/companies").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
