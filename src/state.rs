// state.rs
// AppState and helpers to load users and look them up.

use crate::models::User;
use std::{fs, env};

#[derive(Clone)]
pub struct AppState {
    pub users: Vec<User>,
}

pub fn load_users_from_env() -> Vec<User> {
    // USERS_FILE can be provided via .env or environment; defaults to "users.json"
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "./data/users.json".to_string());
    let users_json = fs::read_to_string(&users_file)
        .unwrap_or_else(|e| panic!("users.json not found ({}): {}", users_file, e));
    serde_json::from_str::<Vec<User>>(&users_json).expect("invalid users.json")
}

pub fn find_user<'a>(state: &'a AppState, email: &str) -> Option<User> {
    state.users.iter().find(|u| u.email == email).cloned()
}
