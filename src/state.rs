// state.rs
// AppState encapsulating Mongo collections plus initialization and helpers.

use anyhow::{Context, Result};
use data_encoding::BASE32_NOPAD;
use futures::stream::TryStreamExt;
use mongodb::{
    Client, Collection, Database,
    bson::{DateTime, doc, oid::ObjectId},
};
use rand::RngCore;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    time::{Duration, SystemTime},
};

use crate::models::{Company, SeedUser, Session, User, UserRole};

pub const SESSION_TTL_SECONDS: u64 = 60 * 30; // 30 minutes

#[derive(Clone)]
pub struct AppState {
    pub users: Collection<User>,
    pub companies: Collection<Company>,
    pub sessions: Collection<Session>,
}

#[derive(Clone)]
pub struct UserWithCompany {
    pub id: ObjectId,
    pub email: String,
    pub secret: String,
    pub company_id: ObjectId,
    pub company_name: String,
    pub role: UserRole,
}

pub async fn init_state() -> Result<AppState> {
    let uri = env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let db_name = env::var("MONGODB_DB").unwrap_or_else(|_| "totp".to_string());

    let client = Client::with_uri_str(uri).await?;
    let db = client.database(&db_name);

    ensure_collections(&db).await?;

    let default_users = load_default_users()?;
    let company_names = derive_company_names(&default_users);
    let company_ids = seed_default_companies(&db, &company_names).await?;
    seed_default_users(&db, &default_users, &company_ids).await?;

    Ok(AppState {
        users: db.collection::<User>("users"),
        companies: db.collection::<Company>("company"),
        sessions: db.collection::<Session>("sessions"),
    })
}

fn load_default_users() -> Result<Vec<SeedUser>> {
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "./data/users.json".to_string());
    let users_json = fs::read_to_string(&users_file)?;
    let users = serde_json::from_str::<Vec<SeedUser>>(&users_json)?;
    Ok(users)
}

fn derive_company_names(users: &[SeedUser]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut companies = Vec::new();
    for user in users {
        if seen.insert(user.company.clone()) {
            companies.push(user.company.clone());
        }
    }
    companies
}

async fn ensure_collections(db: &Database) -> Result<()> {
    let existing = db.list_collection_names().await?;
    if !existing.iter().any(|name| name == "users") {
        db.create_collection("users").await?;
    }
    if !existing.iter().any(|name| name == "company") {
        db.create_collection("company").await?;
    }
    if !existing.iter().any(|name| name == "sessions") {
        db.create_collection("sessions").await?;
    }
    Ok(())
}

async fn seed_default_companies(
    db: &Database,
    names: &[String],
) -> Result<HashMap<String, ObjectId>> {
    let coll = db.collection::<Company>("company");
    let mut map = HashMap::new();

    for name in names {
        if let Some(existing) = coll.find_one(doc! { "name": name.clone() }).await? {
            if let Some(id) = existing.id {
                map.insert(name.clone(), id);
                continue;
            }
        }

        let inserted = coll
            .insert_one(Company {
                id: None,
                name: name.clone(),
            })
            .await?;
        let id = inserted
            .inserted_id
            .as_object_id()
            .context("company insert did not return an ObjectId")?;
        map.insert(name.clone(), id);
    }

    Ok(map)
}

async fn seed_default_users(
    db: &Database,
    defaults: &[SeedUser],
    company_ids: &HashMap<String, ObjectId>,
) -> Result<()> {
    let coll = db.collection::<mongodb::bson::Document>("users");
    for user in defaults {
        let company_id = company_ids
            .get(&user.company)
            .cloned()
            .context(format!("missing company id for {}", user.company))?;

        coll.update_one(
            doc! { "email": &user.email },
            doc! {
                "$set": {
                    "secret": &user.secret,
                    "company": company_id,
                    "role": user.role.as_str(),
                },
                "$setOnInsert": {
                    "email": &user.email,
                }
            },
        )
        .upsert(true)
        .await?;
    }
    Ok(())
}

pub async fn find_user(state: &AppState, email: &str) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "email": email }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_session(state: &AppState, email: &str) -> Result<String> {
    // Optionally drop previous sessions for this user (best-effort)
    let _ = state
        .sessions
        .delete_many(doc! { "user_email": email.to_string() })
        .await;

    let mut token_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let token = BASE32_NOPAD.encode(&token_bytes);

    let expires_at =
        DateTime::from_system_time(SystemTime::now() + Duration::from_secs(SESSION_TTL_SECONDS));

    state
        .sessions
        .insert_one(Session {
            id: None,
            token: token.clone(),
            user_email: email.to_string(),
            expires_at,
        })
        .await?;

    Ok(token)
}

pub async fn find_user_by_session(
    state: &AppState,
    token: &str,
) -> Result<Option<UserWithCompany>> {
    if let Some(session) = state.sessions.find_one(doc! { "token": token }).await? {
        let expires_at = session.expires_at.to_system_time();
        if expires_at <= SystemTime::now() {
            // Remove expired session, ignore result
            if let Some(id) = session.id {
                let _ = state.sessions.delete_one(doc! { "_id": id }).await;
            } else {
                let _ = state
                    .sessions
                    .delete_one(doc! { "token": &session.token })
                    .await;
            }
            return Ok(None);
        }
        find_user(state, &session.user_email).await
    } else {
        Ok(None)
    }
}

pub async fn list_users(state: &AppState) -> Result<Vec<UserWithCompany>> {
    let mut cursor = state.users.find(doc! {}).await?;
    let mut users = Vec::new();
    while let Some(user) = cursor.try_next().await? {
        users.push(build_user_with_company(state, user).await?);
    }
    Ok(users)
}

pub async fn get_user_by_id(state: &AppState, id: &ObjectId) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "_id": id }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_user(
    state: &AppState,
    email: &str,
    secret: &str,
    company_id: &ObjectId,
    role: UserRole,
) -> Result<ObjectId> {
    let res = state
        .users
        .insert_one(User {
            id: None,
            email: email.to_string(),
            secret: secret.to_string(),
            company_id: company_id.clone(),
            role,
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("user insert missing _id")
}

pub async fn update_user(
    state: &AppState,
    id: &ObjectId,
    email: &str,
    secret: &str,
    company_id: &ObjectId,
    role: UserRole,
) -> Result<()> {
    state
        .users
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {"email": email, "secret": secret, "company": company_id, "role": role.as_str()} },
        )
        .await?;
    Ok(())
}

pub async fn delete_user(state: &AppState, id: &ObjectId) -> Result<()> {
    state.users.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

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

pub async fn create_company(state: &AppState, name: &str) -> Result<ObjectId> {
    let res = state
        .companies
        .insert_one(Company {
            id: None,
            name: name.to_string(),
        })
        .await?;
    res.inserted_id
        .as_object_id()
        .context("company insert missing _id")
}

pub async fn update_company(state: &AppState, id: &ObjectId, name: &str) -> Result<()> {
    state
        .companies
        .update_one(doc! { "_id": id }, doc! { "$set": {"name": name} })
        .await?;
    Ok(())
}

pub async fn delete_company(state: &AppState, id: &ObjectId) -> Result<()> {
    state.companies.delete_one(doc! { "_id": id }).await?;
    Ok(())
}

async fn build_user_with_company(state: &AppState, user: User) -> Result<UserWithCompany> {
    let id = user.id.context("user missing _id")?;
    let company = state
        .companies
        .find_one(doc! { "_id": &user.company_id })
        .await?
        .context("user references missing company")?;
    Ok(UserWithCompany {
        id,
        email: user.email,
        secret: user.secret,
        company_id: user.company_id,
        company_name: company.name,
        role: user.role,
    })
}

pub async fn delete_session(state: &AppState, token: &str) -> Result<()> {
    let _ = state.sessions.delete_one(doc! { "token": token }).await?;
    Ok(())
}
