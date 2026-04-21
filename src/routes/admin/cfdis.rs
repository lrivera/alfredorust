use std::sync::Arc;

use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use futures::stream::TryStreamExt;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;
use crate::{session::SessionUser, state::AppState};

const PER_PAGE: u64 = 50;

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
