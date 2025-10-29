// models.rs
// Domain models with serde derive for JSON (users.json).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub email: String,     // Base identity (account name in TOTP)
    pub secret: String,    // Base32 (RFC 4648) TOTP shared secret (NOPAD recommended)
    pub company: String,   // Issuer shown in authenticator apps
}
