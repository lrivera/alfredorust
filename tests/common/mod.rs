use std::{
    env,
    sync::{Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use mongodb::Client;

use alfredodev::state::{init_state, AppState};

/// Global lock so integration tests that mutate the DB run one-at-a-time.
static TEST_DB_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub struct TestContext {
    pub state: AppState,
    pub db_name: String,
    _guard: MutexGuard<'static, ()>,
}

pub async fn setup_state() -> Option<TestContext> {
    let guard = TEST_DB_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("failed to lock test db mutex");

    let uri = env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let db_name = format!(
        "alfredodevtest_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    unsafe {
        env::set_var("MONGODB_DB", &db_name);
    }

    let client = match Client::with_uri_str(&uri).await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Skipping test; cannot connect to MongoDB: {err:?}");
            drop(guard);
            return None;
        }
    };
    if let Err(err) = client.database(&db_name).drop().await {
        eprintln!("Skipping test; cannot drop test DB: {err:?}");
        drop(guard);
        return None;
    }

    match init_state().await {
        Ok(state) => Some(TestContext {
            state,
            db_name,
            _guard: guard,
        }),
        Err(err) => {
            eprintln!("Skipping test; init_state failed: {err:?}");
            drop(guard);
            None
        }
    }
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
