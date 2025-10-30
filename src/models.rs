// models.rs
// Domain models for both seed data (users.json) and MongoDB collections.

use mongodb::bson::{DateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

/// User roles for authorization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Staff,
}

impl UserRole {
    pub fn default_admin() -> Self {
        UserRole::Admin
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Staff => "staff",
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Staff
    }
}

/// User definition as stored in users.json (company is referenced by name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedUser {
    pub email: String,
    pub secret: String,
    pub company: String,
    #[serde(default = "UserRole::default_admin")]
    pub role: UserRole,
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
    pub role: UserRole,
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
