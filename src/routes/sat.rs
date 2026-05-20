use std::{path::Path, sync::Arc};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use tokio::fs;

use crate::{
    cfdi,
    sat::{CfdiDownloadRequest, SatError, download_cfdis},
    session::SessionUser,
    state::AppState,
};

pub async fn sat_cfdi_download(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<CfdiDownloadRequest>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    if payload.output_dir.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"output_dir no se acepta desde la API"})),
        )
            .into_response();
    }
    if !is_allowed_sat_path(payload.cer_path.as_deref())
        || !is_allowed_sat_path(payload.key_path.as_deref())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"ruta de certificado inválida"})),
        )
            .into_response();
    }
    payload.output_dir = None;

    let slug = session_user.active_company_slug().to_string();
    match download_cfdis(&slug, payload).await {
        Ok(result) => {
            // Import CFDIs from every downloaded ZIP into MongoDB
            for package in &result.packages {
                match fs::read(&package.path).await {
                    Ok(zip_bytes) => {
                        match cfdi::import_zip(&state.cfdis, &slug, &zip_bytes).await {
                            Ok(imported) => {
                                eprintln!(
                                    "[cfdi] imported {} CFDIs from {}",
                                    imported.len(),
                                    package.package_id
                                );
                            }
                            Err(e) => {
                                eprintln!("[cfdi] import error for {}: {e}", package.package_id)
                            }
                        }
                    }
                    Err(e) => eprintln!("[cfdi] could not read ZIP {}: {e}", package.path),
                }
            }
            (StatusCode::OK, Json(result)).into_response()
        }
        Err(err) => {
            let status = match err {
                SatError::BadRequest(_) => StatusCode::BAD_REQUEST,
                SatError::Crypto(_) => StatusCode::BAD_REQUEST,
                SatError::Sat(_) => StatusCode::BAD_GATEWAY,
                SatError::Http(_) => StatusCode::BAD_GATEWAY,
                SatError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
                SatError::Parse(_) => StatusCode::BAD_GATEWAY,
            };
            (status, Json(err.body())).into_response()
        }
    }
}

fn is_allowed_sat_path(path: Option<&str>) -> bool {
    let Some(path) = path else {
        return true;
    };
    let path = Path::new(path);
    if path.is_absolute() {
        return false;
    }
    let mut components = path.components();
    matches!(components.next(), Some(std::path::Component::Normal(first)) if first == "uploads")
        && matches!(components.next(), Some(std::path::Component::Normal(second)) if second == "sat")
        && !components.any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
}
