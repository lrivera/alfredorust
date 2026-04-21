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
use uuid::Uuid;

use crate::{
    cfdi,
    models::{ContactType, FlowType, TransactionType},
    sat::{CfdiDownloadRequest, DownloadType, download_cfdis},
    session::SessionUser,
    state::{
        AppState, CfdiJobStatus, create_transaction, get_or_create_category,
        get_or_create_contact_by_rfc, get_or_create_sat_account, get_sat_config,
    },
};

enum TxOutcome { Created, Updated }

#[derive(Deserialize)]
pub struct CfdiDownloadForm {
    pub sat_config_id: String,
    pub start: String,
    pub end: String,
    #[serde(default = "default_both")]
    pub download_type: String,
}

fn default_both() -> String { "both".to_string() }

#[derive(Serialize)]
pub struct StartedJob {
    pub job_id: String,
}

/// POST — starts the download in the background and returns a job_id immediately.
pub async fn company_cfdi_download(
    _session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
    Form(form): Form<CfdiDownloadForm>,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"company_id inválido"}))).into_response(),
    };

    let config_id = match ObjectId::from_str(&form.sat_config_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"sat_config_id inválido"}))).into_response(),
    };

    let config = match get_sat_config(&state, &config_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"Configuración SAT no encontrada"}))).into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"Error de base de datos"}))).into_response(),
    };

    let dl_types: Vec<DownloadType> = match form.download_type.as_str() {
        "received" => vec![DownloadType::Received],
        "both"     => vec![DownloadType::Issued, DownloadType::Received],
        _          => vec![DownloadType::Issued],
    };

    let job_id = Uuid::new_v4().to_string();

    // Register the job as Running
    state.jobs.lock().await.insert(job_id.clone(), CfdiJobStatus::Running);

    // Spawn background task
    let state_bg = state.clone();
    let job_id_bg = job_id.clone();
    let start = form.start.clone();
    let end = form.end.clone();

    tokio::spawn(async move {
        let result = run_download(
            &state_bg,
            &company_id,
            &company_object_id,
            config.cer_path,
            config.key_path,
            config.key_password,
            config.rfc,
            dl_types,
            start,
            end,
        )
        .await;

        state_bg.jobs.lock().await.insert(job_id_bg, result);
    });

    (StatusCode::ACCEPTED, Json(StartedJob { job_id })).into_response()
}

/// GET — poll job status.
pub async fn company_cfdi_job_status(
    _session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path((_company_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let jobs = state.jobs.lock().await;
    match jobs.get(&job_id) {
        Some(status) => (StatusCode::OK, Json(status.clone())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"job no encontrado"}))).into_response(),
    }
}

async fn run_download(
    state: &AppState,
    company_id: &str,
    company_object_id: &ObjectId,
    cer_path: String,
    key_path: String,
    key_password: String,
    rfc: String,
    dl_types: Vec<DownloadType>,
    start: String,
    end: String,
) -> CfdiJobStatus {
    // Store (dl_type, cfdi) so we know whether each cfdi was issued or received
    let mut all_imported: Vec<(DownloadType, crate::cfdi::ImportedCfdi)> = Vec::new();
    let mut errors = Vec::new();

    for dl_type in dl_types {
        let request = CfdiDownloadRequest {
            cer_path: Some(cer_path.clone()),
            key_path: Some(key_path.clone()),
            key_password: Some(key_password.clone()),
            rfc: Some(rfc.clone()),
            download_type: dl_type,
            request_type: crate::sat::RequestType::Xml,
            start: Some(start.clone()),
            end: Some(end.clone()),
            output_dir: None,
            poll_seconds: None,
            max_attempts: None,
        };

        let download_result = match download_cfdis(company_id, request).await {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Error SAT ({}): {e}", dl_type.env_value()));
                continue;
            }
        };

        for package in &download_result.packages {
            match fs::read(&package.path).await {
                Ok(zip_bytes) => {
                    match cfdi::import_zip(&state.cfdis, company_id, &zip_bytes).await {
                        Ok(imported) => {
                            eprintln!("[cfdi] imported {} from {}", imported.len(), package.package_id);
                            all_imported.extend(imported.into_iter().map(|c| (dl_type, c)));
                        }
                        Err(e) => errors.push(format!("Error importando {}: {e}", package.package_id)),
                    }
                }
                Err(e) => errors.push(format!("No se pudo leer ZIP {}: {e}", package.path)),
            }
        }
    }

    let mut tx_created = 0usize;
    let mut tx_updated = 0usize;
    let mut tx_skipped = 0usize;

    let income_category = get_or_create_category(state, company_object_id, "CFDIs Importados (Ingresos)", FlowType::Income).await;
    let expense_category = get_or_create_category(state, company_object_id, "CFDIs Importados (Egresos)", FlowType::Expense).await;
    let sat_account = get_or_create_sat_account(state, company_object_id).await;

    for (dl_type, cfdi_item) in &all_imported {
        // Only process Ingreso/Egreso; skip Pago, Traslado, Nómina, etc.
        if cfdi_item.tipo_de_comprobante != "I" && cfdi_item.tipo_de_comprobante != "E" {
            tx_skipped += 1;
            continue;
        }

        // Classify based on download direction, not TipoDeComprobante.
        // Issued = we are the Emisor = Income for us.
        // Received = we are the Receptor = Expense for us.
        let (tx_type, category_result) = match dl_type {
            DownloadType::Issued   => (TransactionType::Income,  &income_category),
            DownloadType::Received => (TransactionType::Expense, &expense_category),
        };

        let category_id = match category_result {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("Error categoría para {}: {e}", cfdi_item.uuid));
                tx_skipped += 1;
                continue;
            }
        };

        let sat_account_id = match &sat_account {
            Ok(id) => id.clone(),
            Err(e) => {
                errors.push(format!("Error cuenta SAT: {e}"));
                tx_skipped += 1;
                continue;
            }
        };

        let amount: f64 = cfdi_item.total.parse().unwrap_or(0.0);
        if amount == 0.0 { tx_skipped += 1; continue; }
        let date = parse_cfdi_date(&cfdi_item.fecha);

        let (account_from_id, account_to_id) = match tx_type {
            TransactionType::Income  => (None, Some(sat_account_id)),
            TransactionType::Expense => (Some(sat_account_id), None),
            TransactionType::Transfer => (None, None),
        };

        // Issued: customer = Receptor (who we billed). Received: supplier = Emisor (who billed us).
        let (contact_rfc, contact_name, contact_type, description) = match dl_type {
            DownloadType::Issued => (
                cfdi_item.receptor_rfc.as_str(),
                cfdi_item.receptor_nombre.as_str(),
                ContactType::Customer,
                format!("{} — {}", cfdi_item.receptor_nombre, cfdi_item.uuid),
            ),
            DownloadType::Received => (
                cfdi_item.emisor_rfc.as_str(),
                cfdi_item.emisor_nombre.as_str(),
                ContactType::Supplier,
                format!("{} — {}", cfdi_item.emisor_nombre, cfdi_item.uuid),
            ),
        };

        let contact_id = if !contact_rfc.is_empty() {
            get_or_create_contact_by_rfc(state, company_object_id, contact_rfc, contact_name, contact_type).await.ok()
        } else {
            None
        };

        let existing = state.transactions.find_one(bson::doc! { "cfdi_uuid": &cfdi_item.uuid }).await.ok().flatten();

        let outcome = if let Some(existing_tx) = existing {
            let res = state.transactions.update_one(
                bson::doc! { "_id": &existing_tx.id },
                bson::doc! { "$set": {
                    "amount": amount,
                    "date": date,
                    "description": &description,
                    "transaction_type": tx_type.as_str(),
                    "category_id": category_id,
                    "account_from_id": &account_from_id,
                    "account_to_id": &account_to_id,
                    "contact_id": &contact_id,
                    "updated_at": bson::DateTime::now(),
                }},
            ).await;
            match res {
                Ok(_) => TxOutcome::Updated,
                Err(e) => { errors.push(format!("Error actualizando {}: {e}", cfdi_item.uuid)); tx_skipped += 1; continue; }
            }
        } else {
            match create_transaction(state, company_object_id, date, &description, tx_type, category_id, account_from_id, account_to_id, amount, None, true, None, Some(cfdi_item.uuid.clone()), contact_id).await {
                Ok(_) => TxOutcome::Created,
                Err(e) => { errors.push(format!("Error transacción {}: {e}", cfdi_item.uuid)); tx_skipped += 1; continue; }
            }
        };

        match outcome {
            TxOutcome::Created => tx_created += 1,
            TxOutcome::Updated => tx_updated += 1,
        }
    }

    CfdiJobStatus::Done {
        imported: all_imported.len(),
        transactions_created: tx_created,
        transactions_updated: tx_updated,
        transactions_skipped: tx_skipped,
        errors,
    }
}

fn parse_cfdi_date(fecha: &str) -> bson::DateTime {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(fecha, "%Y-%m-%dT%H:%M:%S") {
        let utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
        return bson::DateTime::from_millis(utc.timestamp_millis());
    }
    bson::DateTime::now()
}
