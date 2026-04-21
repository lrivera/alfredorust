/// List all SAT configs with their IDs and company IDs.
/// Usage: cargo run --bin list_sat_configs

use std::sync::Arc;
use alfredodev::state::init_state;
use dotenvy::dotenv;
use futures::stream::TryStreamExt;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let state = Arc::new(init_state().await.expect("failed to init state"));

    let mut cursor = state.sat_configs.find(bson::doc! {}).await.unwrap();
    while let Some(cfg) = cursor.try_next().await.unwrap() {
        println!(
            "company_id={} sat_config_id={} rfc={} label={}",
            cfg.company_id,
            cfg.id.map(|i| i.to_hex()).unwrap_or_default(),
            cfg.rfc,
            cfg.label.unwrap_or_default(),
        );
    }
}
