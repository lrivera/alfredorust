use anyhow::{Context, Result};
use data_encoding::BASE32_NOPAD;
use futures::stream::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use rand::RngCore;
use std::{
    time::{Duration, SystemTime},
};
use slug::slugify;

use crate::models::{Session, User, UserCompany, UserRole};

use super::{AppState, SESSION_TTL_SECONDS};

#[derive(Clone)]
pub struct UserWithCompany {
    pub id: ObjectId,
    pub email: String,
    pub secret: String,
    pub company_id: ObjectId,
    pub company_slug: String,
    pub company_ids: Vec<ObjectId>,
    pub company_slugs: Vec<String>,
    pub company_name: String,
    pub company_names: Vec<String>,
    pub company_roles: Vec<UserRole>,
    pub role: UserRole,
}

pub async fn find_user(state: &AppState, email: &str) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "email": email }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_session(state: &AppState, email: &str) -> Result<String> {
    let _ = state
        .sessions
        .delete_many(doc! { "user_email": email.to_string() })
        .await;

    let mut token_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let token = BASE32_NOPAD.encode(&token_bytes);

    let expires_at =
        DateTime::from_system_time(SystemTime::now() + Duration::from_secs(SESSION_TTL_SECONDS));

    state
        .sessions
        .insert_one(Session {
            id: None,
            token: token.clone(),
            user_email: email.to_string(),
            expires_at,
        })
        .await?;

    Ok(token)
}

pub async fn find_user_by_session(
    state: &AppState,
    token: &str,
) -> Result<Option<UserWithCompany>> {
    if let Some(session) = state.sessions.find_one(doc! { "token": token }).await? {
        let expires_at = session.expires_at.to_system_time();
        if expires_at <= SystemTime::now() {
            // Remove expired session, ignore result
            let _ = state.sessions.delete_one(doc! { "token": token }).await;
            return Ok(None);
        }
        find_user(state, &session.user_email).await
    } else {
        Ok(None)
    }
}

pub async fn list_users(state: &AppState) -> Result<Vec<UserWithCompany>> {
    let mut cursor = state.users.find(doc! {}).await?;
    let mut users = Vec::new();
    while let Some(user) = cursor.try_next().await? {
        users.push(build_user_with_company(state, user).await?);
    }
    Ok(users)
}

pub async fn get_user_by_id(state: &AppState, id: &ObjectId) -> Result<Option<UserWithCompany>> {
    if let Some(user) = state.users.find_one(doc! { "_id": id }).await? {
        build_user_with_company(state, user).await.map(Some)
    } else {
        Ok(None)
    }
}

pub async fn create_user(
    state: &AppState,
    email: &str,
    secret: &str,
    company_roles: &[(ObjectId, UserRole)],
) -> Result<ObjectId> {
    let (primary, _) = company_roles
        .first()
        .cloned()
        .context("at least one company is required for user")?;
    let company_ids: Vec<ObjectId> = company_roles.iter().map(|(id, _)| id.clone()).collect();
    let res = state
        .users
        .insert_one(User {
            id: None,
            email: email.to_string(),
            secret: secret.to_string(),
            company_id: Some(primary),
            company_ids: company_ids.clone(),
        })
        .await?;
    let uid = res
        .inserted_id
        .as_object_id()
        .context("user insert missing _id")?;

    let _ = state
        .user_companies
        .delete_many(doc! { "user_id": &uid })
        .await;
    for (cid, role) in company_roles {
        let _ = state
            .user_companies
            .insert_one(UserCompany {
                id: None,
                user_id: uid.clone(),
                company_id: cid.clone(),
                role: role.clone(),
            })
            .await;
    }

    Ok(uid)
}

pub async fn update_user(
    state: &AppState,
    id: &ObjectId,
    email: &str,
    secret: &str,
    company_roles: &[(ObjectId, UserRole)],
) -> Result<()> {
    let (primary, _) = company_roles
        .first()
        .cloned()
        .context("at least one company is required for user")?;
    let company_ids: Vec<ObjectId> = company_roles.iter().map(|(id, _)| id.clone()).collect();
    state
        .users
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {"email": email, "secret": secret, "company": primary, "companies": company_ids} },
        )
        .await?;

    let _ = state
        .user_companies
        .delete_many(doc! { "user_id": id })
        .await;
    for (cid, role) in company_roles {
        let _ = state
            .user_companies
            .insert_one(UserCompany {
                id: None,
                user_id: id.clone(),
                company_id: cid.clone(),
                role: role.clone(),
            })
            .await;
    }

    Ok(())
}

pub async fn add_user_to_company(
    state: &AppState,
    user_id: &ObjectId,
    company_id: &ObjectId,
    role: UserRole,
) -> Result<()> {
    let user = state
        .users
        .find_one(doc! { "_id": user_id })
        .await?
        .context("user not found when attaching company")?;

    let mut company_ids = user.company_ids.clone();
    if let Some(primary) = &user.company_id {
        if !company_ids.contains(primary) {
            company_ids.push(primary.clone());
        }
    }
    if !company_ids.contains(company_id) {
        company_ids.push(company_id.clone());
    }

    let primary = user
        .company_id
        .clone()
        .unwrap_or_else(|| company_id.clone());

    state
        .users
        .update_one(
            doc! { "_id": user_id },
            doc! { "$set": { "companies": &company_ids, "company": primary } },
        )
        .await?;

    let res = state
        .user_companies
        .update_one(
            doc! { "user_id": user_id, "company_id": company_id },
            doc! { "$set": { "role": role.as_str() } },
        )
        .await?;
    if res.matched_count == 0 {
        let _ = state
            .user_companies
            .insert_one(UserCompany {
                id: None,
                user_id: user_id.clone(),
                company_id: company_id.clone(),
                role,
            })
            .await?;
    }

    Ok(())
}

pub async fn delete_user(state: &AppState, id: &ObjectId) -> Result<()> {
    state.users.delete_one(doc! { "_id": id }).await?;
    let _ = state.user_companies.delete_many(doc! { "user_id": id }).await;
    Ok(())
}

pub async fn delete_session(state: &AppState, token: &str) -> Result<()> {
    let _ = state.sessions.delete_one(doc! { "token": token }).await?;
    Ok(())
}

async fn build_user_with_company(
    state: &AppState,
    user: User,
) -> Result<UserWithCompany> {
    let id = user.id.context("user missing _id")?;
    let mut memberships = Vec::new();
    let mut cursor = state.user_companies.find(doc! { "user_id": &id }).await?;
    while let Some(m) = cursor.try_next().await? {
        memberships.push(m);
    }

    // Union of embedded list and membership collection
    let mut all_company_ids: Vec<ObjectId> = user.company_ids.clone();
    for m in &memberships {
        if !all_company_ids.contains(&m.company_id) {
            all_company_ids.push(m.company_id.clone());
        }
    }
    if let Some(primary) = &user.company_id {
        if let Some(pos) = all_company_ids.iter().position(|id| id == primary) {
            all_company_ids.remove(pos);
        }
        all_company_ids.insert(0, primary.clone());
    }
    if all_company_ids.is_empty() {
        return Err(anyhow::anyhow!("user has no company assigned"));
    }
    let primary_company_id = all_company_ids[0].clone();

    let mut company_names = Vec::new();
    let mut company_slugs = Vec::new();
    let mut company_roles = Vec::new();
    for cid in &all_company_ids {
        if let Some(c) = state.companies.find_one(doc! { "_id": cid }).await? {
            company_names.push(c.name.clone());
            company_slugs.push(c.slug.clone());
        }
        let role_for_company = memberships
            .iter()
            .find(|m| &m.company_id == cid)
            .map(|m| m.role.clone())
            .unwrap_or(UserRole::Staff);
        company_roles.push(role_for_company);
    }
    let primary_company = state
        .companies
        .find_one(doc! { "_id": &primary_company_id })
        .await?
        .context("user references missing primary company")?;

    let normalized_slugs: Vec<String> = company_slugs
        .iter()
        .zip(company_names.iter())
        .map(|(slug, name)| {
            if slug.is_empty() {
                slugify(name)
            } else {
                slug.clone()
            }
        })
        .collect();
    let primary_slug = if normalized_slugs.is_empty() {
        slugify(&primary_company.name)
    } else {
        normalized_slugs[0].clone()
    };

    let effective_role = company_roles.get(0).cloned().unwrap_or(UserRole::Staff);
    Ok(UserWithCompany {
        id,
        email: user.email,
        secret: user.secret,
        company_id: primary_company_id,
        company_slug: primary_slug,
        company_ids: all_company_ids,
        company_slugs: normalized_slugs,
        company_name: primary_company.name,
        company_names,
        company_roles,
        role: effective_role,
    })
}
