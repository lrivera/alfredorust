use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use slug::slugify;
use std::time::SystemTime;

use crate::models::Company;

use super::AppState;

pub async fn list_companies(state: &AppState) -> Result<Vec<Company>> {
    let mut cursor = state.companies.find(doc! {}).await?;
    let mut companies = Vec::new();
    while let Some(company) = cursor.try_next().await? {
        companies.push(company);
    }
    Ok(companies)
}

pub async fn get_company_by_id(state: &AppState, id: &ObjectId) -> Result<Option<Company>> {
    state
        .companies
        .find_one(doc! { "_id": id })
        .await
        .map_err(Into::into)
}

pub async fn create_company(
    state: &AppState,
    name: &str,
    slug: &str,
    default_currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<ObjectId> {
    let slug = if slug.is_empty() {
        slugify(name)
    } else {
        slug.to_string()
    };

    let currency = if default_currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        default_currency.trim().to_string()
    };

    let res = state
        .companies
        .insert_one(Company {
            id: None,
            name: name.to_string(),
            slug,
            default_currency: currency,
            is_active,
            created_at: Some(DateTime::from_system_time(SystemTime::now())),
            updated_at: None,
            notes,
        })
        .await?;

    res.inserted_id
        .as_object_id()
        .context("company insert missing _id")
}

pub async fn update_company(
    state: &AppState,
    id: &ObjectId,
    name: &str,
    slug: &str,
    default_currency: &str,
    is_active: bool,
    notes: Option<String>,
) -> Result<()> {
    let slug = if slug.is_empty() {
        slugify(name)
    } else {
        slug.to_string()
    };

    let currency = if default_currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        default_currency.trim().to_string()
    };

    state
        .companies
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "name": name,
                "slug": slug,
                "default_currency": currency,
                "is_active": is_active,
                "notes": notes,
                "updated_at": DateTime::from_system_time(SystemTime::now())
            } },
        )
        .await?;

    Ok(())
}

pub async fn delete_company(state: &AppState, id: &ObjectId) -> Result<()> {
    let has_dependents = state
        .accounts
        .find_one(doc! { "company_id": id })
        .await?
        .is_some()
        || state
            .categories
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .contacts
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .recurring_plans
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .planned_entries
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .transactions
            .find_one(doc! { "company_id": id })
            .await?
            .is_some()
        || state
            .forecasts
            .find_one(doc! { "company_id": id })
            .await?
            .is_some();

    if has_dependents {
        state
            .companies
            .update_one(
                doc! { "_id": id },
                doc! { "$set": {
                    "is_active": false,
                    "updated_at": DateTime::from_system_time(SystemTime::now())
                } },
            )
            .await?;
        return Ok(());
    }

    state.companies.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub(super) async fn company_default_currency(state: &AppState, company_id: &ObjectId) -> Result<String> {
    let company = state
        .companies
        .find_one(doc! { "_id": company_id })
        .await?
        .context("company not found for currency fallback")?;
    Ok(if company.default_currency.trim().is_empty() {
        "MXN".to_string()
    } else {
        company.default_currency
    })
}
