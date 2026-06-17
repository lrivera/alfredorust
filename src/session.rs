// session.rs
// Session middleware to protect routes and extractor to access session data.

use std::{env, sync::Arc};

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header::COOKIE, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;

use mongodb::bson::oid::ObjectId;

use crate::{
    models::UserPermission,
    state::{AppState, UserWithCompany, find_user_by_session},
};

pub const SESSION_COOKIE_NAME: &str = "session";

#[derive(Clone)]
pub struct SessionData {
    pub user: UserWithCompany,
    pub token: String,
}

pub async fn require_session(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    let tokens = extract_cookies(request.headers(), SESSION_COOKIE_NAME);
    if tokens.is_empty() {
        return Err(unauthorized_response());
    }

    // Try all cookies with the session name until one is valid
    let mut found = None;
    for token in tokens {
        match find_user_by_session(&state, &token).await {
            Ok(Some(user)) => {
                found = Some((user, token));
                break;
            }
            Ok(None) => continue,
            Err(_) => {
                return Err(
                    (StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response()
                );
            }
        }
    }

    if let Some((mut user, token)) = found {
        // Select active company strictly by a trusted tenant subdomain if present.
        if let Some(host) = request.headers().get("host").and_then(|h| h.to_str().ok()) {
            if let Some(sub) = tenant_subdomain_from_host(host) {
                if let Some(idx) = user
                    .company_slugs
                    .iter()
                    .position(|s| s.eq_ignore_ascii_case(sub))
                {
                    user.company_id = user.company_ids[idx].clone();
                    user.company_slug = user.company_slugs[idx].clone();
                    user.company_name = user.company_names[idx].clone();
                    if let Some(role) = user.company_roles.get(idx) {
                        user.role = role.clone();
                    }
                    user.permissions = user
                        .company_permissions
                        .get(idx)
                        .cloned()
                        .unwrap_or_default();
                } else {
                    // Subdominio no corresponde a ninguna compañía del usuario
                    return Err(unauthorized_response());
                }
            }
        }

        request.extensions_mut().insert(SessionData { user, token });
        Ok(next.run(request).await)
    } else {
        Err(unauthorized_response())
    }
}

pub fn tenant_subdomain_from_host(host: &str) -> Option<&str> {
    let host_no_port = host.split(':').next().unwrap_or(host).trim_end_matches('.');
    if host_no_port.is_empty() || host_no_port.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }

    if let Ok(base_domain) = env::var("BASE_DOMAIN") {
        let base = base_domain
            .trim()
            .trim_start_matches('.')
            .trim_end_matches('.');
        if base.is_empty() || host_no_port.eq_ignore_ascii_case(base) {
            return None;
        }
        let suffix = format!(".{base}");
        return host_no_port.strip_suffix(&suffix).filter(|sub| {
            !sub.is_empty() && !sub.contains('.') && !sub.eq_ignore_ascii_case("app")
        });
    }

    let parts: Vec<&str> = host_no_port.split('.').collect();
    if parts.len() == 2
        && parts[1].eq_ignore_ascii_case("localhost")
        && !parts[0].eq_ignore_ascii_case("app")
    {
        return Some(parts[0]);
    }
    if parts.len() == 3
        && parts[2].eq_ignore_ascii_case("local")
        && !parts[0].eq_ignore_ascii_case("app")
    {
        return Some(parts[0]);
    }
    None
}

pub struct SessionUser(pub SessionData);

impl SessionUser {
    pub fn user(&self) -> &UserWithCompany {
        &self.0.user
    }

    pub fn token(&self) -> &str {
        &self.0.token
    }

    pub fn user_id(&self) -> &ObjectId {
        &self.0.user.id
    }

    pub fn is_admin(&self) -> bool {
        self.0.user.role.is_admin()
    }

    pub fn active_role(&self) -> &crate::models::UserRole {
        &self.0.user.role
    }

    pub fn has_permission(&self, permission: UserPermission) -> bool {
        self.is_admin() || self.0.user.permissions.contains(&permission)
    }

    pub fn active_company_id(&self) -> &ObjectId {
        &self.0.user.company_id
    }

    pub fn active_company_slug(&self) -> &str {
        &self.0.user.company_slug
    }

    pub fn can_edit_user(&self, target: &ObjectId) -> bool {
        self.is_admin() || self.user_id() == target
    }
}

#[allow(refining_impl_trait)]
impl<S> FromRequestParts<S> for SessionUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> BoxFuture<'static, Result<Self, Self::Rejection>> {
        let data = parts
            .extensions
            .get::<SessionData>()
            .cloned()
            .ok_or_else(unauthorized_response);

        Box::pin(async move {
            match data {
                Ok(session) => Ok(SessionUser(session)),
                Err(resp) => Err(resp),
            }
        })
    }
}

fn unauthorized_response() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

fn extract_cookies(headers: &HeaderMap, name: &str) -> Vec<String> {
    headers
        .get_all(COOKIE)
        .into_iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| {
            let mut split = pair.trim().splitn(2, '=');
            let key = split.next()?.trim();
            let value = split.next()?.trim();
            if key == name {
                Some(value.to_owned())
            } else {
                None
            }
        })
        .collect()
}

/// Process-wide lock for tests that mutate global environment variables
/// (notably `BASE_DOMAIN`). Tests live in several modules but touch the same
/// process env, so they must all serialize on this single mutex — per-module
/// mutexes do not prevent the cross-module data race. Poisoning is recovered
/// from so one panicking test does not cascade failures into the rest.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::tenant_subdomain_from_host;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        super::test_env_lock()
    }

    #[test]
    fn tenant_subdomain_requires_trusted_base_domain() {
        let _guard = env_lock();
        unsafe {
            std::env::set_var("BASE_DOMAIN", "alfredorivera.dev");
        }

        assert_eq!(
            tenant_subdomain_from_host("acme.alfredorivera.dev"),
            Some("acme")
        );
        assert_eq!(
            tenant_subdomain_from_host("acme.alfredorivera.dev:8090"),
            Some("acme")
        );
        assert_eq!(tenant_subdomain_from_host("acme.evil.test"), None);
        assert_eq!(tenant_subdomain_from_host("app.alfredorivera.dev"), None);
        assert_eq!(
            tenant_subdomain_from_host("nested.acme.alfredorivera.dev"),
            None
        );

        unsafe {
            std::env::remove_var("BASE_DOMAIN");
        }
    }

    #[test]
    fn tenant_subdomain_allows_local_development_hosts_only_without_base_domain() {
        let _guard = env_lock();
        unsafe {
            std::env::remove_var("BASE_DOMAIN");
        }

        assert_eq!(
            tenant_subdomain_from_host("acme.localhost:8090"),
            Some("acme")
        );
        assert_eq!(
            tenant_subdomain_from_host("acme.miapp.local:8090"),
            Some("acme")
        );
        assert_eq!(tenant_subdomain_from_host("app.localhost:8090"), None);
        assert_eq!(tenant_subdomain_from_host("app.miapp.local:8090"), None);
        assert_eq!(tenant_subdomain_from_host("acme.evil.test"), None);
        assert_eq!(tenant_subdomain_from_host("127.0.0.1:8090"), None);
    }
}
