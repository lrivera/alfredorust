use std::sync::Arc;

use askama::Template;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::filters;
use crate::{session::SessionUser, state::AppState};

const PER_PAGE: u64 = 50;
const API_LIMIT: i64 = 5000;

#[derive(Template)]
#[template(path = "admin/cfdis/index.html")]
struct CfdisIndexTemplate {
    cfdis: Vec<CfdiRow>,
    page: u64,
    total_pages: u64,
    total: u64,
}

struct CfdiRow {
    uuid: String,
    tipo: String,
    emisor_rfc: String,
    emisor_nombre: String,
    receptor_rfc: String,
    receptor_nombre: String,
    total: String,
    moneda: String,
    fecha: String,
}

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]
    page: u64,
}

fn default_page() -> u64 { 1 }

fn str_field(doc: &bson::Document, key: &str) -> String {
    doc.get_str(key).unwrap_or("").to_string()
}

fn nested_str(doc: &bson::Document, nested: &str, key: &str) -> String {
    doc.get_document(nested)
        .ok()
        .and_then(|d| d.get_str(key).ok())
        .unwrap_or("")
        .to_string()
}

pub async fn cfdis_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> Result<Html<String>, StatusCode> {
    let active_company = session_user.active_company_id();
    let filter = bson::doc! { "company_id": active_company.to_hex() };

    let total = state
        .cfdis
        .count_documents(filter.clone())
        .await
        .unwrap_or(0);

    let total_pages = (total + PER_PAGE - 1) / PER_PAGE;
    let page = q.page.max(1).min(total_pages.max(1));
    let skip = (page - 1) * PER_PAGE;

    let opts = mongodb::options::FindOptions::builder()
        .sort(bson::doc! { "comprobante.fecha": -1 })
        .skip(skip)
        .limit(PER_PAGE as i64)
        .build();

    let mut cursor = state
        .cfdis
        .find(filter)
        .with_options(opts)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut rows = Vec::new();
    while let Some(doc) = cursor.try_next().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        let comp = doc.get_document("comprobante").ok();
        rows.push(CfdiRow {
            uuid: str_field(&doc, "uuid"),
            tipo: comp.map(|c| str_field(c, "tipoDeComprobante")).unwrap_or_default(),
            emisor_rfc: nested_str(&doc, "emisor", "rfc"),
            emisor_nombre: nested_str(&doc, "emisor", "nombre"),
            receptor_rfc: nested_str(&doc, "receptor", "rfc"),
            receptor_nombre: nested_str(&doc, "receptor", "nombre"),
            total: comp.map(|c| str_field(c, "total")).unwrap_or_default(),
            moneda: comp.map(|c| str_field(c, "moneda")).unwrap_or_default(),
            fecha: comp.map(|c| str_field(c, "fecha")).unwrap_or_default(),
        });
    }

    CfdisIndexTemplate { cfdis: rows, page, total_pages, total }
        .render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── JSON API for the React dashboard ──────────────────────────────────────

#[derive(Serialize)]
pub struct CfdiApiItem {
    pub uuid: String,
    pub folio: String,
    pub tipo: String,
    pub fecha: String,
    pub subtotal: f64,
    pub iva: f64,
    pub total: f64,
    pub moneda: String,
    pub forma_pago: String,
    pub metodo_pago: String,
    pub emisor_rfc: String,
    pub emisor_nombre: String,
    pub receptor_rfc: String,
    pub receptor_nombre: String,
    pub concepto: String,
}

fn parse_f64(s: &str) -> f64 {
    s.trim().parse().unwrap_or(0.0)
}

pub async fn cfdis_data_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CfdiApiItem>>, StatusCode> {
    let active_company = session_user.active_company_id();
    let filter = bson::doc! { "company_id": active_company.to_hex() };

    let opts = mongodb::options::FindOptions::builder()
        .sort(bson::doc! { "comprobante.fecha": -1 })
        .limit(API_LIMIT)
        .build();

    let mut cursor = state
        .cfdis
        .find(filter)
        .with_options(opts)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut items: Vec<CfdiApiItem> = Vec::new();

    while let Some(doc) = cursor.try_next().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        let (folio, tipo, fecha, subtotal, iva, total, moneda, forma_pago, metodo_pago, concepto) =
            if let Ok(comp) = doc.get_document("comprobante") {
                let folio = str_field(comp, "folio");
                let tipo  = str_field(comp, "tipoDeComprobante");
                let fecha = str_field(comp, "fecha");
                let subtotal = parse_f64(comp.get_str("subTotal").unwrap_or("0"));
                let total    = parse_f64(comp.get_str("total").unwrap_or("0"));
                let moneda   = str_field(comp, "moneda");
                let forma_pago  = str_field(comp, "formaPago");
                let metodo_pago = str_field(comp, "metodoPago");

                let iva = doc.get_document("impuestos").ok()
                    .and_then(|imp| imp.get_str("totalImpuestosTrasladados").ok())
                    .map(parse_f64)
                    .unwrap_or(0.0);

                let concepto = doc.get_array("conceptos").ok()
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_document())
                    .and_then(|d| d.get_str("descripcion").ok())
                    .unwrap_or("")
                    .to_string();

                (folio, tipo, fecha, subtotal, iva, total, moneda, forma_pago, metodo_pago, concepto)
            } else {
                Default::default()
            };

        items.push(CfdiApiItem {
            uuid: str_field(&doc, "uuid"),
            folio,
            tipo,
            fecha,
            subtotal,
            iva,
            total,
            moneda,
            forma_pago,
            metodo_pago,
            emisor_rfc:    nested_str(&doc, "emisor", "rfc"),
            emisor_nombre: nested_str(&doc, "emisor", "nombre"),
            receptor_rfc:    nested_str(&doc, "receptor", "rfc"),
            receptor_nombre: nested_str(&doc, "receptor", "nombre"),
            concepto,
        });
    }

    Ok(Json(items))
}
