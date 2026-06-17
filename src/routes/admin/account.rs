use std::sync::Arc;

use askama::Template;
use axum::{
    Json,
    extract::{Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};

use crate::{
    session::SessionUser,
    state::{AppState, update_user},
};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "account/edit.html")]
struct AccountTemplate {
    form: AccountFormView,
    message: Option<String>,
    errors: Option<String>,
}

#[derive(Clone)]
struct AccountFormView {
    email: String,
    secret: String,
}

#[derive(Deserialize)]
pub(crate) struct AccountFormData {
    email: String,
    secret: String,
}

#[derive(Serialize)]
pub struct AccountData {
    id: String,
    email: String,
}

#[derive(Deserialize)]
pub struct AccountPayload {
    email: String,
    secret: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct AccountQuery {
    saved: Option<bool>,
}

pub async fn account_edit(
    SessionUser(session): SessionUser,
    Query(query): Query<AccountQuery>,
) -> Result<Html<String>, StatusCode> {
    let message = if query.saved.unwrap_or(false) {
        Some("Tu información se guardó correctamente".to_string())
    } else {
        None
    };

    let form = AccountFormView {
        email: session.user.email.clone(),
        secret: session.user.secret.clone(),
    };

    render(AccountTemplate {
        form,
        message,
        errors: None,
    })
}

pub async fn account_profile_data_api(SessionUser(session): SessionUser) -> Json<AccountData> {
    Json(AccountData {
        id: session.user.id.to_hex(),
        email: session.user.email,
    })
}

pub async fn account_profile_update_api(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AccountPayload>,
) -> impl IntoResponse {
    let email = payload.email.trim().to_string();
    let secret = payload.secret.trim().to_string();
    if email.is_empty() || secret.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let user = session_user.user();
    let company_roles: Vec<_> = user
        .company_ids
        .iter()
        .zip(user.company_roles.iter())
        .map(|(id, role)| (id.clone(), role.clone()))
        .collect();
    match update_user(
        &state,
        session_user.user_id(),
        &email,
        &secret,
        &company_roles,
    )
    .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn account_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<AccountFormData>,
) -> impl IntoResponse {
    let email = form.email.trim().to_string();
    let secret = form.secret.trim().to_string();

    let form_view = AccountFormView {
        email: email.clone(),
        secret: secret.clone(),
    };

    if email.is_empty() || secret.is_empty() {
        return render(AccountTemplate {
            form: form_view,
            message: None,
            errors: Some("Email y secreto son obligatorios".into()),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response());
    }

    let user = session_user.user();
    let company_roles: Vec<_> = user
        .company_ids
        .iter()
        .zip(user.company_roles.iter())
        .map(|(id, role)| (id.clone(), role.clone()))
        .collect();
    let update_result = update_user(
        &state,
        session_user.user_id(),
        &email,
        &secret,
        &company_roles,
    )
    .await;

    match update_result {
        Ok(_) => Redirect::to("/account?saved=1").into_response(),
        Err(_) => render(AccountTemplate {
            form: form_view,
            message: None,
            errors: Some("No se pudo guardar la información".into()),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response()),
    }
}
