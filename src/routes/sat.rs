use std::sync::Arc;

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
    Json(payload): Json<CfdiDownloadRequest>,
) -> impl IntoResponse {
    let slug = session_user.active_company_slug().to_string();
    match download_cfdis(&slug, payload).await {
        Ok(result) => {
            // Import CFDIs from every downloaded ZIP into MongoDB
            for package in &result.packages {
                match fs::read(&package.path).await {
                    Ok(zip_bytes) => {
                        match cfdi::import_zip(&state.cfdis, &slug, &zip_bytes).await {
                            Ok(uuids) => {
                                eprintln!("[cfdi] imported {} CFDIs from {}", uuids.len(), package.package_id);
                            }
                            Err(e) => eprintln!("[cfdi] import error for {}: {e}", package.package_id),
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
