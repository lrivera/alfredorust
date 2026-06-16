use std::sync::Arc;

use askama::Template;
use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use serde::{Deserialize, Deserializer, Serialize};

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{Resource, ResourceType},
    session::SessionUser,
    state::{
        AppState, create_resource_with_cost, delete_resource, get_resource_by_id,
        get_resource_by_id_for_company, list_concept_statuses, list_resources, update_resource,
        update_resource_allowed_statuses, update_resource_cost,
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
    allowed_statuses: String,
    notes: String,
}

#[derive(Serialize)]
pub struct ResourceData {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub resource_type: String,
    pub resource_type_label: String,
    pub is_active: bool,
    pub hourly_cost: f64,
    pub currency: String,
    pub allowed_status_ids: Vec<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ResourcePayload {
    pub name: String,
    #[serde(default = "default_resource_type")]
    pub resource_type: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub hourly_cost: f64,
    pub currency: Option<String>,
    #[serde(default)]
    pub allowed_status_ids: Vec<String>,
    pub notes: Option<String>,
}

struct ParsedResourcePayload {
    name: String,
    resource_type: ResourceType,
    is_active: bool,
    hourly_cost: f64,
    currency: String,
    allowed_status_ids: Vec<ObjectId>,
    notes: Option<String>,
}

fn default_resource_type() -> String {
    "machinery".to_string()
}

fn default_true() -> bool {
    true
}

pub async fn resources_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    let resources = list_resources(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let statuses = list_concept_statuses(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = resources
        .into_iter()
        .map(|r| {
            let allowed_statuses = if r.allowed_status_ids.is_empty() {
                "Ningún estado".to_string()
            } else {
                statuses
                    .iter()
                    .filter(|s| {
                        s.id.as_ref()
                            .is_some_and(|id| r.allowed_status_ids.contains(id))
                    })
                    .map(|s| s.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            ResourceRow {
                id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
                name: r.name,
                resource_type: r.resource_type.as_str().to_string(),
                resource_type_label: r.resource_type.label().to_string(),
                is_active: r.is_active,
                hourly_cost: format!("{:.2}", r.hourly_cost),
                currency: r.currency,
                allowed_statuses,
                notes: r.notes.unwrap_or_default(),
            }
        })
        .collect();

    render(ResourcesIndexTemplate { resources: rows })
}

pub async fn resources_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ResourceData>>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let resources = list_resources(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        resources.into_iter().filter_map(resource_data).collect(),
    ))
}

pub async fn resource_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ResourceData>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let resource = get_resource_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    resource_data(resource)
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
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
    statuses: Vec<SimpleOption>,
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

fn resource_data(resource: Resource) -> Option<ResourceData> {
    let id = resource.id?.to_hex();
    Some(ResourceData {
        id,
        company_id: resource.company_id.to_hex(),
        name: resource.name,
        resource_type: resource.resource_type.as_str().to_string(),
        resource_type_label: resource.resource_type.label().to_string(),
        is_active: resource.is_active,
        hourly_cost: resource.hourly_cost,
        currency: resource.currency,
        allowed_status_ids: resource
            .allowed_status_ids
            .into_iter()
            .map(|id| id.to_hex())
            .collect(),
        notes: resource.notes,
        created_at: resource
            .created_at
            .map(|date| date.to_chrono().to_rfc3339()),
        updated_at: resource
            .updated_at
            .map(|date| date.to_chrono().to_rfc3339()),
    })
}

fn parse_resource_type(s: &str) -> ResourceType {
    match s {
        "vehicle" => ResourceType::Vehicle,
        "equipment" => ResourceType::Equipment,
        "other" => ResourceType::Other,
        _ => ResourceType::Machinery,
    }
}

fn parse_resource_type_strict(s: &str) -> Result<ResourceType, StatusCode> {
    match s {
        "machinery" => Ok(ResourceType::Machinery),
        "vehicle" => Ok(ResourceType::Vehicle),
        "equipment" => Ok(ResourceType::Equipment),
        "other" => Ok(ResourceType::Other),
        _ => Err(StatusCode::BAD_REQUEST),
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

    #[test]
    fn resource_form_accepts_single_allowed_status_id() {
        let form: ResourceForm = serde_json::from_value(serde_json::json!({
            "name": "CNC",
            "resource_type": "machinery",
            "allowed_status_ids": "69fa2bc87bc6c685d00722d0"
        }))
        .unwrap();

        assert_eq!(form.allowed_status_ids, vec!["69fa2bc87bc6c685d00722d0"]);
    }

    #[test]
    fn resource_form_accepts_multiple_allowed_status_ids() {
        let form: ResourceForm = serde_json::from_value(serde_json::json!({
            "name": "CNC",
            "resource_type": "machinery",
            "allowed_status_ids": ["a", "b"]
        }))
        .unwrap();

        assert_eq!(form.allowed_status_ids, vec!["a", "b"]);
    }

    #[test]
    fn resource_form_defaults_missing_allowed_status_ids_to_empty() {
        let form: ResourceForm = serde_json::from_value(serde_json::json!({
            "name": "CNC",
            "resource_type": "machinery"
        }))
        .unwrap();

        assert!(form.allowed_status_ids.is_empty());
    }
}

#[derive(Debug, Deserialize)]
pub struct ResourceForm {
    pub name: String,
    pub resource_type: String,
    pub is_active: Option<String>,
    pub hourly_cost: Option<String>,
    pub currency: Option<String>,
    #[serde(default, deserialize_with = "deserialize_status_ids")]
    pub allowed_status_ids: Vec<String>,
    pub notes: Option<String>,
}

pub async fn resources_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;

    render(ResourceFormTemplate {
        action: "/admin/resources".into(),
        id: String::new(),
        name: String::new(),
        resource_type: "machinery".into(),
        is_active: "on".into(),
        hourly_cost: "0.00".into(),
        currency: "MXN".into(),
        statuses: resource_status_options(&state, &company_id, &[]).await?,
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
            statuses: resource_status_options(&state, &company_id, &[])
                .await
                .unwrap_or_default(),
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
        Ok(id) => {
            let allowed = parse_status_ids(form.allowed_status_ids);
            match update_resource_allowed_statuses(&state, &id, &company_id, allowed).await {
                Ok(_) => Redirect::to("/admin/resources").into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resources_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResourcePayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_resource_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    let id = match create_resource_with_cost(
        &state,
        &company_id,
        &parsed.name,
        parsed.resource_type,
        parsed.is_active,
        parsed.hourly_cost,
        &parsed.currency,
        parsed.notes,
    )
    .await
    {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if update_resource_allowed_statuses(&state, &id, &company_id, parsed.allowed_status_ids)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id.to_hex() })),
    )
        .into_response()
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
        statuses: resource_status_options(&state, &company_id, &resource.allowed_status_ids)
            .await?,
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
            statuses: resource_status_options(&state, &company_id, &[])
                .await
                .unwrap_or_default(),
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
            let allowed = parse_status_ids(form.allowed_status_ids);
            if update_resource_cost(&state, &oid, &company_id, hourly_cost, &currency)
                .await
                .is_err()
            {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            match update_resource_allowed_statuses(&state, &oid, &company_id, allowed).await {
                Ok(_) => Redirect::to("/admin/resources").into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn resource_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ResourcePayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match get_resource_by_id_for_company(&state, &oid, &company_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let parsed = match parse_resource_payload(&state, &company_id, payload).await {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };

    if update_resource(
        &state,
        &oid,
        &company_id,
        &parsed.name,
        parsed.resource_type,
        parsed.is_active,
        parsed.notes,
    )
    .await
    .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if update_resource_cost(
        &state,
        &oid,
        &company_id,
        parsed.hourly_cost,
        &parsed.currency,
    )
    .await
    .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match update_resource_allowed_statuses(&state, &oid, &company_id, parsed.allowed_status_ids)
        .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn resource_status_options(
    state: &AppState,
    company_id: &ObjectId,
    selected: &[ObjectId],
) -> Result<Vec<SimpleOption>, StatusCode> {
    Ok(list_concept_statuses(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter_map(|status| {
            let id = status.id?;
            Some(SimpleOption {
                selected: selected.contains(&id),
                value: id.to_hex(),
                label: status.name,
            })
        })
        .collect())
}

fn parse_status_ids(values: Vec<String>) -> Vec<ObjectId> {
    values
        .into_iter()
        .filter_map(|id| ObjectId::parse_str(id).ok())
        .collect()
}

fn deserialize_status_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    Ok(match Option::<OneOrMany>::deserialize(deserializer)? {
        Some(OneOrMany::One(value)) if !value.is_empty() => vec![value],
        Some(OneOrMany::One(_)) | None => Vec::new(),
        Some(OneOrMany::Many(values)) => values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect(),
    })
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

pub async fn resource_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    get_resource_by_id_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    delete_resource(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

async fn parse_resource_payload(
    state: &AppState,
    company_id: &ObjectId,
    payload: ResourcePayload,
) -> Result<ParsedResourcePayload, StatusCode> {
    let name = payload.name.trim().to_string();
    if name.is_empty() || payload.hourly_cost < 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let allowed_status_ids = parse_status_ids_strict(payload.allowed_status_ids)?;
    validate_allowed_statuses(state, company_id, &allowed_status_ids).await?;
    Ok(ParsedResourcePayload {
        name,
        resource_type: parse_resource_type_strict(&payload.resource_type)?,
        is_active: payload.is_active,
        hourly_cost: payload.hourly_cost,
        currency: clean_opt(payload.currency).unwrap_or_else(|| "MXN".to_string()),
        allowed_status_ids,
        notes: clean_opt(payload.notes),
    })
}

fn clean_opt(input: Option<String>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn parse_status_ids_strict(values: Vec<String>) -> Result<Vec<ObjectId>, StatusCode> {
    values
        .into_iter()
        .map(|id| ObjectId::parse_str(id).map_err(|_| StatusCode::BAD_REQUEST))
        .collect()
}

async fn validate_allowed_statuses(
    state: &AppState,
    company_id: &ObjectId,
    ids: &[ObjectId],
) -> Result<(), StatusCode> {
    if ids.is_empty() {
        return Ok(());
    }
    let statuses = list_concept_statuses(state, company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for id in ids {
        if !statuses.iter().any(|status| status.id.as_ref() == Some(id)) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}
