use std::{collections::HashSet, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mongodb::bson::oid::ObjectId;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

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

pub(crate) struct UserFormData {
    email: String,
    secret: String,
    company_ids: Vec<String>,
    role: String,
}

impl<'de> Deserialize<'de> for UserFormData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Email,
            Secret,
            CompanyIds,
            Role,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;
                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`email`, `secret`, `company_ids`, or `role`")
                    }
                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        match v {
                            "email" => Ok(Field::Email),
                            "secret" => Ok(Field::Secret),
                            "company_ids" => Ok(Field::CompanyIds),
                            "role" => Ok(Field::Role),
                            other => Err(de::Error::unknown_field(
                                other,
                                &["email", "secret", "company_ids", "role"],
                            )),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct FormVisitor;
        impl<'de> Visitor<'de> for FormVisitor {
            type Value = UserFormData;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("user form data")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut email: Option<String> = None;
                let mut secret: Option<String> = None;
                let mut company_ids: Vec<String> = Vec::new();
                let mut role: Option<String> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Email => {
                            if email.is_some() {
                                return Err(de::Error::duplicate_field("email"));
                            }
                            email = Some(map.next_value()?);
                        }
                        Field::Secret => {
                            if secret.is_some() {
                                return Err(de::Error::duplicate_field("secret"));
                            }
                            secret = Some(map.next_value()?);
                        }
                        Field::CompanyIds => {
                            let part: OneOrManyStrings = map.next_value()?;
                            company_ids.extend(part.0);
                        }
                        Field::Role => {
                            if role.is_some() {
                                return Err(de::Error::duplicate_field("role"));
                            }
                            role = Some(map.next_value()?);
                        }
                    }
                }

                let email = email.ok_or_else(|| de::Error::missing_field("email"))?;
                let secret = secret.ok_or_else(|| de::Error::missing_field("secret"))?;
                let role = role.ok_or_else(|| de::Error::missing_field("role"))?;

                Ok(UserFormData {
                    email,
                    secret,
                    company_ids,
                    role,
                })
            }
        }

        deserializer.deserialize_map(FormVisitor)
    }
}

/// Helper that accepts either a single string or a sequence of strings.
struct OneOrManyStrings(Vec<String>);

impl<'de> Deserialize<'de> for OneOrManyStrings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OneOrManyVisitor;
        impl<'de> Visitor<'de> for OneOrManyVisitor {
            type Value = OneOrManyStrings;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or list of strings")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OneOrManyStrings(vec![v.to_string()]))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OneOrManyStrings(vec![v]))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(v) = seq.next_element::<String>()? {
                    values.push(v);
                }
                Ok(OneOrManyStrings(values))
            }
        }

        deserializer.deserialize_any(OneOrManyVisitor)
    }
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
        .map(|user| {
            let company_label = if user.company_names.is_empty() {
                user.company_name.clone()
            } else {
                user.company_names.join(", ")
            };
            UserRow {
                id: user.id.to_hex(),
                email: user.email,
                company: company_label,
                role: user.role.as_str().to_string(),
                is_self: current_id == user.id,
            }
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

    let companies =
        load_company_options(&state, None, session_user.user().company_ids.as_slice()).await?;
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

    match process_user_form(
        form,
        None,
        &state,
        true,
        session_user.user().company_ids.as_slice(),
    )
    .await
    {
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

    let companies = load_company_options(
        &state,
        Some(&user.company_ids),
        session_user.user().company_ids.as_slice(),
    )
    .await?;
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
        session_user.user().company_ids.as_slice(),
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
    allowed_company_ids: &[ObjectId],
) -> Result<(), (UserFormView, Vec<CompanyOption>, String, UserRole)> {
    let allowed: HashSet<ObjectId> = allowed_company_ids.iter().cloned().collect();
    let company_ids: Vec<ObjectId> = {
        let mut ids = Vec::new();
        for raw in &form.company_ids {
            if let Ok(id) = ObjectId::from_str(raw.trim()) {
                if allowed.contains(&id) && !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        if ids.is_empty() {
            let companies = load_company_options(state, None, allowed_company_ids)
                .await
                .unwrap_or_default();
            return Err((
                form_view_from_data(&form),
                companies,
                "Selecciona al menos una compañía válida (que tengas asignada)".into(),
                role_from_str(&form.role),
            ));
        }
        ids
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
        let companies = load_company_options(state, Some(&company_ids), allowed_company_ids)
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
            &company_ids,
            role.clone(),
        )
        .await
        {
            let companies = load_company_options(state, Some(&company_ids), allowed_company_ids)
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
            &company_ids,
            role.clone(),
        )
        .await
        {
            let companies = load_company_options(state, Some(&company_ids), allowed_company_ids)
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
    selected: Option<&[ObjectId]>,
    allowed: &[ObjectId],
) -> Result<Vec<CompanyOption>, StatusCode> {
    let allowed_set: HashSet<ObjectId> = allowed.iter().cloned().collect();
    let companies = list_companies(state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut options: Vec<CompanyOption> = companies
        .into_iter()
        .filter_map(|company| {
            let cid = company.id?;
            if !allowed_set.contains(&cid) {
                return None;
            }
            let id = cid.to_hex();
            let selected_flag = selected
                .map(|sel_list| sel_list.iter().any(|sel| sel.to_hex() == id))
                .unwrap_or(false);
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
