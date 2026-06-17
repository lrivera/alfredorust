use std::{path::PathBuf, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::SatConfig,
    session::SessionUser,
    state::{
        AppState, create_sat_config, delete_sat_config, get_company_by_id,
        get_sat_config_for_company, list_sat_configs,
    },
};

use super::finance::helpers::require_admin_active;

const MAX_SAT_FILE_BYTES: usize = 2 * 1024 * 1024;

fn require_company_admin(
    session_user: &SessionUser,
    company_id: &ObjectId,
) -> Result<(), StatusCode> {
    if session_user
        .user()
        .company_ids
        .iter()
        .zip(session_user.user().company_roles.iter())
        .any(|(cid, role)| cid == company_id && role.is_admin())
    {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn safe_upload_filename(filename: Option<&str>, fallback: &str) -> String {
    let name = filename
        .and_then(|raw| raw.rsplit(['/', '\\']).next())
        .unwrap_or(fallback)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .collect::<String>();
    if name.is_empty() || name == "." || name == ".." {
        fallback.to_string()
    } else {
        name
    }
}

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
pub struct SatConfigData {
    pub id: String,
    pub company_id: String,
    pub rfc: String,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SatConfigPayload {
    pub rfc: String,
    pub cer_path: String,
    pub key_path: String,
    pub key_password: String,
    pub label: Option<String>,
}

fn sat_config_data(config: SatConfig) -> Option<SatConfigData> {
    let id = config.id?.to_hex();
    Some(SatConfigData {
        id,
        company_id: config.company_id.to_hex(),
        rfc: config.rfc,
        label: config.label,
        created_at: config.created_at.to_chrono().to_rfc3339(),
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/sat-configs",
    tag = "admin",
    responses(
        (status = 200, description = "List of SAT configs"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn sat_configs_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<SatConfigData>>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let configs = list_sat_configs(&state, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        configs.into_iter().filter_map(sat_config_data).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/sat-configs/{id}",
    tag = "admin",
    params(("id" = String, Path, description = "SAT config id")),
    responses(
        (status = 200, description = "SAT config detail"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn sat_config_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SatConfigData>, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let config = get_sat_config_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    sat_config_data(config)
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    post,
    path = "/api/admin/sat-configs",
    tag = "admin",
    request_body = SatConfigPayload,
    responses(
        (status = 201, description = "SAT config created"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn sat_config_create_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SatConfigPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let parsed = match parse_sat_config_payload(payload) {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    let id = ObjectId::new();
    match create_sat_config(
        &state,
        id,
        company_id,
        parsed.rfc,
        parsed.cer_path,
        parsed.key_path,
        parsed.key_password,
        parsed.label,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Multipart twin of `sat_config_create_api` that accepts the actual `.cer` and
/// `.key` file uploads (instead of pre-existing server paths), so a SPA/spcli
/// client can provision a SAT config end-to-end. Scoped to the active company.
#[utoipa::path(
    post,
    path = "/api/admin/sat-configs/upload",
    tag = "admin",
    responses(
        (status = 201, description = "SAT config created from multipart/form-data file upload (.cer + .key)"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn sat_config_upload_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let mut rfc = String::new();
    let mut label = None::<String>;
    let mut key_password = String::new();
    let mut cer_bytes: Option<(String, Vec<u8>)> = None;
    let mut key_bytes: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "rfc" => {
                rfc = field.text().await.unwrap_or_default().trim().to_uppercase();
            }
            "label" => {
                let value = field.text().await.unwrap_or_default();
                let value = value.trim();
                if !value.is_empty() {
                    label = Some(value.to_string());
                }
            }
            "key_password" => {
                key_password = field.text().await.unwrap_or_default();
            }
            "cer_file" => {
                let filename = safe_upload_filename(field.file_name(), "cert.cer");
                let data = field.bytes().await.unwrap_or_default().to_vec();
                if data.len() > MAX_SAT_FILE_BYTES {
                    return StatusCode::PAYLOAD_TOO_LARGE.into_response();
                }
                if !data.is_empty() {
                    cer_bytes = Some((filename, data));
                }
            }
            "key_file" => {
                let filename = safe_upload_filename(field.file_name(), "private.key");
                let data = field.bytes().await.unwrap_or_default().to_vec();
                if data.len() > MAX_SAT_FILE_BYTES {
                    return StatusCode::PAYLOAD_TOO_LARGE.into_response();
                }
                if !data.is_empty() {
                    key_bytes = Some((filename, data));
                }
            }
            _ => {}
        }
    }

    if rfc.is_empty() || cer_bytes.is_none() || key_bytes.is_none() || key_password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "rfc, key_password, cer_file and key_file are required"
            })),
        )
            .into_response();
    }

    let config_id = ObjectId::new();
    let upload_dir = PathBuf::from("uploads")
        .join("sat")
        .join(company_id.to_hex())
        .join(config_id.to_hex());

    if fs::create_dir_all(&upload_dir).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let (cer_filename, cer_data) = cer_bytes.unwrap();
    let (key_filename, key_data) = key_bytes.unwrap();
    let cer_path = upload_dir.join(&cer_filename);
    let key_path = upload_dir.join(&key_filename);

    if fs::write(&cer_path, &cer_data).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if fs::write(&key_path, &key_data).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match create_sat_config(
        &state,
        config_id,
        company_id,
        rfc,
        cer_path.to_string_lossy().to_string(),
        key_path.to_string_lossy().to_string(),
        key_password,
        label,
    )
    .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/sat-configs/{id}/update",
    tag = "admin",
    params(("id" = String, Path, description = "SAT config id")),
    request_body = SatConfigPayload,
    responses(
        (status = 200, description = "SAT config updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn sat_config_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<SatConfigPayload>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };
    let oid = match ObjectId::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if get_sat_config_for_company(&state, &oid, &company_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let parsed = match parse_sat_config_payload(payload) {
        Ok(parsed) => parsed,
        Err(status) => return status.into_response(),
    };
    match state
        .sat_configs
        .update_one(
            bson::doc! { "_id": oid, "company_id": company_id },
            bson::doc! { "$set": {
                "rfc": parsed.rfc,
                "cer_path": parsed.cer_path,
                "key_path": parsed.key_path,
                "key_password": parsed.key_password,
                "label": parsed.label,
            }},
        )
        .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/sat-configs/{id}/delete",
    tag = "admin",
    params(("id" = String, Path, description = "SAT config id")),
    responses(
        (status = 200, description = "SAT config deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn sat_config_delete_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let company_id = require_admin_active(&session_user)?;
    let oid = ObjectId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    get_sat_config_for_company(&state, &oid, &company_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    delete_sat_config(&state, &oid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

fn parse_sat_config_payload(payload: SatConfigPayload) -> Result<SatConfigPayload, StatusCode> {
    let rfc = payload.rfc.trim().to_uppercase();
    let cer_path = payload.cer_path.trim().to_string();
    let key_path = payload.key_path.trim().to_string();
    if rfc.is_empty()
        || cer_path.is_empty()
        || key_path.is_empty()
        || payload.key_password.is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(SatConfigPayload {
        rfc,
        cer_path,
        key_path,
        key_password: payload.key_password,
        label: payload.label.and_then(|label| {
            let label = label.trim().to_string();
            if label.is_empty() { None } else { Some(label) }
        }),
    })
}

#[derive(Template)]
#[template(path = "admin/sat_configs/form.html")]
struct SatConfigFormTemplate {
    company_id: String,
    company_name: String,
    errors: Option<String>,
}

pub async fn sat_configs_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let object_id = ObjectId::from_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    require_company_admin(&session_user, &object_id)?;
    let company = get_company_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    render(SatConfigFormTemplate {
        company_id,
        company_name: company.name,
        errors: None,
    })
}

pub async fn sat_configs_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = require_company_admin(&session_user, &company_object_id) {
        return status.into_response();
    }

    let mut rfc = String::new();
    let mut label = None::<String>;
    let mut key_password = String::new();
    let mut cer_bytes: Option<(String, Vec<u8>)> = None;
    let mut key_bytes: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "rfc" => {
                rfc = field.text().await.unwrap_or_default().trim().to_uppercase();
            }
            "label" => {
                let v = field.text().await.unwrap_or_default();
                let v = v.trim();
                if !v.is_empty() {
                    label = Some(v.to_string());
                }
            }
            "key_password" => {
                key_password = field.text().await.unwrap_or_default();
            }
            "cer_file" => {
                let filename = safe_upload_filename(field.file_name(), "cert.cer");
                let data = field.bytes().await.unwrap_or_default().to_vec();
                if data.len() > MAX_SAT_FILE_BYTES {
                    return StatusCode::PAYLOAD_TOO_LARGE.into_response();
                }
                if !data.is_empty() {
                    cer_bytes = Some((filename, data));
                }
            }
            "key_file" => {
                let filename = safe_upload_filename(field.file_name(), "private.key");
                let data = field.bytes().await.unwrap_or_default().to_vec();
                if data.len() > MAX_SAT_FILE_BYTES {
                    return StatusCode::PAYLOAD_TOO_LARGE.into_response();
                }
                if !data.is_empty() {
                    key_bytes = Some((filename, data));
                }
            }
            _ => {}
        }
    }

    if rfc.is_empty() || cer_bytes.is_none() || key_bytes.is_none() || key_password.is_empty() {
        let company = get_company_by_id(&state, &company_object_id)
            .await
            .ok()
            .flatten();
        return render(SatConfigFormTemplate {
            company_id: company_id.clone(),
            company_name: company.map(|c| c.name).unwrap_or_default(),
            errors: Some("RFC, contraseña, archivo .cer y archivo .key son obligatorios.".into()),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|s| s.into_response());
    }

    let config_id = ObjectId::new();
    let upload_dir = PathBuf::from("uploads")
        .join("sat")
        .join(company_id.as_str())
        .join(config_id.to_hex());

    if let Err(e) = fs::create_dir_all(&upload_dir).await {
        eprintln!("[sat_configs] failed to create upload dir: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let (cer_filename, cer_data) = cer_bytes.unwrap();
    let (key_filename, key_data) = key_bytes.unwrap();

    let cer_path = upload_dir.join(&cer_filename);
    let key_path = upload_dir.join(&key_filename);

    if let Err(e) = fs::write(&cer_path, &cer_data).await {
        eprintln!("[sat_configs] failed to write cer: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Err(e) = fs::write(&key_path, &key_data).await {
        eprintln!("[sat_configs] failed to write key: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match create_sat_config(
        &state,
        config_id,
        company_object_id,
        rfc,
        cer_path.to_string_lossy().to_string(),
        key_path.to_string_lossy().to_string(),
        key_password,
        label,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/admin/companies/{}/edit", company_id)).into_response(),
        Err(e) => {
            eprintln!("[sat_configs] db insert error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn sat_configs_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path((company_id, config_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let config_object_id = match ObjectId::from_str(&config_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if let Err(status) = require_company_admin(&session_user, &company_object_id) {
        return status.into_response();
    }

    // Optionally remove the files from disk
    let config = match state
        .sat_configs
        .find_one(bson::doc! { "_id": &config_object_id, "company_id": &company_object_id })
        .await
    {
        Ok(Some(config)) => config,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    {
        let _ = fs::remove_file(&config.cer_path).await;
        let _ = fs::remove_file(&config.key_path).await;
        // Remove directory if empty
        if let Some(parent) = std::path::Path::new(&config.cer_path).parent() {
            let _ = fs::remove_dir(parent).await;
        }
    }

    match delete_sat_config(&state, &config_object_id).await {
        Ok(_) => Redirect::to(&format!("/admin/companies/{}/edit", company_id)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub struct SatConfigRow {
    pub id: String,
    pub rfc: String,
    pub label: String,
    pub cer_path: String,
}

pub async fn load_sat_configs_for_company(
    state: &AppState,
    company_id: &ObjectId,
) -> Vec<SatConfigRow> {
    list_sat_configs(state, company_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|c| SatConfigRow {
            id: c.id.map(|id| id.to_hex()).unwrap_or_default(),
            rfc: c.rfc,
            label: c.label.unwrap_or_default(),
            cer_path: c.cer_path,
        })
        .collect()
}
