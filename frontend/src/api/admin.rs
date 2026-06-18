//! Admin domain types: companies, users (+ per-company memberships), SAT fiscal
//! configs (with multipart certificate upload), and the logged-in user's own
//! account profile. CRUD goes through the generic `get_json`/`post_json`
//! helpers; the SAT upload needs a dedicated multipart call.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{File, FormData};

use super::{err_for, transport, ApiError};

// --- companies ------------------------------------------------------------

/// A company row. Mirrors the backend `CompanyData`. `is_current` marks the
/// tenant the session is currently scoped to (it can't be deleted).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompanyData {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub default_currency: String,
    pub is_active: bool,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub is_current: bool,
}

/// Create/update payload. Mirrors the backend `CompanyPayload`. An empty slug
/// is auto-derived from the name server-side.
#[derive(Clone, Debug, Serialize)]
pub struct CompanyPayload {
    pub name: String,
    pub slug: Option<String>,
    pub default_currency: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

// --- users ----------------------------------------------------------------

/// One company membership for a user, with the per-company role and the staff
/// permission grants. Mirrors the backend `UserMembershipData`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UserMembershipData {
    pub company_id: String,
    #[serde(default)]
    pub company_name: String,
    pub role: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// A user row from `GET /api/admin/users`. Mirrors the backend `UserRowData`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UserRowData {
    pub id: String,
    pub username: String,
    pub role: String,
    #[serde(default)]
    pub companies: Vec<String>,
    #[serde(default)]
    pub memberships: Vec<UserMembershipData>,
    /// Only present on the single-user detail endpoint (for copying alongside
    /// the QR); empty in the list.
    #[serde(default)]
    pub secret: String,
}

/// Membership in a create/update payload. Mirrors `UserMembershipPayload`.
#[derive(Clone, Debug, Serialize)]
pub struct UserMembershipPayload {
    pub company_id: String,
    pub role: Option<String>,
    pub permissions: Vec<String>,
}

/// Create/update payload (the backend uses the same shape for both). An omitted
/// `secret` is generated on create and kept unchanged on update.
#[derive(Clone, Debug, Serialize)]
pub struct UserPayload {
    pub username: String,
    pub secret: Option<String>,
    pub memberships: Vec<UserMembershipPayload>,
}

// --- SAT configs ----------------------------------------------------------

/// A stored SAT fiscal config. Mirrors the backend `SatConfigData`. Secrets and
/// certificate bytes are never returned — only the metadata.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SatConfigData {
    pub id: String,
    pub company_id: String,
    pub rfc: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

/// Update payload (JSON). Mirrors the backend `SatConfigPayload`. Updates keep
/// the existing certificate files; only metadata/paths/password are editable.
#[derive(Clone, Debug, Serialize)]
pub struct SatConfigPayload {
    pub rfc: String,
    pub cer_path: String,
    pub key_path: String,
    pub key_password: String,
    pub label: Option<String>,
}

/// Upload a new SAT config via multipart. Part names must match the backend:
/// `rfc`, `label`, `key_password`, `cer_file`, `key_file`. The browser sets the
/// multipart boundary automatically when the body is a `FormData`.
pub async fn upload_sat_config(
    rfc: &str,
    label: &str,
    key_password: &str,
    cer_file: &File,
    key_file: &File,
) -> Result<(), ApiError> {
    let form = FormData::new().map_err(|_| ApiError::Transport("FormData".into()))?;
    let set = |k: &str, v: &str| form.append_with_str(k, v);
    set("rfc", rfc).ok();
    set("label", label).ok();
    set("key_password", key_password).ok();
    form.append_with_blob("cer_file", cer_file)
        .map_err(|_| ApiError::Transport("cer_file".into()))?;
    form.append_with_blob("key_file", key_file)
        .map_err(|_| ApiError::Transport("key_file".into()))?;

    let resp = Request::post("/api/admin/sat-configs/upload")
        .body(form)
        .map_err(transport)?
        .send()
        .await
        .map_err(transport)?;
    err_for(resp.status()).map_or(Ok(()), Err)
}

// --- CFDI download jobs ----------------------------------------------------

/// Body for starting a CFDI download. Mirrors the backend `CfdiDownloadForm`.
/// `download_type` is `issued` | `received` | `both`.
#[derive(Clone, Debug, Serialize)]
pub struct CfdiDownloadPayload {
    pub sat_config_id: String,
    pub start: String,
    pub end: String,
    pub download_type: String,
    pub auto_create_payments: bool,
}

/// Status of one download job. The backend serializes the job status as an
/// internally-tagged enum, so `status` (`queued`/`running`/`done`/`failed`)
/// sits alongside the variant fields in the same object.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CfdiJobStatus {
    pub status: String,
    #[serde(default)]
    pub imported: u64,
    #[serde(default)]
    pub transactions_created: u64,
    #[serde(default)]
    pub transactions_updated: u64,
    #[serde(default)]
    pub transactions_skipped: u64,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// One CFDI download job (one monthly chunk). Mirrors the backend `CfdiJob`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CfdiJob {
    pub job_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub chunk_start: String,
    #[serde(default)]
    pub started_at: String,
    pub status: CfdiJobStatus,
}

// --- account (own profile) ------------------------------------------------

/// The logged-in user's own profile. Mirrors the backend `AccountData` (named
/// `Profile*` here to avoid clashing with the finance `Account`). The TOTP
/// secret is intentionally never returned.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProfileData {
    pub id: String,
    pub username: String,
}

/// Update payload for the own-account form. Mirrors the backend `AccountPayload`.
/// A blank `secret` tells the backend to keep the existing one.
#[derive(Clone, Debug, Serialize)]
pub struct ProfilePayload {
    pub username: String,
    pub secret: String,
}
