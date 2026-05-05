use std::sync::Arc;

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::ResourceType,
    session::SessionUser,
    state::{
        AppState, create_resource_with_cost, delete_resource, get_resource_by_id, list_resources,
        update_resource, update_resource_cost,
    },
};

use super::finance::helpers::*;

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/resources/index.html")]
struct ResourcesIndexTemplate {
    resources: Vec<ResourceRow>,
}

struct ResourceRow {
    id: String,
    name: String,
    resource_type: String,
    resource_type_label: String,
    is_active: bool,
    hourly_cost: String,
    currency: String,
    notes: String,
}

pub async fn resources_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let resources = list_resources(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = resources
        .into_iter()
        .map(|r| ResourceRow {
            id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: r.name,
            resource_type: r.resource_type.as_str().to_string(),
            resource_type_label: r.resource_type.label().to_string(),
            is_active: r.is_active,
            hourly_cost: format!("{:.2}", r.hourly_cost),
            currency: r.currency,
            notes: r.notes.unwrap_or_default(),
        })
        .collect();

    render(ResourcesIndexTemplate { resources: rows })
}

#[derive(Template)]
#[template(path = "admin/resources/form.html")]
struct ResourceFormTemplate {
    action: String,
    id: String,
    name: String,
    resource_type: String,
    is_active: String,
    hourly_cost: String,
    currency: String,
    notes: String,
    resource_type_options: Vec<(String, String)>,
    is_edit: bool,
    errors: Option<String>,
}

fn resource_type_options() -> Vec<(String, String)> {
    vec![
        ("machinery".into(), "Maquinaria".into()),
        ("vehicle".into(), "Vehículo".into()),
        ("equipment".into(), "Equipo".into()),
        ("other".into(), "Otro".into()),
    ]
}

fn parse_resource_type(s: &str) -> ResourceType {
    match s {
        "vehicle" => ResourceType::Vehicle,
        "equipment" => ResourceType::Equipment,
        "other" => ResourceType::Other,
        _ => ResourceType::Machinery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_resource_type_defaults_to_machinery() {
        assert_eq!(parse_resource_type("vehicle"), ResourceType::Vehicle);
        assert_eq!(parse_resource_type("equipment"), ResourceType::Equipment);
        assert_eq!(parse_resource_type("other"), ResourceType::Other);
        assert_eq!(parse_resource_type("unknown"), ResourceType::Machinery);
    }

    #[test]
    fn resource_type_options_cover_all_form_values() {
        let values: Vec<_> = resource_type_options()
            .into_iter()
            .map(|(value, _)| value)
            .collect();

        assert_eq!(values, vec!["machinery", "vehicle", "equipment", "other"]);
    }
}

#[derive(Debug, Deserialize)]
pub struct ResourceForm {
    pub name: String,
    pub resource_type: String,
    pub is_active: Option<String>,
    pub hourly_cost: Option<String>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

pub async fn resources_new(
    session_user: SessionUser,
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    require_admin_active(&session_user)?;

    render(ResourceFormTemplate {
        action: "/admin/resources".into(),
        id: String::new(),
        name: String::new(),
        resource_type: "machinery".into(),
        is_active: "on".into(),
        hourly_cost: "0.00".into(),
        currency: "MXN".into(),
        notes: String::new(),
        resource_type_options: resource_type_options(),
        is_edit: false,
        errors: None,
    })
}

pub async fn resources_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ResourceForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };

    let name = form.name.trim();
    if name.is_empty() {
        return render(ResourceFormTemplate {
            action: "/admin/resources".into(),
            id: String::new(),
            name: form.name,
            resource_type: form.resource_type,
            is_active: form
                .is_active
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            hourly_cost: form.hourly_cost.clone().unwrap_or_default(),
            currency: form.currency.clone().unwrap_or_else(|| "MXN".into()),
            notes: form.notes.clone().unwrap_or_default(),
            resource_type_options: resource_type_options(),
            is_edit: false,
            errors: Some("El nombre es requerido".into()),
        })
        .into_response();
    }

    let resource_type = parse_resource_type(&form.resource_type);
    let is_active = form.is_active.is_some();
    let hourly_cost = form
        .hourly_cost
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0.0);
    let currency = form.currency.as_deref().unwrap_or("MXN");
    let notes = form.notes.filter(|s| !s.trim().is_empty());

    match create_resource_with_cost(
        &state,
        &company_id,
        name,
        resource_type,
        is_active,
        hourly_cost,
        currency,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/resources").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resources_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let resource = get_resource_by_id(&state, &oid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    ensure_same_company(&resource.company_id, &company_id)?;

    render(ResourceFormTemplate {
        action: format!("/admin/resources/{}/update", id),
        id: id.clone(),
        name: resource.name,
        resource_type: resource.resource_type.as_str().to_string(),
        is_active: if resource.is_active {
            "on".to_string()
        } else {
            String::new()
        },
        hourly_cost: format!("{:.2}", resource.hourly_cost),
        currency: resource.currency,
        notes: resource.notes.unwrap_or_default(),
        resource_type_options: resource_type_options(),
        is_edit: true,
        errors: None,
    })
}

pub async fn resources_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ResourceForm>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(o) => o,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let name = form.name.trim();
    if name.is_empty() {
        return render(ResourceFormTemplate {
            action: format!("/admin/resources/{}/update", id),
            id: id.clone(),
            name: form.name,
            resource_type: form.resource_type,
            is_active: form
                .is_active
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            hourly_cost: form.hourly_cost.clone().unwrap_or_default(),
            currency: form.currency.clone().unwrap_or_else(|| "MXN".into()),
            notes: form.notes.clone().unwrap_or_default(),
            resource_type_options: resource_type_options(),
            is_edit: true,
            errors: Some("El nombre es requerido".into()),
        })
        .into_response();
    }

    let resource_type = parse_resource_type(&form.resource_type);
    let is_active = form.is_active.is_some();
    let hourly_cost = form
        .hourly_cost
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0.0);
    let currency = form.currency.as_deref().unwrap_or("MXN").to_string();
    let notes = form.notes.filter(|s| !s.trim().is_empty());

    match update_resource(
        &state,
        &oid,
        &company_id,
        name,
        resource_type,
        is_active,
        notes,
    )
    .await
    {
        Ok(_) => {
            match update_resource_cost(&state, &oid, &company_id, hourly_cost, &currency).await {
                Ok(_) => Redirect::to("/admin/resources").into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resources_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(s) => return s.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(o) => o,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match delete_resource(&state, &oid, &company_id).await {
        Ok(_) => Redirect::to("/admin/resources").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
