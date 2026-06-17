use std::{str::FromStr, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Form, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bson::oid::ObjectId;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::Semaphore, time::sleep};
use uuid::Uuid;

use crate::{
    cfdi,
    models::{ContactType, FlowType},
    sat::{CfdiDownloadRequest, DownloadType, download_cfdis},
    session::SessionUser,
    state::{
        AppState, CfdiJob, CfdiJobStatus, create_or_update_planned_entry_from_cfdi,
        get_or_create_category, get_or_create_contact_by_rfc, get_or_create_sat_account,
        get_sat_config, pay_planned_entry,
    },
};

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

enum PlannedOutcome {
    Created,
    Updated,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CfdiDownloadForm {
    pub sat_config_id: String,
    pub start: String,
    pub end: String,
    #[serde(default = "default_both")]
    pub download_type: String,
    #[serde(default)]
    pub auto_create_payments: bool,
}

fn default_both() -> String {
    "both".to_string()
}

#[derive(Serialize)]
pub struct StartedJobInfo {
    pub job_id: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct StartedJobs {
    pub jobs: Vec<StartedJobInfo>,
}

// ── POST: start download (one job per monthly chunk) ───────────────────────

#[utoipa::path(
    post,
    path = "/admin/companies/{id}/cfdi/download",
    tag = "cfdi",
    params(("id" = String, Path, description = "Company id")),
    responses(
        (status = 201, description = "Download jobs started"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 400, description = "Invalid input")
    ),
    security(("session" = []))
)]
pub async fn company_cfdi_download(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
    Form(form): Form<CfdiDownloadForm>,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"company_id inválido"})),
            )
                .into_response();
        }
    };

    let config_id = match ObjectId::from_str(&form.sat_config_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"sat_config_id inválido"})),
            )
                .into_response();
        }
    };

    if let Err(status) = require_company_admin(&session_user, &company_object_id) {
        return status.into_response();
    }

    let config = match get_sat_config(&state, &config_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error":"Configuración SAT no encontrada"})),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"Error de base de datos"})),
            )
                .into_response();
        }
    };

    if config.company_id != company_object_id {
        return StatusCode::FORBIDDEN.into_response();
    }

    let dl_types: Vec<DownloadType> = match form.download_type.as_str() {
        "received" => vec![DownloadType::Received],
        "both" => vec![DownloadType::Issued, DownloadType::Received],
        _ => vec![DownloadType::Issued],
    };

    let chunks = monthly_chunks(&form.start, &form.end);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut started = Vec::new();

    // Max 3 concurrent SAT requests; shared across all monthly jobs.
    let semaphore = Arc::new(Semaphore::new(3));

    for (label, chunk_start, chunk_end) in chunks {
        let job_id = Uuid::new_v4().to_string();

        state.jobs.lock().await.insert(
            job_id.clone(),
            CfdiJob {
                job_id: job_id.clone(),
                company_id: company_id.clone(),
                label: label.clone(),
                chunk_start: chunk_start.clone(),
                started_at: today.clone(),
                status: CfdiJobStatus::Queued,
            },
        );

        let state_bg = state.clone();
        let job_id_bg = job_id.clone();
        let company_id_bg = company_id.clone();
        let company_oid_bg = company_object_id.clone();
        let dl_types_bg = dl_types.clone();
        let cer = config.cer_path.clone();
        let key = config.key_path.clone();
        let pwd = config.key_password.clone();
        let rfc = config.rfc.clone();
        let sem = semaphore.clone();
        let auto_create_payments = form.auto_create_payments;

        tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            // Mark as actively running now that we have a semaphore slot.
            {
                let mut jobs = state_bg.jobs.lock().await;
                if let Some(job) = jobs.get_mut(&job_id_bg) {
                    job.status = CfdiJobStatus::Running;
                }
            }

            let result = run_download(
                &state_bg,
                &company_id_bg,
                &company_oid_bg,
                cer.clone(),
                key.clone(),
                pwd.clone(),
                rfc.clone(),
                dl_types_bg.clone(),
                auto_create_payments,
                chunk_start.clone(),
                chunk_end.clone(),
            )
            .await;

            // 1 retry after 10s if the download failed completely.
            let result = if should_retry(&result) {
                sleep(Duration::from_secs(10)).await;
                run_download(
                    &state_bg,
                    &company_id_bg,
                    &company_oid_bg,
                    cer,
                    key,
                    pwd,
                    rfc,
                    dl_types_bg,
                    auto_create_payments,
                    chunk_start,
                    chunk_end,
                )
                .await
            } else {
                result
            };

            let mut jobs = state_bg.jobs.lock().await;
            if let Some(job) = jobs.get_mut(&job_id_bg) {
                job.status = result;
            }
        });

        started.push(StartedJobInfo { job_id, label });
    }

    (StatusCode::ACCEPTED, Json(StartedJobs { jobs: started })).into_response()
}

// ── GET: list all jobs for a company ───────────────────────────────────────

#[utoipa::path(
    get,
    path = "/admin/companies/{id}/cfdi/jobs",
    tag = "cfdi",
    params(("id" = String, Path, description = "Company id")),
    responses(
        (status = 200, description = "List of CFDI download jobs"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn company_cfdi_jobs_list(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&company_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(status) = require_company_admin(&session_user, &company_object_id) {
        return status.into_response();
    }

    let jobs = state.jobs.lock().await;
    let mut result: Vec<&CfdiJob> = jobs
        .values()
        .filter(|j| j.company_id == company_id)
        .collect();
    result.sort_by(|a, b| a.chunk_start.cmp(&b.chunk_start));
    (StatusCode::OK, Json(result)).into_response()
}

// ── GET: single job status ─────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/admin/companies/{id}/cfdi/jobs/{job_id}",
    tag = "cfdi",
    params(
        ("id" = String, Path, description = "Company id"),
        ("job_id" = String, Path, description = "Job id")
    ),
    responses(
        (status = 200, description = "CFDI download job status"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn company_cfdi_job_status(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path((_company_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let company_object_id = match ObjectId::from_str(&_company_id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(status) = require_company_admin(&session_user, &company_object_id) {
        return status.into_response();
    }

    let jobs = state.jobs.lock().await;
    match jobs.get(&job_id) {
        Some(job) if job.company_id == _company_id => {
            (StatusCode::OK, Json(job.clone())).into_response()
        }
        Some(_) => StatusCode::FORBIDDEN.into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"job no encontrado"})),
        )
            .into_response(),
    }
}

// ── Background download logic ──────────────────────────────────────────────

async fn run_download(
    state: &AppState,
    company_id: &str,
    company_object_id: &ObjectId,
    cer_path: String,
    key_path: String,
    key_password: String,
    rfc: String,
    dl_types: Vec<DownloadType>,
    auto_create_payments: bool,
    start: String,
    end: String,
) -> CfdiJobStatus {
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
                            eprintln!(
                                "[cfdi] imported {} from {}",
                                imported.len(),
                                package.package_id
                            );
                            all_imported.extend(imported.into_iter().map(|c| (dl_type, c)));
                        }
                        Err(e) => {
                            errors.push(format!("Error importando {}: {e}", package.package_id))
                        }
                    }
                }
                Err(e) => errors.push(format!("No se pudo leer ZIP {}: {e}", package.path)),
            }
        }
    }

    let mut commitments_created = 0usize;
    let mut commitments_updated = 0usize;
    let mut commitments_skipped = 0usize;

    let income_category = get_or_create_category(
        state,
        company_object_id,
        "CFDIs Importados (Ingresos)",
        FlowType::Income,
    )
    .await;
    let expense_category = get_or_create_category(
        state,
        company_object_id,
        "CFDIs Importados (Egresos)",
        FlowType::Expense,
    )
    .await;
    let sat_account = get_or_create_sat_account(state, company_object_id).await;

    for (dl_type, cfdi_item) in &all_imported {
        if cfdi_item.tipo_de_comprobante != "I" && cfdi_item.tipo_de_comprobante != "E" {
            commitments_skipped += 1;
            continue;
        }

        let (flow_type, category_result) = match dl_type {
            DownloadType::Issued => (FlowType::Income, &income_category),
            DownloadType::Received => (FlowType::Expense, &expense_category),
        };

        let category_id = match category_result {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("Error categoría para {}: {e}", cfdi_item.uuid));
                commitments_skipped += 1;
                continue;
            }
        };

        let sat_account_id = match &sat_account {
            Ok(id) => id.clone(),
            Err(e) => {
                errors.push(format!("Error cuenta SAT: {e}"));
                commitments_skipped += 1;
                continue;
            }
        };

        let amount: f64 = cfdi_item.total.parse().unwrap_or(0.0);
        if amount == 0.0 {
            commitments_skipped += 1;
            continue;
        }
        let date = parse_cfdi_date(&cfdi_item.fecha);

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
            get_or_create_contact_by_rfc(
                state,
                company_object_id,
                contact_rfc,
                contact_name,
                contact_type,
            )
            .await
            .ok()
        } else {
            None
        };

        let currency = Some(cfdi_item.moneda.clone()).filter(|s| !s.is_empty());
        let folio = Some(cfdi_item.folio.clone()).filter(|s| !s.is_empty());
        let (planned_entry_id, created) = match create_or_update_planned_entry_from_cfdi(
            state,
            company_object_id,
            date,
            &description,
            flow_type,
            category_id,
            &sat_account_id,
            contact_id,
            amount,
            &cfdi_item.uuid,
            currency,
            folio,
            None,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => {
                errors.push(format!("Error compromiso CFDI {}: {e}", cfdi_item.uuid));
                commitments_skipped += 1;
                continue;
            }
        };

        let outcome = if created {
            PlannedOutcome::Created
        } else {
            PlannedOutcome::Updated
        };

        if auto_create_payments {
            let existing_payment = state
                .transactions
                .find_one(bson::doc! { "company_id": company_object_id, "planned_entry_id": &planned_entry_id })
                .await
                .ok()
                .flatten();
            if existing_payment.is_none() {
                if let Err(e) = pay_planned_entry(
                    state,
                    &planned_entry_id,
                    company_object_id,
                    &sat_account_id,
                    amount,
                    date,
                    None,
                )
                .await
                {
                    errors.push(format!("Error pago automático {}: {e}", cfdi_item.uuid));
                }
            }
        }

        match outcome {
            PlannedOutcome::Created => commitments_created += 1,
            PlannedOutcome::Updated => commitments_updated += 1,
        }
    }

    CfdiJobStatus::Done {
        imported: all_imported.len(),
        transactions_created: commitments_created,
        transactions_updated: commitments_updated,
        transactions_skipped: commitments_skipped,
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

// ── Monthly chunking ───────────────────────────────────────────────────────

fn monthly_chunks(start_iso: &str, end_iso: &str) -> Vec<(String, String, String)> {
    let start_str = &start_iso[..start_iso.len().min(10)];
    let end_str = &end_iso[..end_iso.len().min(10)];

    let start = match NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return vec![(
                "Descarga".into(),
                start_iso.to_string(),
                end_iso.to_string(),
            )];
        }
    };
    let end = match NaiveDate::parse_from_str(end_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return vec![(
                "Descarga".into(),
                start_iso.to_string(),
                end_iso.to_string(),
            )];
        }
    };

    if start > end {
        return vec![];
    }

    // The SAT interprets dates in Mexico City time (UTC-6 CST / UTC-5 CDT).
    // Subtracting 6h from UTC gives a safe approximation that's always in the
    // past from the SAT's perspective, even during CDT (+1h buffer).
    let mexico_now = chrono::Utc::now() - chrono::Duration::hours(6);
    let end = end.min(mexico_now.date_naive());

    if start > end {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut month_cursor = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();

    loop {
        let chunk_from =
            if month_cursor.year() == start.year() && month_cursor.month() == start.month() {
                start
            } else {
                month_cursor
            };

        let next_month = next_month_start(month_cursor);
        let month_last = next_month.pred_opt().unwrap();
        let chunk_to = month_last.min(end);

        let label = format!(
            "{} {}",
            month_name_es(month_cursor.month()),
            month_cursor.year()
        );
        // Use current Mexico City time for today's chunk so we don't send a future timestamp.
        let end_time = if chunk_to == mexico_now.date_naive() {
            format!("{}T{}", chunk_to, mexico_now.format("%H:%M:%S"))
        } else {
            format!("{}T23:59:59", chunk_to)
        };
        chunks.push((label, format!("{}T00:00:00", chunk_from), end_time));

        if month_last >= end {
            break;
        }
        month_cursor = next_month;
    }

    chunks
}

fn next_month_start(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap()
}

fn month_name_es(month: u32) -> &'static str {
    match month {
        1 => "Ene",
        2 => "Feb",
        3 => "Mar",
        4 => "Abr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Ago",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dic",
        _ => "???",
    }
}

/// Retry if the download failed completely: no CFDIs imported and at least one error.
fn should_retry(status: &CfdiJobStatus) -> bool {
    match status {
        CfdiJobStatus::Done {
            imported, errors, ..
        } => *imported == 0 && !errors.is_empty(),
        CfdiJobStatus::Failed { .. } => true,
        CfdiJobStatus::Running | CfdiJobStatus::Queued => false,
    }
}
