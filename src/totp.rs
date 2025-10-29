// totp.rs
// TOTP utilities: build a TOTP instance and generate Base32 secrets.

use anyhow::Result;
use data_encoding::BASE32_NOPAD;
use rand::RngCore;
use totp_rs::{Algorithm, Secret, TOTP};

pub const MIN_SECRET_BYTES: usize = 16;      // 128 bits (mandatory minimum)
pub const DEFAULT_SECRET_BYTES: usize = 20;  // 160 bits (recommended)

/// Build a TOTP instance using user's issuer (company), account name (email), and Base32 secret.
/// Validates minimum secret length after Base32 decoding.
pub fn build_totp(issuer: &str, email: &str, base32_secret: &str) -> Result<TOTP> {
    // Decode Base32; accept NOPAD format (recommended).
    let secret = Secret::Encoded(base32_secret.to_string()).to_bytes()?;
    if secret.len() < MIN_SECRET_BYTES {
        anyhow::bail!(
            "Shared secret too short: {} bytes, need >= {} ({} bits)",
            secret.len(),
            MIN_SECRET_BYTES,
            MIN_SECRET_BYTES * 8
        );
    }
    let totp = TOTP::new(
        Algorithm::SHA1,          // compatible with Google Authenticator
        6,                        // digits
        1,                        // skew (±1 timestep to absorb small clock drift)
        30,                       // period in seconds
        secret,                   // secret bytes
        Some(issuer.to_string()), // issuer
        email.to_string(),        // account name
    )?;
    Ok(totp)
}

/// Generate a random Base32 (NOPAD) secret of `bytes` length.
pub fn generate_base32_secret_n(bytes: usize) -> String {
    let n = bytes.max(MIN_SECRET_BYTES);
    let mut buf = vec![0u8; n];
    let mut rng = rand::rng(); // rand 0.9: thread-local RNG
    rng.fill_bytes(&mut buf);
    BASE32_NOPAD.encode(&buf)
}
