// state.rs
// AppState encapsulating Mongo collections plus initialization and helpers.

use anyhow::{Context, Result};
use mongodb::{
    bson::{doc, oid::ObjectId},
    Client, Collection, Database,
};
use std::{collections::{HashMap, HashSet}, env, fs};

use crate::models::{Company, SeedUser, User};

#[derive(Clone)]
pub struct AppState {
    pub users: Collection<User>,
    pub companies: Collection<Company>,
}

pub struct UserWithCompany {
    pub email: String,
    pub secret: String,
    pub company_name: String,
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
    Ok(())
}

async fn seed_default_companies(db: &Database, names: &[String]) -> Result<HashMap<String, ObjectId>> {
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
        let company = state
            .companies
            .find_one(doc! { "_id": &user.company_id })
            .await?
            .context("user references missing company")?;
        Ok(Some(UserWithCompany {
            email: user.email,
            secret: user.secret,
            company_name: company.name,
        }))
    } else {
        Ok(None)
    }
}
