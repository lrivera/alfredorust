use anyhow::Result;
use bson::{doc, oid::ObjectId};
use futures::TryStreamExt;

use crate::{models::SatConfig, state::AppState};

pub async fn list_sat_configs(state: &AppState, company_id: &ObjectId) -> Result<Vec<SatConfig>> {
    let cursor = state
        .sat_configs
        .find(doc! { "company_id": company_id })
        .await?;
    Ok(cursor.try_collect().await?)
}

pub async fn create_sat_config(
    state: &AppState,
    id: ObjectId,
    company_id: ObjectId,
    rfc: String,
    cer_path: String,
    key_path: String,
    key_password: String,
    label: Option<String>,
) -> Result<ObjectId> {
    let config = SatConfig {
        id: Some(id),
        company_id,
        rfc,
        cer_path,
        key_path,
        key_password,
        label,
        created_at: bson::DateTime::now(),
    };
    state.sat_configs.insert_one(config).await?;
    Ok(id)
}

pub async fn delete_sat_config(state: &AppState, config_id: &ObjectId) -> Result<()> {
    state
        .sat_configs
        .delete_one(doc! { "_id": config_id })
        .await?;
    Ok(())
}

pub async fn get_sat_config(
    state: &AppState,
    config_id: &ObjectId,
) -> Result<Option<SatConfig>> {
    Ok(state
        .sat_configs
        .find_one(doc! { "_id": config_id })
        .await?)
}
