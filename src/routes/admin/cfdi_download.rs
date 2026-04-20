use std::{str::FromStr, sync::Arc};

use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    cfdi,
    models::{ContactType, FlowType, TransactionType},
    sat::{CfdiDownloadRequest, DownloadType, download_cfdis},
    session::SessionUser,
    state::{AppState, create_transaction, get_or_create_category, get_or_create_contact_by_rfc, get_sat_config},
};

/// Counter tracking whether a transaction was created or updated.
enum TxOutcome { Created, Updated }

#[derive(Deserialize)]
pub struct CfdiDownloadForm {
    pub sat_config_id: String,
    pub start: String,
    pub end: String,
    #[serde(default = "default_issued")]
    pub download_type: String,
}

fn default_issued() -> String {
    "issued".to_string()
}

#[derive(Serialize)]
pub struct CfdiDownloadResult {
    pub ok: bool,
    pub imported: usize,
    pub transactions_created: usize,
    pub transactions_updated: usize,
    pub transactions_skipped: usize,
    pub errors: Vec<String>,
}

pub async fn company_cfdi_download(
    _session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
    Form(form): Form<CfdiDownloadForm>,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(err("company_id inválido"))).into_response(),
    };

    let config_id = match ObjectId::from_str(&form.sat_config_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(err("sat_config_id inválido"))).into_response(),
    };

    let config = match get_sat_config(&state, &config_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(err("Configuración SAT no encontrada"))).into_response(),
        Err(e) => {
            eprintln!("[cfdi_download] db error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err("Error de base de datos"))).into_response();
        }
    };

    let dl_types: &[DownloadType] = match form.download_type.as_str() {
        "received" => &[DownloadType::Received],
        "both" => &[DownloadType::Issued, DownloadType::Received],
        _ => &[DownloadType::Issued],
    };

    let mut all_imported = Vec::new();
    let mut errors = Vec::new();

    for &dl_type in dl_types {
        let request = CfdiDownloadRequest {
            cer_path: Some(config.cer_path.clone()),
            key_path: Some(config.key_path.clone()),
            key_password: Some(config.key_password.clone()),
            rfc: Some(config.rfc.clone()),
            download_type: dl_type,
            request_type: crate::sat::RequestType::Xml,
            start: Some(form.start.clone()),
            end: Some(form.end.clone()),
            output_dir: None,
            poll_seconds: None,
            max_attempts: None,
        };

        let download_result = match download_cfdis(&company_id, request).await {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Error SAT ({}): {e}", dl_type.env_value()));
                continue;
            }
        };

        for package in &download_result.packages {
            match fs::read(&package.path).await {
                Ok(zip_bytes) => {
                    match cfdi::import_zip(&state.cfdis, &company_id, &zip_bytes).await {
                        Ok(imported) => {
                            eprintln!(
                                "[cfdi_download] imported {} from {}",
                                imported.len(),
                                package.package_id
                            );
                            all_imported.extend(imported);
                        }
                        Err(e) => {
                            errors.push(format!("Error importando {}: {e}", package.package_id));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("No se pudo leer ZIP {}: {e}", package.path));
                }
            }
        }
    }

    // Create transactions for imported CFDIs
    let mut tx_created = 0usize;
    let mut tx_updated = 0usize;
    let mut tx_skipped = 0usize;

    // Look up or create default categories once
    let income_category = get_or_create_category(
        &state,
        &company_object_id,
        "CFDIs Importados (Ingresos)",
        FlowType::Income,
    )
    .await;

    let expense_category = get_or_create_category(
        &state,
        &company_object_id,
        "CFDIs Importados (Egresos)",
        FlowType::Expense,
    )
    .await;

    for cfdi_item in &all_imported {
        let (tx_type, category_result) = match cfdi_item.tipo_de_comprobante.as_str() {
            "I" => (TransactionType::Income, &income_category),
            "E" => (TransactionType::Expense, &expense_category),
            _ => {
                tx_skipped += 1;
                continue;
            }
        };

        let category_id = match category_result {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("Error categoría para {}: {e}", cfdi_item.uuid));
                tx_skipped += 1;
                continue;
            }
        };

        let amount: f64 = cfdi_item.total.parse().unwrap_or(0.0);
        let date = parse_cfdi_date(&cfdi_item.fecha);

        // For Ingresos the client is the Receptor; for Egresos it's the Emisor.
        let (contact_rfc, contact_name, contact_type) = match cfdi_item.tipo_de_comprobante.as_str() {
            "I" => (cfdi_item.receptor_rfc.as_str(), cfdi_item.receptor_nombre.as_str(), ContactType::Customer),
            _   => (cfdi_item.emisor_rfc.as_str(),   cfdi_item.emisor_nombre.as_str(),   ContactType::Supplier),
        };

        let contact_id = if !contact_rfc.is_empty() {
            get_or_create_contact_by_rfc(&state, &company_object_id, contact_rfc, contact_name, contact_type)
                .await
                .ok()
        } else {
            None
        };

        let description = match cfdi_item.tipo_de_comprobante.as_str() {
            "I" => format!("{} — {}", cfdi_item.emisor_nombre, cfdi_item.uuid),
            _ => format!("{} — {}", cfdi_item.receptor_nombre, cfdi_item.uuid),
        };

        // Check if a transaction already exists for this UUID
        let existing = state
            .transactions
            .find_one(bson::doc! { "cfdi_uuid": &cfdi_item.uuid })
            .await
            .ok()
            .flatten();

        let outcome = if let Some(existing_tx) = existing {
            // Update the existing transaction (amount, date, description may have changed in SAT)
            let update_result = state
                .transactions
                .update_one(
                    bson::doc! { "_id": &existing_tx.id },
                    bson::doc! { "$set": {
                        "amount": amount,
                        "date": date,
                        "description": &description,
                        "transaction_type": tx_type.as_str(),
                        "category_id": category_id,
                        "contact_id": &contact_id,
                        "updated_at": bson::DateTime::now(),
                    }},
                )
                .await;
            match update_result {
                Ok(_) => TxOutcome::Updated,
                Err(e) => {
                    errors.push(format!("Error actualizando {}: {e}", cfdi_item.uuid));
                    tx_skipped += 1;
                    continue;
                }
            }
        } else {
            // Create new transaction
            match create_transaction(
                &state,
                &company_object_id,
                date,
                &description,
                tx_type,
                category_id,
                None,
                None,
                amount,
                None,
                true,
                None,
                Some(cfdi_item.uuid.clone()),
                contact_id,
            )
            .await
            {
                Ok(_) => TxOutcome::Created,
                Err(e) => {
                    errors.push(format!("Error transacción {}: {e}", cfdi_item.uuid));
                    tx_skipped += 1;
                    continue;
                }
            }
        };

        match outcome {
            TxOutcome::Created => tx_created += 1,
            TxOutcome::Updated => tx_updated += 1,
        }
    }

    (
        StatusCode::OK,
        Json(CfdiDownloadResult {
            ok: true,
            imported: all_imported.len(),
            transactions_created: tx_created,
            transactions_updated: tx_updated,
            transactions_skipped: tx_skipped,
            errors,
        }),
    )
        .into_response()
}

fn parse_cfdi_date(fecha: &str) -> bson::DateTime {
    // CFDI fecha format: "2024-01-15T12:00:00"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(fecha, "%Y-%m-%dT%H:%M:%S") {
        let utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
        return bson::DateTime::from_millis(utc.timestamp_millis());
    }
    bson::DateTime::now()
}

fn err(msg: &str) -> CfdiDownloadResult {
    CfdiDownloadResult {
        ok: false,
        imported: 0,
        transactions_created: 0,
        transactions_updated: 0,
        transactions_skipped: 0,
        errors: vec![msg.to_string()],
    }
}
