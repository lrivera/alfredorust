use std::{collections::HashMap, sync::Arc};

use askama::Template;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::Html,
};
use chrono::{
    DateTime as ChronoDateTime, Datelike, Months, SecondsFormat, TimeZone, Timelike, Utc,
};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

use crate::{
    models::{FlowType, PlannedStatus, TransactionType},
    session::SessionUser,
    state::AppState,
};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "tiempo/index.html")]
struct TiempoTemplate {}

pub async fn tiempo_page(_session: SessionUser) -> Result<Html<String>, StatusCode> {
    render(TiempoTemplate {})
}

#[derive(Deserialize)]
pub struct TiempoQuery {
    mode: String,
    from: String,
    to: String,
}

#[derive(Serialize)]
pub struct TimelineBucket {
    start: String,
    end: String,
    real_income: f64,
    real_expense: f64,
    planned_income: f64,
    planned_expense: f64,
    net_real: f64,
    net_planned: f64,
    cumulative_real: f64,
    cumulative_planned: f64,
    transactions: Vec<TxItem>,
    planned_entries: Vec<PlannedItem>,
}

#[derive(Serialize)]
pub struct TxItem {
    id: String,
    description: String,
    amount: f64,
    date: String,
    r#type: String,
}

#[derive(Serialize)]
pub struct PlannedItem {
    id: String,
    name: String,
    amount_estimated: f64,
    due_date: String,
    flow_type: String,
    status: String,
}

#[derive(Clone, Copy)]
enum Mode {
    Day,
    Week,
    Month,
    Year,
}

impl Mode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "day" => Some(Mode::Day),
            "week" => Some(Mode::Week),
            "month" => Some(Mode::Month),
            "year" => Some(Mode::Year),
            _ => None,
        }
    }
}

pub async fn tiempo_data(
    SessionUser(session): SessionUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TiempoQuery>,
) -> Result<Json<Vec<TimelineBucket>>, StatusCode> {
    let mode = Mode::parse(&query.mode).ok_or(StatusCode::BAD_REQUEST)?;

    let start = parse_iso(&query.from).ok_or(StatusCode::BAD_REQUEST)?;
    let mut end = parse_iso(&query.to).ok_or(StatusCode::BAD_REQUEST)?;

    let max_future = Utc::now() + chrono::Duration::days(365 * 5);
    if end > max_future {
        end = max_future;
    }

    let (base_income, base_expense) =
        sum_transactions_before(&state, &session.user.company_id, start)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (base_planned_income, base_planned_expense) =
        sum_planned_before(&state, &session.user.company_id, start)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Load transactions
    let mut tx_cursor = state
        .transactions
        .find(doc! {
            "company_id": &session.user.company_id,
            "date": { "$gte": DateTime::from_chrono(start), "$lt": DateTime::from_chrono(end) }
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut buckets: HashMap<ChronoDateTime<Utc>, TimelineBucket> = HashMap::new();

    while let Some(tx) = tx_cursor
        .try_next()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let key = bucket_start(tx.date.to_chrono(), mode);
        let bucket = buckets
            .entry(key)
            .or_insert_with(|| empty_bucket(key, mode));

        match tx.transaction_type {
            TransactionType::Income => bucket.real_income += tx.amount,
            TransactionType::Expense => bucket.real_expense += tx.amount,
            TransactionType::Transfer => {}
        }
        bucket.net_real = bucket.real_income - bucket.real_expense;
        bucket.transactions.push(TxItem {
            id: tx.id.map(|i| i.to_hex()).unwrap_or_default(),
            description: tx.description,
            amount: tx.amount,
            date: fmt_iso(tx.date.to_chrono()),
            r#type: tx.transaction_type.as_str().to_string(),
        });
    }

    // Planned entries
    let mut pe_cursor = state
        .planned_entries
        .find(doc! {
            "company_id": &session.user.company_id,
            "due_date": { "$gte": DateTime::from_chrono(start), "$lt": DateTime::from_chrono(end) }
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(pe) = pe_cursor
        .try_next()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if matches!(pe.status, PlannedStatus::Cancelled) {
            continue;
        }
        let key = bucket_start(pe.due_date.to_chrono(), mode);
        let bucket = buckets
            .entry(key)
            .or_insert_with(|| empty_bucket(key, mode));

        match pe.flow_type {
            FlowType::Income => bucket.planned_income += pe.amount_estimated,
            FlowType::Expense => bucket.planned_expense += pe.amount_estimated,
        }
        bucket.net_planned = bucket.planned_income - bucket.planned_expense;
        bucket.planned_entries.push(PlannedItem {
            id: pe.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: pe.name,
            amount_estimated: pe.amount_estimated,
            due_date: fmt_iso(pe.due_date.to_chrono()),
            flow_type: pe.flow_type.as_str().to_string(),
            status: pe.status.as_str().to_string(),
        });
    }

    let mut list: Vec<TimelineBucket> = Vec::new();
    let mut running_real = base_income - base_expense;
    let mut running_planned = base_planned_income - base_planned_expense;

    let mut cursor_start = bucket_start(start, mode);
    let end_limit = bucket_start(end, mode);

    while cursor_start < end_limit {
        let mut bucket = buckets
            .remove(&cursor_start)
            .unwrap_or_else(|| empty_bucket(cursor_start, mode));
        running_real += bucket.net_real;
        running_planned += bucket.net_planned;
        bucket.cumulative_real = running_real;
        bucket.cumulative_planned = running_planned;
        list.push(bucket);
        cursor_start = next_bucket(cursor_start, mode);
    }

    Ok(Json(list))
}

fn empty_bucket(start: ChronoDateTime<Utc>, mode: Mode) -> TimelineBucket {
    let end = match mode {
        Mode::Day => start + chrono::Duration::days(1),
        Mode::Week => start + chrono::Duration::days(7),
        Mode::Month => {
            let mut d = start;
            d = d
                .with_day(1)
                .unwrap_or(start)
                .with_month(start.month() % 12 + 1)
                .unwrap_or(start);
            if start.month() == 12 {
                d = d.with_year(start.year() + 1).unwrap_or(d);
            }
            d
        }
        Mode::Year => start
            .with_month(1)
            .unwrap_or(start)
            .with_year(start.year() + 1)
            .unwrap_or(start),
    };

    TimelineBucket {
        start: fmt_iso(start),
        end: fmt_iso(end),
        real_income: 0.0,
        real_expense: 0.0,
        planned_income: 0.0,
        planned_expense: 0.0,
        net_real: 0.0,
        net_planned: 0.0,
        cumulative_real: 0.0,
        cumulative_planned: 0.0,
        transactions: Vec::new(),
        planned_entries: Vec::new(),
    }
}

fn bucket_start(dt: ChronoDateTime<Utc>, mode: Mode) -> ChronoDateTime<Utc> {
    match mode {
        Mode::Day => dt
            .with_hour(0)
            .and_then(|d| d.with_minute(0))
            .and_then(|d| d.with_second(0))
            .and_then(|d| d.with_nanosecond(0))
            .unwrap_or(dt),
        Mode::Week => {
            let weekday = dt.weekday().num_days_from_monday() as i64;
            let start = dt - chrono::Duration::days(weekday);
            start
                .with_hour(0)
                .and_then(|d| d.with_minute(0))
                .and_then(|d| d.with_second(0))
                .and_then(|d| d.with_nanosecond(0))
                .unwrap_or(start)
        }
        Mode::Month => Utc
            .with_ymd_and_hms(dt.year(), dt.month(), 1, 0, 0, 0)
            .single()
            .unwrap_or(dt),
        Mode::Year => Utc
            .with_ymd_and_hms(dt.year(), 1, 1, 0, 0, 0)
            .single()
            .unwrap_or(dt),
    }
}

fn next_bucket(dt: ChronoDateTime<Utc>, mode: Mode) -> ChronoDateTime<Utc> {
    match mode {
        Mode::Day => dt + chrono::Duration::days(1),
        Mode::Week => dt + chrono::Duration::days(7),
        Mode::Month => dt + Months::new(1),
        Mode::Year => dt + Months::new(12),
    }
}

fn parse_iso(value: &str) -> Option<ChronoDateTime<Utc>> {
    ChronoDateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn fmt_iso(dt: ChronoDateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Millis, true)
}

async fn sum_transactions_before(
    state: &AppState,
    company_id: &ObjectId,
    before: ChronoDateTime<Utc>,
) -> mongodb::error::Result<(f64, f64)> {
    let pipeline = vec![
        doc! { "$match": {
            "company_id": company_id,
            "date": { "$lt": DateTime::from_chrono(before) },
        }},
        doc! { "$group": {
            "_id": "$transaction_type",
            "total": { "$sum": "$amount" },
        }},
    ];
    let mut income = 0.0;
    let mut expense = 0.0;
    let mut cursor = state.transactions.aggregate(pipeline).await?;
    while let Some(doc) = cursor.try_next().await? {
        if let Some(kind) = doc.get_str("_id").ok() {
            let total = doc.get_f64("total").unwrap_or(0.0);
            match kind {
                "income" => income += total,
                "expense" => expense += total,
                _ => {}
            }
        }
    }
    Ok((income, expense))
}

async fn sum_planned_before(
    state: &AppState,
    company_id: &ObjectId,
    before: ChronoDateTime<Utc>,
) -> mongodb::error::Result<(f64, f64)> {
    let pipeline = vec![
        doc! { "$match": {
            "company_id": company_id,
            "due_date": { "$lt": DateTime::from_chrono(before) },
            "status": { "$ne": PlannedStatus::Cancelled.as_str() },
        }},
        doc! { "$group": {
            "_id": "$flow_type",
            "total": { "$sum": "$amount_estimated" },
        }},
    ];
    let mut income = 0.0;
    let mut expense = 0.0;
    let mut cursor = state.planned_entries.aggregate(pipeline).await?;
    while let Some(doc) = cursor.try_next().await? {
        if let Some(kind) = doc.get_str("_id").ok() {
            let total = doc.get_f64("total").unwrap_or(0.0);
            match kind {
                "income" => income += total,
                "expense" => expense += total,
                _ => {}
            }
        }
    }
    Ok((income, expense))
}
