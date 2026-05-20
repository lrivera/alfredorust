use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};

use askama::Template;
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mongodb::bson::oid::ObjectId;

#[allow(unused_imports)]
use crate::filters;

use crate::{
    models::{UserPermission, UserRole},
    session::SessionUser,
    state::{
        AppState, create_user_with_permissions, delete_user, get_user_by_id, list_companies,
        list_users, update_user_with_permissions,
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
}

struct CompanyOption {
    id: String,
    name: String,
    selected: bool,
    role: String,
    view_projects: bool,
    view_project_money: bool,
    edit_resource_usage_today: bool,
    view_resource_usage_history: bool,
    view_timeline: bool,
}

pub(crate) struct UserFormData {
    email: String,
    secret: String,
    company_ids: Vec<String>,
    role_map: std::collections::HashMap<String, String>,
    permission_map: HashMap<String, HashSet<String>>,
}

fn parse_user_form(body: &str) -> Result<UserFormData, String> {
    let mut email = String::new();
    let mut secret = String::new();
    let mut company_ids = Vec::new();
    let mut role_map = HashMap::new();
    let mut permission_map: HashMap<String, HashSet<String>> = HashMap::new();

    for (key, value) in form_urlencoded::parse(body.as_bytes()) {
        match key.as_ref() {
            "email" => email = value.into_owned(),
            "secret" => secret = value.into_owned(),
            "company_ids" => company_ids.push(value.into_owned()),
            key if key.starts_with("role_") => {
                role_map.insert(key.to_string(), value.into_owned());
            }
            key if key.starts_with("perm_") => {
                let value = value.into_owned();
                let Some(rest) = key.strip_prefix("perm_") else {
                    continue;
                };
                let Some((company_id, permission)) = rest.split_once('_') else {
                    continue;
                };
                permission_map
                    .entry(company_id.to_string())
                    .or_default()
                    .insert(permission.to_string());
                if value.is_empty() {
                    continue;
                }
            }
            _ => {}
        }
    }

    Ok(UserFormData {
        email,
        secret,
        company_ids,
        role_map,
        permission_map,
    })
}

fn admin_company_ids(session_user: &SessionUser) -> Vec<ObjectId> {
    session_user
        .user()
        .company_ids
        .iter()
        .zip(session_user.user().company_roles.iter())
        .filter(|(_, role)| role.is_admin())
        .map(|(id, _)| id.clone())
        .collect()
}

fn user_shares_admin_company(user_company_ids: &[ObjectId], admin_companies: &[ObjectId]) -> bool {
    user_company_ids
        .iter()
        .any(|cid| admin_companies.contains(cid))
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
    let active_company = session_user.active_company_id();

    let rows = users
        .into_iter()
        .filter(|user| user.company_ids.iter().any(|cid| cid == active_company))
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

    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }

    let companies = load_company_options(&state, None, admin_companies.as_slice(), None).await?;
    let form = UserFormView {
        email: String::new(),
        secret: generate_base32_secret_n(DEFAULT_SECRET_BYTES),
    };

    render(UserFormTemplate {
        form,
        companies,
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
    body: String,
) -> impl IntoResponse {
    if !session_user.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    let form = match parse_user_form(&body) {
        Ok(form) => form,
        Err(message) => return (StatusCode::UNPROCESSABLE_ENTITY, message).into_response(),
    };

    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }

    match process_user_form(form, None, &state, true, admin_companies.as_slice()).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err((form_view, companies, message)) => render(UserFormTemplate {
            form: form_view,
            companies,
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
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }

    let object_id = ObjectId::from_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let user = get_user_by_id(&state, &object_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !session_user.can_edit_user(&user.id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let is_self = session_user.user_id() == &user.id;
    if !is_self && !user_shares_admin_company(&user.company_ids, &admin_companies) {
        return Err(StatusCode::FORBIDDEN);
    }

    let selected_roles: Vec<(ObjectId, UserRole, Vec<UserPermission>)> = user
        .company_ids
        .iter()
        .zip(user.company_roles.iter())
        .zip(user.company_permissions.iter())
        .map(|((id, role), permissions)| (id.clone(), role.clone(), permissions.clone()))
        .collect();
    let companies = load_company_options(
        &state,
        Some(&selected_roles),
        admin_companies.as_slice(),
        None,
    )
    .await?;
    let form = UserFormView {
        email: user.email.clone(),
        secret: user.secret.clone(),
    };

    let can_edit_role = session_user.is_admin();

    render(UserFormTemplate {
        form,
        companies,
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
    body: String,
) -> impl IntoResponse {
    let form = match parse_user_form(&body) {
        Ok(form) => form,
        Err(message) => return (StatusCode::UNPROCESSABLE_ENTITY, message).into_response(),
    };

    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }

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

    let is_self = session_user.user_id() == &object_id;
    if !is_self && !user_shares_admin_company(&target_user.company_ids, &admin_companies) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let admin_action = session_user.is_admin();
    match process_user_form(
        form,
        Some((
            &object_id,
            target_user
                .company_ids
                .iter()
                .zip(target_user.company_roles.iter())
                .zip(target_user.company_permissions.iter())
                .map(|((id, role), permissions)| (id.clone(), role.clone(), permissions.clone()))
                .collect(),
        )),
        &state,
        admin_action,
        admin_companies.as_slice(),
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err((form_view, companies, message)) => render(UserFormTemplate {
            form: form_view,
            companies,
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

    let target_user = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let admin_companies = admin_company_ids(&session_user);
    if admin_companies.is_empty()
        || !user_shares_admin_company(&target_user.company_ids, &admin_companies)
    {
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

    let user = match get_user_by_id(&state, &object_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let is_self = session_user.user_id() == &object_id;
    if !is_self {
        let admin_companies = admin_company_ids(&session_user);
        if admin_companies.is_empty()
            || !user_shares_admin_company(&user.company_ids, &admin_companies)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

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
    existing: Option<(&ObjectId, Vec<(ObjectId, UserRole, Vec<UserPermission>)>)>,
    state: &Arc<AppState>,
    allow_role_change: bool,
    allowed_company_ids: &[ObjectId],
) -> Result<(), (UserFormView, Vec<CompanyOption>, String)> {
    let allowed: HashSet<ObjectId> = allowed_company_ids.iter().cloned().collect();
    let mut company_roles: Vec<(ObjectId, UserRole, Vec<UserPermission>)> = Vec::new();
    for raw in &form.company_ids {
        if let Ok(id) = ObjectId::from_str(raw.trim()) {
            if allowed.contains(&id) && !company_roles.iter().any(|(cid, _, _)| cid == &id) {
                let role_key = format!("role_{}", id.to_hex());
                let role_val = form
                    .role_map
                    .get(&role_key)
                    .cloned()
                    .unwrap_or_else(|| "staff".to_string());
                let role = role_from_str(&role_val);
                let permission_values = form
                    .permission_map
                    .get(&id.to_hex())
                    .cloned()
                    .unwrap_or_default();
                let permissions = permissions_from_values(&permission_values);
                company_roles.push((id, role, permissions));
            }
        }
    }

    if company_roles.is_empty() {
        let companies = load_company_options(state, None, allowed_company_ids, None)
            .await
            .unwrap_or_default();
        return Err((
            form_view_from_data(&form),
            companies,
            "Selecciona al menos una compañía válida (que tengas asignada)".into(),
        ));
    }

    if !allow_role_change {
        if let Some((_id, existing_roles)) = &existing {
            company_roles = company_roles
                .into_iter()
                .map(|(cid, _, permissions)| {
                    let role = existing_roles
                        .iter()
                        .find(|(eid, _, _)| eid == &cid)
                        .map(|(_, r, _)| r.clone())
                        .unwrap_or(UserRole::Staff);
                    (cid, role, permissions)
                })
                .collect();
        }
    }

    let email_trimmed = form.email.trim().to_string();
    let secret_trimmed = form.secret.trim().to_string();
    let form_view = UserFormView {
        email: email_trimmed.clone(),
        secret: secret_trimmed.clone(),
    };

    if email_trimmed.is_empty() || secret_trimmed.is_empty() {
        let companies = load_company_options(
            state,
            Some(&company_roles),
            allowed_company_ids,
            Some(&form.role_map),
        )
        .await
        .unwrap_or_default();
        return Err((
            form_view,
            companies,
            "Email y secreto son obligatorios".into(),
        ));
    }

    if let Some((id, _existing_roles)) = existing {
        if let Err(_) =
            update_user_with_permissions(state, id, &email_trimmed, &secret_trimmed, &company_roles)
                .await
        {
            let companies = load_company_options(
                state,
                Some(&company_roles),
                allowed_company_ids,
                Some(&form.role_map),
            )
            .await
            .unwrap_or_default();
            return Err((
                form_view,
                companies,
                "No se pudo actualizar el usuario".into(),
            ));
        }
    } else if let Err(_) =
        create_user_with_permissions(state, &email_trimmed, &secret_trimmed, &company_roles).await
    {
        let companies = load_company_options(
            state,
            Some(&company_roles),
            allowed_company_ids,
            Some(&form.role_map),
        )
        .await
        .unwrap_or_default();
        return Err((
            form_view,
            companies,
            "No se pudo crear el usuario (¿email duplicado?)".into(),
        ));
    }

    Ok(())
}

fn form_view_from_data(data: &UserFormData) -> UserFormView {
    UserFormView {
        email: data.email.clone(),
        secret: data.secret.clone(),
    }
}

fn permissions_from_values(values: &HashSet<String>) -> Vec<UserPermission> {
    let mut permissions = Vec::new();
    for value in values {
        match value.as_str() {
            "view_projects" => permissions.push(UserPermission::ViewProjects),
            "view_project_money" => permissions.push(UserPermission::ViewProjectMoney),
            "edit_resource_usage_today" => permissions.push(UserPermission::EditResourceUsageToday),
            "view_resource_usage_history" => {
                permissions.push(UserPermission::ViewResourceUsageHistory)
            }
            "view_timeline" => permissions.push(UserPermission::ViewTimeline),
            _ => {}
        }
    }
    permissions
}

async fn load_company_options(
    state: &Arc<AppState>,
    selected: Option<&[(ObjectId, UserRole, Vec<UserPermission>)]>,
    allowed: &[ObjectId],
    role_map: Option<&HashMap<String, String>>,
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
                .map(|sel_list| sel_list.iter().any(|(sel, _, _)| sel.to_hex() == id))
                .unwrap_or(false);
            let selected_role = selected
                .and_then(|sel_list| {
                    sel_list
                        .iter()
                        .find(|(sel, _, _)| sel.to_hex() == id)
                        .map(|(_, role, _)| role.as_str().to_string())
                })
                .or_else(|| role_map.and_then(|map| map.get(&format!("role_{}", id)).cloned()))
                .unwrap_or_else(|| UserRole::Staff.as_str().to_string());
            let selected_permissions = selected
                .and_then(|sel_list| {
                    sel_list
                        .iter()
                        .find(|(sel, _, _)| sel.to_hex() == id)
                        .map(|(_, _, permissions)| permissions.clone())
                })
                .unwrap_or_default();
            Some(CompanyOption {
                id,
                name: company.name,
                selected: selected_flag,
                role: selected_role,
                view_projects: selected_permissions.contains(&UserPermission::ViewProjects),
                view_project_money: selected_permissions
                    .contains(&UserPermission::ViewProjectMoney),
                edit_resource_usage_today: selected_permissions
                    .contains(&UserPermission::EditResourceUsageToday),
                view_resource_usage_history: selected_permissions
                    .contains(&UserPermission::ViewResourceUsageHistory),
                view_timeline: selected_permissions.contains(&UserPermission::ViewTimeline),
            })
        })
        .collect();

    if selected.is_none() {
        if let Some(first) = options.first_mut() {
            first.selected = true;
            if first.role.is_empty() {
                first.role = UserRole::Staff.as_str().to_string();
            }
        }
    }

    Ok(options)
}

fn role_from_str(value: &str) -> UserRole {
    match value {
        "admin" => UserRole::Admin,
        _ => UserRole::Staff,
    }
}
