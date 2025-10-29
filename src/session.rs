// session.rs
// Session cookie helper and extractor to enforce authenticated routes.

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::COOKIE, request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;

use crate::state::{find_user_by_session, AppState, UserWithCompany};

pub const SESSION_COOKIE_NAME: &str = "session";

pub struct SessionUser {
    pub user: UserWithCompany,
    pub token: String,
}

fn unauthorized_response() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

#[allow(refining_impl_trait)]
impl<S> FromRequestParts<S> for SessionUser
where
    Arc<AppState>: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> BoxFuture<'static, Result<Self, Self::Rejection>> {
        let token = extract_cookie(parts, SESSION_COOKIE_NAME).map(|value| value.to_string());
        let app_state = Arc::<AppState>::from_ref(state);

        Box::pin(async move {
            let token = match token {
                Some(t) => t,
                None => return Err(unauthorized_response()),
            };

            match find_user_by_session(&app_state, &token).await {
                Ok(Some(user)) => Ok(SessionUser { user, token }),
                Ok(None) => Err(unauthorized_response()),
                Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response()),
            }
        })
    }
}

fn extract_cookie(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
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
