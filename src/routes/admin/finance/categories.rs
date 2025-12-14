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
    state::{AppState, create_category, delete_category, get_category_by_id, list_categories, update_category},
};

use super::helpers::*;

#[derive(Template)]
#[template(path = "admin/categories/index.html")]
struct CategoriesIndexTemplate {
    categories: Vec<CategoryRow>,
}

struct CategoryRow {
    id: String,
    name: String,
    company: String,
    flow_type: String,
    parent: String,
}

#[derive(Template)]
#[template(path = "admin/categories/form.html")]
struct CategoryFormTemplate {
    action: String,
    name: String,
    flow_type: String,
    parent_id: Option<String>,
    companies: Vec<SimpleOption>,
    flow_options: Vec<SimpleOption>,
    parent_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct CategoryFormData {
    name: String,
    company_id: String,
    flow_type: String,
    #[serde(default)]
    parent_id: Option<String>,
}

pub async fn categories_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let categories = list_categories(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .collect::<Vec<_>>();
    let category_map = build_lookup_map(
        categories
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c.name.clone())))
            .collect(),
    );
    let active_company = session_user.active_company_id().clone();
    let active_name = session_user.user().company_name.clone();

    let rows = categories
        .into_iter()
        .filter(|cat| cat.company_id == active_company)
        .filter_map(|cat| {
            cat.id.map(|id| CategoryRow {
                id: id.to_hex(),
                name: cat.name,
                company: active_name.clone(),
                flow_type: flow_type_value(&cat.flow_type).to_string(),
                parent: cat
                    .parent_id
                    .and_then(|pid| category_map.get(&pid).cloned())
                    .unwrap_or_else(|| "-".into()),
            })
        })
        .collect();

    render(CategoriesIndexTemplate { categories: rows })
}

pub async fn categories_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let companies = company_options(&state, &active_company).await?;
    let parents = category_parent_options(&state, None, &active_company).await?;

    render(CategoryFormTemplate {
        action: "/admin/categories".into(),
        name: String::new(),
        flow_type: "income".into(),
        parent_id: None,
        companies,
        flow_options: flow_options("income"),
        parent_options: parents,
        is_edit: false,
        errors: None,
    })
}

pub async fn categories_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CategoryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id).await.unwrap_or_default();
    let parents = category_parent_options(&state, None, &company_id)
        .await
        .unwrap_or_default();

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(msg) => {
            return render(CategoryFormTemplate {
                action: "/admin/categories".into(),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                parent_id: form.parent_id.clone(),
                companies,
                flow_options: flow_options(&form.flow_type),
                parent_options: category_parent_options(&state, None, &company_id)
                    .await
                    .unwrap_or_default(),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let parent_id = match form.parent_id.clone().and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(pid) => Some(
            parse_object_id(&pid, "Categoría padre")
                .map_err(|e| e)
                .map_err(|msg| {
                    render(CategoryFormTemplate {
                        action: "/admin/categories".into(),
                        name: form.name.clone(),
                        flow_type: form.flow_type.clone(),
                        parent_id: Some(pid.clone()),
                        companies: companies.clone(),
                        flow_options: flow_options(&form.flow_type),
                        parent_options: parents.clone(),
                        is_edit: false,
                        errors: Some(msg),
                    })
                    .map(IntoResponse::into_response)
                    .unwrap_or_else(|status| status.into_response())
                }),
        ),
        None => None,
    };

    let parent_id = match parent_id {
        Some(Ok(id)) => Some(id),
        Some(Err(resp)) => return resp,
        None => None,
    };

    if let Some(pid) = parent_id {
        match get_category_by_id(&state, &pid).await {
            Ok(Some(cat)) => {
                if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                    return status.into_response();
                }
            }
            Ok(None) => return StatusCode::BAD_REQUEST.into_response(),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

    match create_category(
        &state,
        &company_id,
        form.name.trim(),
        flow_type,
        parent_id,
        None,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn categories_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let category = get_category_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&category.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;
    let parents =
        category_parent_options(&state, category.parent_id.as_ref(), &active_company).await?;

    render(CategoryFormTemplate {
        action: format!("/admin/categories/{}/update", id),
        name: category.name,
        flow_type: flow_type_value(&category.flow_type).to_string(),
        parent_id: opt_to_string(&category.parent_id),
        companies,
        flow_options: flow_options(flow_type_value(&category.flow_type)),
        parent_options: parents,
        is_edit: true,
        errors: None,
    })
}

pub async fn categories_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<CategoryFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_category_by_id(&state, &object_id).await {
        Ok(Some(cat)) => {
            if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let flow_type = match parse_flow_type(&form.flow_type) {
        Ok(f) => f,
        Err(msg) => {
            let companies = company_options(&state, &company_id)
                .await
                .unwrap_or_default();
            let parents = category_parent_options(&state, None, &company_id)
                .await
                .unwrap_or_default();
            return render(CategoryFormTemplate {
                action: format!("/admin/categories/{}/update", id),
                name: form.name.clone(),
                flow_type: form.flow_type.clone(),
                parent_id: form.parent_id.clone(),
                companies,
                flow_options: flow_options(&form.flow_type),
                parent_options: parents,
                is_edit: true,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let parent_id = match form.parent_id.clone().and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(pid) => match parse_object_id(&pid, "Categoría padre") {
            Ok(id) => Some(id),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    if let Some(pid) = parent_id {
        match get_category_by_id(&state, &pid).await {
            Ok(Some(cat)) => {
                if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                    return status.into_response();
                }
            }
            Ok(None) => return StatusCode::BAD_REQUEST.into_response(),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

    match update_category(
        &state,
        &object_id,
        &company_id,
        form.name.trim(),
        flow_type,
        parent_id,
        None,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn categories_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_category_by_id(&state, &object_id).await {
        Ok(Some(cat)) => {
            if let Err(status) = ensure_same_company(&cat.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match delete_category(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/categories").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn category_parent_options(
    state: &AppState,
    selected: Option<&ObjectId>,
    company_id: &ObjectId,
) -> Result<Vec<SimpleOption>, StatusCode> {
    let categories = list_categories(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == *company_id)
        .collect::<Vec<_>>();
    let mut options = Vec::new();
    options.push(SimpleOption {
        value: "".into(),
        label: "Sin padre".into(),
        selected: selected.is_none(),
    });
    options.extend(categories.into_iter().filter_map(|c| {
        c.id.map(|id| SimpleOption {
            value: id.to_hex(),
            label: c.name,
            selected: selected.map(|s| *s == id).unwrap_or(false),
        })
    }));
    Ok(options)
}
