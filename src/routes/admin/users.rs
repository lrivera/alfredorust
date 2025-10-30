use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::UserRole,
    session::SessionUser,
    state::{
        AppState, create_user, delete_user, get_user_by_id, list_companies, list_users, update_user,
    },
    totp::{DEFAULT_SECRET_BYTES, build_totp, generate_base32_secret_n},
};
use image::{DynamicImage, ImageFormat, Luma};
use qrcode::QrCode;
use std::io::Cursor;

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "admin/users/index.html")]
struct UsersIndexTemplate {
    users: Vec<UserRow>,
}

struct UserRow {
    id: String,
    email: String,
    company: String,
    role: String,
    is_self: bool,
}

#[derive(Template)]
#[template(path = "admin/users/form.html")]
struct UserFormTemplate {
    form: UserFormView,
    companies: Vec<CompanyOption>,
    roles: Vec<RoleOption>,
    action: String,
    is_edit: bool,
    can_edit_role: bool,
    user_id: Option<String>,
    errors: Option<String>,
}

#[derive(Clone)]
struct UserFormView {
    email: String,
    secret: String,
    role: String,
}

struct CompanyOption {
    id: String,
    name: String,
    selected: bool,
}

struct RoleOption {
    value: &'static str,
    label: &'static str,
    selected: bool,
}

#[derive(Deserialize)]
pub(crate) struct UserFormData {
    email: String,
    secret: String,
    company_id: String,
    role: String,
}

pub async fn users_index(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    let users = list_users(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_id = session_user.user_id().clone();

    let rows = users
        .into_iter()
        .map(|user| UserRow {
            id: user.id.to_hex(),
            email: user.email,
            company: user.company_name,
            role: user.role.as_str().to_string(),
            is_self: current_id == user.id,
        })
        .collect();

    render(UsersIndexTemplate { users: rows })
}

pub async fn users_new(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if !session_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    let companies = load_company_options(&state, None).await?;
    let form = UserFormView {
        email: String::new(),
        secret: generate_base32_secret_n(DEFAULT_SECRET_BYTES),
        role: UserRole::Staff.as_str().to_string(),
    };

    render(UserFormTemplate {
        form,
        companies,
        roles: role_options(UserRole::Staff),
        action: "/admin/users".into(),
        is_edit: false,
        user_id: None,
        can_edit_role: true,
        errors: None,
    })
}

pub async fn users_create(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Form(form): Form<UserFormData>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    match process_user_form(form, None, &state, true).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err((form_view, companies, message, selected_role)) => render(UserFormTemplate {
            form: form_view,
            companies,
            roles: role_options(selected_role),
            action: "/admin/users".into(),
            is_edit: false,
            user_id: None,
            can_edit_role: true,
            errors: Some(message),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response()),
    }
}

pub async fn users_edit(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let user = get_user_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !session_user.can_edit_user(&user.id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let companies = load_company_options(&state, Some(&user.company_id)).await?;
    let form = UserFormView {
        email: user.email.clone(),
        secret: user.secret.clone(),
        role: user.role.as_str().to_string(),
    };

    let can_edit_role = session_user.is_admin();

    render(UserFormTemplate {
        form,
        companies,
        roles: role_options(user.role.clone()),
        action: format!("/admin/users/{}/update", id),
        is_edit: true,
        user_id: Some(user.id.to_hex()),
        can_edit_role,
        errors: None,
    })
}

pub async fn users_update(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<UserFormData>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let target_user = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let can_edit_role = session_user.is_admin();
    if !session_user.can_edit_user(&object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let admin_action = session_user.is_admin();
    match process_user_form(
        form,
        Some((&object_id, target_user.role.clone())),
        &state,
        admin_action,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err((form_view, companies, message, selected_role)) => render(UserFormTemplate {
            form: form_view,
            companies,
            roles: role_options(selected_role),
            action: format!("/admin/users/{}/update", id),
            is_edit: true,
            user_id: Some(id),
            can_edit_role,
            errors: Some(message),
        })
        .map(IntoResponse::into_response)
        .unwrap_or_else(|status| status.into_response()),
    }
}

pub async fn users_delete(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if session_user.user_id() == &object_id {
        return StatusCode::FORBIDDEN.into_response();
    }

    match delete_user(&state, &object_id).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn users_qrcode(
    session_user: SessionUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let object_id = match ObjectId::from_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if !session_user.can_edit_user(&object_id) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let user = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let totp = match build_totp(&user.company_name, &user.email, &user.secret) {
        Ok(totp) => totp,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let url = totp.get_url();
    let code = match QrCode::new(url.as_bytes()) {
        Ok(code) => code,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let img = code.render::<Luma<u8>>().min_dimensions(400, 400).build();
    let mut cursor = Cursor::new(Vec::<u8>::new());
    if DynamicImage::ImageLuma8(img)
        .write_to(&mut cursor, ImageFormat::Png)
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let png = cursor.into_inner();
    Response::builder()
        .header("Content-Type", "image/png")
        .body(Body::from(png))
        .unwrap()
}

async fn process_user_form(
    form: UserFormData,
    existing: Option<(&ObjectId, UserRole)>,
    state: &Arc<AppState>,
    allow_role_change: bool,
) -> Result<(), (UserFormView, Vec<CompanyOption>, String, UserRole)> {
    let company_id = match ObjectId::from_str(form.company_id.trim()) {
        Ok(id) => id,
        Err(_) => {
            let companies = load_company_options(state, None).await.unwrap_or_default();
            return Err((
                form_view_from_data(&form),
                companies,
                "Compañía inválida".into(),
                role_from_str(&form.role),
            ));
        }
    };

    let requested_role = role_from_str(&form.role);
    let role = if allow_role_change {
        requested_role
    } else {
        existing
            .as_ref()
            .map(|(_, role)| role.clone())
            .unwrap_or(requested_role)
    };

    let email_trimmed = form.email.trim().to_string();
    let secret_trimmed = form.secret.trim().to_string();
    let form_view = UserFormView {
        email: email_trimmed.clone(),
        secret: secret_trimmed.clone(),
        role: role.as_str().to_string(),
    };

    if email_trimmed.is_empty() || secret_trimmed.is_empty() {
        let companies = load_company_options(state, Some(&company_id))
            .await
            .unwrap_or_default();
        return Err((
            form_view,
            companies,
            "Email y secreto son obligatorios".into(),
            role,
        ));
    }

    if let Some((id, _existing_role)) = existing {
        if let Err(_) = update_user(
            state,
            id,
            &email_trimmed,
            &secret_trimmed,
            &company_id,
            role.clone(),
        )
        .await
        {
            let companies = load_company_options(state, Some(&company_id))
                .await
                .unwrap_or_default();
            return Err((
                form_view,
                companies,
                "No se pudo actualizar el usuario".into(),
                role,
            ));
        }
    } else {
        if let Err(_) = create_user(
            state,
            &email_trimmed,
            &secret_trimmed,
            &company_id,
            role.clone(),
        )
        .await
        {
            let companies = load_company_options(state, Some(&company_id))
                .await
                .unwrap_or_default();
            return Err((
                form_view,
                companies,
                "No se pudo crear el usuario (¿email duplicado?)".into(),
                role,
            ));
        }
    }

    Ok(())
}

fn form_view_from_data(data: &UserFormData) -> UserFormView {
    UserFormView {
        email: data.email.clone(),
        secret: data.secret.clone(),
        role: data.role.clone(),
    }
}

async fn load_company_options(
    state: &Arc<AppState>,
    selected: Option<&ObjectId>,
) -> Result<Vec<CompanyOption>, StatusCode> {
    let companies = list_companies(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options: Vec<CompanyOption> = companies
        .into_iter()
        .filter_map(|company| {
            let id = company.id?.to_hex();
            let selected_flag = selected.map(|sel| sel.to_hex() == id).unwrap_or(false);
            Some(CompanyOption {
                id,
                name: company.name,
                selected: selected_flag,
            })
        })
        .collect();

    if selected.is_none() {
        if let Some(first) = options.first_mut() {
            first.selected = true;
        }
    }

    Ok(options)
}

fn role_options(selected: UserRole) -> Vec<RoleOption> {
    vec![
        RoleOption {
            value: "admin",
            label: "Administrador",
            selected: matches!(selected, UserRole::Admin),
        },
        RoleOption {
            value: "staff",
            label: "Staff",
            selected: matches!(selected, UserRole::Staff),
        },
    ]
}

fn role_from_str(value: &str) -> UserRole {
    match value {
        "admin" => UserRole::Admin,
        _ => UserRole::Staff,
    }
}
