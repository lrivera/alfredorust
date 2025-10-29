// session.rs
// Session middleware to protect routes and extractor to access session data.

use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header::COOKIE, request::Parts, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;

use crate::state::{find_user_by_session, AppState, UserWithCompany};

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
    let token = match extract_cookie(request.headers(), SESSION_COOKIE_NAME) {
        Some(value) => value.to_string(),
        None => return Err(unauthorized_response()),
    };

    match find_user_by_session(&state, &token).await {
        Ok(Some(user)) => {
            request
                .extensions_mut()
                .insert(SessionData { user, token });
            Ok(next.run(request).await)
        }
        Ok(None) => Err(unauthorized_response()),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response()),
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

    pub fn ensure_email(&self, expected: &str) -> Result<(), Response> {
        if self.user().email != expected {
            Err((StatusCode::FORBIDDEN, "mismatched session").into_response())
        } else {
            Ok(())
        }
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

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
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
        .next()
}
