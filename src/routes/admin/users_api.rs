// users_api.rs
// JSON API for company-admin user management (SPA / spcli friendly).
//
// All endpoints are admin-only and tenant-scoped: an admin may only see and
// manage users that share at least one company where the admin holds the
// Admin role. Secrets are never returned in JSON; provisioning material is
// exposed exclusively through the existing protected QR endpoint.

use std::{
    collections::HashSet,
    str::FromStr,
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::{
    models::{UserPermission, UserRole},
    session::SessionUser,
    state::{
        AppState, UserWithCompany, create_user_with_permissions, delete_user, get_user_by_id,
        list_users, update_user_with_permissions,
    },
    totp::{DEFAULT_SECRET_BYTES, generate_base32_secret_n},
};

use super::finance::helpers::require_admin_active;
use super::users::{admin_company_ids, user_shares_admin_company};

#[derive(Serialize)]
pub struct UserMembershipData {
    pub company_id: String,
    pub company_name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

#[derive(Serialize)]
pub struct UserRowData {
    pub id: String,
    pub email: String,
    pub role: String,
    pub companies: Vec<String>,
    pub memberships: Vec<UserMembershipData>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UserMembershipPayload {
    pub company_id: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UserCreatePayload {
    pub email: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub memberships: Vec<UserMembershipPayload>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UserUpdatePayload {
    pub email: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub memberships: Vec<UserMembershipPayload>,
}

fn json_error(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn role_from_str(value: &str) -> UserRole {
    match value {
        "admin" => UserRole::Admin,
        _ => UserRole::Staff,
    }
}

fn permission_from_str(value: &str) -> Option<UserPermission> {
    match value {
        "view_projects" => Some(UserPermission::ViewProjects),
        "view_project_money" => Some(UserPermission::ViewProjectMoney),
        "edit_resource_usage_today" => Some(UserPermission::EditResourceUsageToday),
        "view_resource_usage_history" => Some(UserPermission::ViewResourceUsageHistory),
        "view_timeline" => Some(UserPermission::ViewTimeline),
        _ => None,
    }
}

fn user_row_data(user: UserWithCompany) -> UserRowData {
    let memberships = user
        .company_ids
        .iter()
        .enumerate()
        .map(|(idx, cid)| UserMembershipData {
            company_id: cid.to_hex(),
            company_name: user.company_names.get(idx).cloned().unwrap_or_default(),
            role: user
                .company_roles
                .get(idx)
                .map(|r| r.as_str().to_string())
                .unwrap_or_else(|| UserRole::Staff.as_str().to_string()),
            permissions: user
                .company_permissions
                .get(idx)
                .map(|perms| perms.iter().map(|p| p.as_str().to_string()).collect())
                .unwrap_or_default(),
        })
        .collect();

    UserRowData {
        id: user.id.to_hex(),
        email: user.email,
        role: user.role.as_str().to_string(),
        companies: user.company_names.clone(),
        memberships,
    }
}

/// Parse membership payloads, keeping only companies the admin manages and
/// dropping duplicates. Returns the canonical role/permission tuples.
fn parse_memberships(
    payloads: &[UserMembershipPayload],
    allowed: &[ObjectId],
) -> Vec<(ObjectId, UserRole, Vec<UserPermission>)> {
    let allowed: HashSet<ObjectId> = allowed.iter().cloned().collect();
    let mut out: Vec<(ObjectId, UserRole, Vec<UserPermission>)> = Vec::new();
    for membership in payloads {
        let Ok(cid) = ObjectId::parse_str(membership.company_id.trim()) else {
            continue;
        };
        if !allowed.contains(&cid) || out.iter().any(|(existing, _, _)| existing == &cid) {
            continue;
        }
        let role = role_from_str(membership.role.as_deref().unwrap_or("staff"));
        let permissions = membership
            .permissions
            .iter()
            .filter_map(|p| permission_from_str(p))
            .collect();
        out.push((cid, role, permissions));
    }
    out
}

#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "admin",
    responses(
        (status = 200, description = "List of users"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden")
    ),
    security(("session" = []))
)]
pub async fn api_users_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UserRowData>>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let users = list_users(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = users
        .into_iter()
        .filter(|user| user.company_ids.iter().any(|cid| cid == &active_company))
        .map(user_row_data)
        .collect();

    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/api/admin/users/{id}",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "User detail"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn api_user_detail(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<UserRowData>, StatusCode> {
    require_admin_active(&session_user)?;
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let user = get_user_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !user_shares_admin_company(&user.company_ids, &admin_companies) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(user_row_data(user)))
}

#[utoipa::path(
    post,
    path = "/api/admin/users",
    tag = "admin",
    request_body = UserCreatePayload,
    responses(
        (status = 201, description = "User created"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 422, description = "Validation error")
    ),
    security(("session" = []))
)]
pub async fn api_users_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserCreatePayload>,
) -> impl IntoResponse {
    if require_admin_active(&session_user).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }

    let email = payload.email.trim().to_string();
    if email.is_empty() {
        return json_error(StatusCode::UNPROCESSABLE_ENTITY, "email is required");
    }

    let company_roles = parse_memberships(&payload.memberships, &admin_companies);
    if company_roles.is_empty() {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "at least one membership in a company you administer is required",
        );
    }

    // Provisioning: never accept a printed secret back from the client unless
    // it explicitly supplies one; otherwise generate a fresh secret server-side.
    // The secret is never returned in JSON — admins read the QR via
    // GET /admin/users/{id}/qrcode.
    let secret = payload
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| generate_base32_secret_n(DEFAULT_SECRET_BYTES));

    match create_user_with_permissions(&state, &email, &secret, &company_roles).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id.to_hex() })),
        )
            .into_response(),
        Err(_) => json_error(
            StatusCode::BAD_REQUEST,
            "could not create user (duplicate email?)",
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/users/{id}/update",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    request_body = UserUpdatePayload,
    responses(
        (status = 200, description = "User updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 422, description = "Validation error")
    ),
    security(("session" = []))
)]
pub async fn api_users_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UserUpdatePayload>,
) -> impl IntoResponse {
    if require_admin_active(&session_user).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let target = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if !user_shares_admin_company(&target.company_ids, &admin_companies) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let email = payload.email.trim().to_string();
    if email.is_empty() {
        return json_error(StatusCode::UNPROCESSABLE_ENTITY, "email is required");
    }

    let company_roles = parse_memberships(&payload.memberships, &admin_companies);
    if company_roles.is_empty() {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "at least one membership in a company you administer is required",
        );
    }

    // Keep the existing secret when the client does not supply a new one.
    let secret = payload
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| target.secret.clone());

    match update_user_with_permissions(&state, &object_id, &email, &secret, &company_roles).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => json_error(StatusCode::BAD_REQUEST, "could not update user"),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/users/{id}/delete",
    tag = "admin",
    params(("id" = String, Path, description = "Record id")),
    responses(
        (status = 200, description = "User deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("session" = []))
)]
pub async fn api_users_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if require_admin_active(&session_user).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if session_user.user_id() == &object_id {
        return json_error(StatusCode::FORBIDDEN, "cannot delete your own user");
    }

    let target = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty()
        || !user_shares_admin_company(&target.company_ids, &admin_companies)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    match delete_user(&state, &object_id).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
