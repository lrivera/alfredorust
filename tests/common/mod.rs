use std::env;

use mongodb::Client;
use uuid::Uuid;

use alfredodev::state::{AppState, init_state_with_db_name};

pub struct TestContext {
    pub state: AppState,
    pub db_name: String,
}

pub async fn setup_state() -> Option<TestContext> {
    let uri = with_server_selection_timeout(
        &env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
    );
    let db_name = format!("alfredodevtest_{}", Uuid::new_v4().simple());

    let client = match Client::with_uri_str(&uri).await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Skipping test; cannot connect to MongoDB: {err:?}");
            return None;
        }
    };
    if let Err(err) = client.database(&db_name).drop().await {
        eprintln!("Skipping test; cannot drop test DB: {err:?}");
        return None;
    }

    match init_state_with_db_name(&uri, &db_name).await {
        Ok(state) => Some(TestContext { state, db_name }),
        Err(err) => {
            eprintln!("Skipping test; init_state failed: {err:?}");
            None
        }
    }
}

fn with_server_selection_timeout(uri: &str) -> String {
    if uri.contains("serverSelectionTimeoutMS=") {
        return uri.to_string();
    }

    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}serverSelectionTimeoutMS=2000")
}

pub async fn teardown(ctx: Option<TestContext>) {
    if let Some(ctx) = ctx {
        if let Ok(uri) = env::var("MONGODB_URI") {
            if let Ok(client) = Client::with_uri_str(&uri).await {
                let _ = client.database(&ctx.db_name).drop().await;
            }
        }
        drop(ctx);
    }
}
