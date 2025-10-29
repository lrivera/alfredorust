// models.rs
// Domain models for both seed data (users.json) and MongoDB collections.

use mongodb::bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

/// User definition as stored in users.json (company is referenced by name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedUser {
    pub email: String,
    pub secret: String,
    pub company: String,
}

/// Company document stored in MongoDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
}

/// User document stored in MongoDB referencing the company by ObjectId.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    pub secret: String,
    #[serde(rename = "company")]
    pub company_id: ObjectId,
}

/// Session document stored in MongoDB linking a token to a user and expiry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub token: String,
    pub user_email: String,
    pub expires_at: DateTime,
}
