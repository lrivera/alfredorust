// session.rs
// Session middleware to protect routes and extractor to access session data.

use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header::COOKIE, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;

use mongodb::bson::oid::ObjectId;

use crate::state::{AppState, UserWithCompany, find_user_by_session};

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
            Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response()),
        }
    }

    if let Some((mut user, token)) = found {
            // Select active company strictly by subdomain if present
            if let Some(host) = request.headers().get("host").and_then(|h| h.to_str().ok()) {
                let host_no_port = host.split(':').next().unwrap_or(host);
                let parts: Vec<&str> = host_no_port.split('.').collect();
                let has_local_sub = parts.len() == 2 && parts[1].eq_ignore_ascii_case("localhost");
                let has_std_sub = parts.len() >= 3;
                let current_subdomain = if has_std_sub || has_local_sub {
                    Some(parts[0])
                } else {
                    None
                };

                if let Some(sub) = current_subdomain {
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
