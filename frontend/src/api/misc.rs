//! Read-only reporting types: the time timeline (tiempo) and CFDIs.

use serde::Deserialize;

// --- tiempo (timeline) ----------------------------------------------------

/// One timeline bucket from `GET /api/tiempo`. Summary fields only.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TimelineBucket {
    pub start: String,
    pub end: String,
    pub real_income: f64,
    pub real_expense: f64,
    pub net_real: f64,
    pub net_planned: f64,
    pub cumulative_real: f64,
    pub cumulative_planned: f64,
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
