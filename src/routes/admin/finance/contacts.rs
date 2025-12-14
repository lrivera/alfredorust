use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    session::SessionUser,
    state::{AppState, create_contact, delete_contact, get_contact_by_id, list_contacts, update_contact},
};

use super::helpers::*;

#[derive(Template)]
#[template(path = "admin/contacts/index.html")]
struct ContactsIndexTemplate {
    contacts: Vec<ContactRow>,
}

struct ContactRow {
    id: String,
    name: String,
    company: String,
    kind: String,
    email: String,
}

#[derive(Template)]
#[template(path = "admin/contacts/form.html")]
struct ContactFormTemplate {
    action: String,
    name: String,
    contact_type: String,
    email: String,
    phone: String,
    notes: String,
    companies: Vec<SimpleOption>,
    contact_options: Vec<SimpleOption>,
    is_edit: bool,
    errors: Option<String>,
}

#[derive(Deserialize)]
pub struct ContactFormData {
    name: String,
    company_id: String,
    contact_type: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

pub async fn contacts_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let contacts = list_contacts(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .collect::<Vec<_>>();
    let active_name = session_user.user().company_name.clone();

    let rows = contacts
        .into_iter()
        .filter(|c| c.company_id == active_company)
        .filter_map(|c| {
            c.id.map(|id| ContactRow {
                id: id.to_hex(),
                name: c.name,
                company: active_name.clone(),
                kind: contact_type_value(&c.contact_type).to_string(),
                email: c.email.unwrap_or_else(|| "-".into()),
            })
        })
        .collect();

    render(ContactsIndexTemplate { contacts: rows })
}

pub async fn contacts_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;
    let companies = company_options(&state, &active_company).await?;

    render(ContactFormTemplate {
        action: "/admin/contacts".into(),
        name: String::new(),
        contact_type: "customer".into(),
        email: String::new(),
        phone: String::new(),
        notes: String::new(),
        companies,
        contact_options: contact_type_options("customer"),
        is_edit: false,
        errors: None,
    })
}

pub async fn contacts_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ContactFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let companies = company_options(&state, &company_id).await.unwrap_or_default();

    let contact_type = match parse_contact_type(&form.contact_type) {
        Ok(c) => c,
        Err(msg) => {
            return render(ContactFormTemplate {
                action: "/admin/contacts".into(),
                name: form.name.clone(),
                contact_type: form.contact_type.clone(),
                email: form.email.clone().unwrap_or_default(),
                phone: form.phone.clone().unwrap_or_default(),
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                contact_options: contact_type_options(&form.contact_type),
                is_edit: false,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let email = clean_opt(form.email);
    let phone = clean_opt(form.phone);
    let notes = clean_opt(form.notes);

    match create_contact(
        &state,
        &company_id,
        form.name.trim(),
        contact_type,
        email,
        phone,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn contacts_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let active_company = require_admin_active(&session_user)?;

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let contact = get_contact_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    ensure_same_company(&contact.company_id, &active_company)?;

    let companies = company_options(&state, &active_company).await?;

    render(ContactFormTemplate {
        action: format!("/admin/contacts/{}/update", id),
        name: contact.name,
        contact_type: contact_type_value(&contact.contact_type).to_string(),
        email: contact.email.unwrap_or_default(),
        phone: contact.phone.unwrap_or_default(),
        notes: contact.notes.unwrap_or_default(),
        companies,
        contact_options: contact_type_options(contact_type_value(&contact.contact_type)),
        is_edit: true,
        errors: None,
    })
}

pub async fn contacts_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ContactFormData>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_contact_by_id(&state, &object_id).await {
        Ok(Some(contact)) => {
            if let Err(status) = ensure_same_company(&contact.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let contact_type = match parse_contact_type(&form.contact_type) {
        Ok(c) => c,
        Err(msg) => {
            let companies = company_options(&state, session_user.active_company_id())
                .await
                .unwrap_or_default();
            return render(ContactFormTemplate {
                action: format!("/admin/contacts/{}/update", id),
                name: form.name.clone(),
                contact_type: form.contact_type.clone(),
                email: form.email.clone().unwrap_or_default(),
                phone: form.phone.clone().unwrap_or_default(),
                notes: form.notes.clone().unwrap_or_default(),
                companies,
                contact_options: contact_type_options(&form.contact_type),
                is_edit: true,
                errors: Some(msg),
            })
            .map(IntoResponse::into_response)
            .unwrap_or_else(|status| status.into_response());
        }
    };

    let email = clean_opt(form.email);
    let phone = clean_opt(form.phone);
    let notes = clean_opt(form.notes);

    match update_contact(
        &state,
        &object_id,
        &company_id,
        form.name.trim(),
        contact_type,
        email,
        phone,
        notes,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn contacts_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let company_id = match require_admin_active(&session_user) {
        Ok(id) => id,
        Err(status) => return status.into_response(),
    };

    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match get_contact_by_id(&state, &object_id).await {
        Ok(Some(contact)) => {
            if let Err(status) = ensure_same_company(&contact.company_id, &company_id) {
                return status.into_response();
            }
        }
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match delete_contact(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/contacts").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
