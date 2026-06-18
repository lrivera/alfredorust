//! Read-only reporting types: the time timeline (tiempo) and CFDIs.

use serde::Deserialize;

// --- tiempo (timeline) ----------------------------------------------------

/// One timeline bucket from `GET /api/tiempo`: real and planned aggregates plus
/// running cumulatives, and the individual transactions / planned entries that
/// fall in the period.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TimelineBucket {
    pub start: String,
    pub end: String,
    pub real_income: f64,
    pub real_expense: f64,
    #[serde(default)]
    pub planned_income: f64,
    #[serde(default)]
    pub planned_expense: f64,
    pub net_real: f64,
    pub net_planned: f64,
    pub cumulative_real: f64,
    pub cumulative_planned: f64,
    #[serde(default)]
    pub transactions: Vec<TxItem>,
    #[serde(default)]
    pub planned_entries: Vec<PlannedItem>,
}

/// A real transaction inside a bucket. Mirrors the backend `TxItem`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TxItem {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub amount: f64,
    #[serde(default)]
    pub date: String,
    #[serde(rename = "type", default)]
    pub tx_type: String,
}

/// A planned entry inside a bucket. Mirrors the backend `PlannedItem`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlannedItem {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub amount_estimated: f64,
    #[serde(default)]
    pub due_date: String,
    #[serde(default)]
    pub flow_type: String,
    #[serde(default)]
    pub status: String,
}

// --- cfdi (read-only) -----------------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Cfdi {
    pub uuid: String,
    #[serde(default)]
    pub folio: String,
    #[serde(default)]
    pub tipo: String,
    #[serde(default)]
    pub fecha: String,
    pub total: f64,
    #[serde(default)]
    pub moneda: String,
    #[serde(default)]
    pub emisor_nombre: String,
    #[serde(default)]
    pub receptor_nombre: String,
    pub es_emitido: bool,
}

/// `GET /api/admin/cfdis/data` returns `{ company_rfcs, items }`.
#[derive(Clone, Debug, Deserialize)]
pub struct CfdiList {
    #[serde(default)]
    pub items: Vec<Cfdi>,
}
