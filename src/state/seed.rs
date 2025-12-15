use anyhow::{Context, Result};
use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection, Database,
};
use serde::de::DeserializeOwned;
use slug::slugify;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
};

use crate::models::{
    Account, Category, Company, Contact, Forecast, PlannedEntry, RecurringPlan, SeedUser, Transaction, User,
    UserCompany,
};

pub(super) async fn is_database_empty(db: &Database) -> Result<bool> {
    let users_coll = db.collection::<User>("users");
    let count = users_coll.estimated_document_count().await?;
    Ok(count == 0)
}

pub(super) fn load_default_users() -> Result<Vec<SeedUser>> {
    let users_file = env::var("USERS_FILE").unwrap_or_else(|_| "./data/users.json".to_string());
    let users_json = fs::read_to_string(&users_file)?;
    let users = serde_json::from_str::<Vec<SeedUser>>(&users_json)?;
    Ok(users)
}

pub(super) fn load_json_array<T: DeserializeOwned>(env_key: &str, default_path: &str) -> Result<Vec<T>> {
    let path = env::var(env_key).unwrap_or_else(|_| default_path.to_string());
    if let Ok(contents) = fs::read_to_string(&path) {
        let parsed = serde_json::from_str::<Vec<T>>(&contents)?;
        Ok(parsed)
    } else {
        Ok(Vec::new())
    }
}

pub(super) fn derive_company_names(users: &[SeedUser]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut companies = Vec::new();
    for user in users {
        let mut push_name = |name: &String| {
            if seen.insert(name.clone()) {
                companies.push(name.clone());
            }
        };
        push_name(&user.company);
        for extra in &user.companies {
            push_name(extra);
        }
    }
    companies
}

pub(super) async fn ensure_collections(db: &Database) -> Result<()> {
    let existing = db.list_collection_names().await?;
    if !existing.iter().any(|name| name == "users") {
        db.create_collection("users").await?;
    }
    if !existing.iter().any(|name| name == "user_companies") {
        db.create_collection("user_companies").await?;
    }
    if !existing.iter().any(|name| name == "company") {
        db.create_collection("company").await?;
    }
    if !existing.iter().any(|name| name == "sessions") {
        db.create_collection("sessions").await?;
    }
    if !existing.iter().any(|name| name == "accounts") {
        db.create_collection("accounts").await?;
    }
    if !existing.iter().any(|name| name == "categories") {
        db.create_collection("categories").await?;
    }
    if !existing.iter().any(|name| name == "contacts") {
        db.create_collection("contacts").await?;
    }
    if !existing.iter().any(|name| name == "recurring_plans") {
        db.create_collection("recurring_plans").await?;
    }
    if !existing.iter().any(|name| name == "planned_entries") {
        db.create_collection("planned_entries").await?;
    }
    if !existing.iter().any(|name| name == "transactions") {
        db.create_collection("transactions").await?;
    }
    if !existing.iter().any(|name| name == "forecasts") {
        db.create_collection("forecasts").await?;
    }
    Ok(())
}

pub(super) async fn seed_default_companies(
    db: &Database,
    companies: &[String],
) -> Result<HashMap<String, ObjectId>> {
    let mut map = HashMap::new();
    let companies_coll = db.collection::<Company>("company");
    for name in companies {
        let slug = slugify(name);
        // Insert if not exists
        let existing = companies_coll
            .find_one(doc! { "slug": slug.clone() })
            .await?
            .and_then(|c| c.id);
        if let Some(id) = existing {
            map.insert(name.clone(), id);
            continue;
        }
        let result = companies_coll
            .insert_one(Company {
                id: None,
                name: name.clone(),
                slug: slug.clone(),
                default_currency: "MXN".to_string(),
                is_active: true,
                created_at: None,
                updated_at: None,
                notes: None,
            })
            .await?;
        let id = result
            .inserted_id
            .as_object_id()
            .context("company insert missing _id")?;
        map.insert(name.clone(), id);
    }
    Ok(map)
}

pub(super) async fn seed_default_users(
    db: &Database,
    users: &[SeedUser],
    company_ids: &HashMap<String, ObjectId>,
) -> Result<()> {
    let users_coll = db.collection::<User>("users");
    let memberships = db.collection::<UserCompany>("user_companies");

    for user in users {
        let primary_company_id = company_ids
            .get(&user.company)
            .context("missing company for user")?
            .clone();
        let company_ids_vec = {
            let mut ids = Vec::new();
            ids.push(primary_company_id.clone());
            for name in &user.companies {
                if let Some(id) = company_ids.get(name) {
                    if !ids.contains(id) {
                        ids.push(id.clone());
                    }
                }
            }
            ids
        };
        let role_final = user.role.clone();
        let companies_final: Vec<ObjectId> = company_ids_vec;

        // Upsert user
        let res = users_coll
            .update_one(
                doc! { "email": &user.email },
                doc! {
                    "$set": {
                        "email": &user.email,
                        "secret": &user.secret,
                        "company": &primary_company_id,
                        "companies": &companies_final,
                    }
                },
            )
            .await?;

        let user_id = if res.matched_count == 0 {
            let inserted = users_coll
                .insert_one(User {
                    id: None,
                    email: user.email.clone(),
                    secret: user.secret.clone(),
                    company_id: Some(primary_company_id.clone()),
                    company_ids: companies_final.clone(),
                })
                .await?;
            inserted
                .inserted_id
                .as_object_id()
                .context("missing user id after insert")?
        } else {
            users_coll
                .find_one(doc! { "email": &user.email })
                .await?
                .and_then(|u| u.id)
                .context("missing user id after update")?
        };

        // Reset memberships
        let _ = memberships
            .delete_many(doc! { "user_id": &user_id })
            .await;
        for cid in &companies_final {
            let _ = memberships
                .insert_one(UserCompany {
                    id: None,
                    user_id: user_id.clone(),
                    company_id: cid.clone(),
                    role: role_final.clone(),
                })
                .await;
        }
    }
    Ok(())
}

pub(super) async fn seed_sample_finance(db: &Database, company_id: Option<ObjectId>) -> Result<()> {
    let company_id = if let Some(id) = company_id {
        id
    } else {
        return Ok(());
    };

    let accounts_coll = db.collection::<Account>("accounts");
    if !is_collection_empty(&accounts_coll).await? {
        return Ok(());
    }

    let categories_coll = db.collection::<Category>("categories");
    let contacts_coll = db.collection::<Contact>("contacts");
    let plans_coll = db.collection::<RecurringPlan>("recurring_plans");
    let planned_coll = db.collection::<PlannedEntry>("planned_entries");
    let tx_coll = db.collection::<Transaction>("transactions");
    let forecast_coll = db.collection::<Forecast>("forecasts");

    let accounts_seed: Vec<Account> = load_json_array("ACCOUNTS_FILE", "./data/accounts.json")?;
    let categories_seed: Vec<Category> =
        load_json_array("CATEGORIES_FILE", "./data/categories.json")?;
    let contacts_seed: Vec<Contact> = load_json_array("CONTACTS_FILE", "./data/contacts.json")?;
    let plans_seed: Vec<RecurringPlan> =
        load_json_array("RECURRING_PLANS_FILE", "./data/recurring_plans.json")?;
    let planned_seed: Vec<PlannedEntry> =
        load_json_array("PLANNED_ENTRIES_FILE", "./data/planned_entries.json")?;
    let tx_seed: Vec<Transaction> =
        load_json_array("TRANSACTIONS_FILE", "./data/transactions.json")?;
    let forecasts_seed: Vec<Forecast> = load_json_array("FORECASTS_FILE", "./data/forecasts.json")?;

    let mut account_map = HashMap::new();
    for acc in accounts_seed {
        let old_id = acc.id.unwrap_or_else(ObjectId::new);
        let res = accounts_coll
            .insert_one(Account {
                id: None,
                company_id: company_id.clone(),
                name: acc.name,
                account_type: acc.account_type,
                currency: acc.currency,
                is_active: acc.is_active,
                created_at: acc.created_at,
                updated_at: acc.updated_at,
                notes: acc.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("account insert missing _id")?;
        account_map.insert(old_id, new_id);
    }

    let mut category_map = HashMap::new();
    for cat in categories_seed {
        let old_id = cat.id.unwrap_or_else(ObjectId::new);
        let parent_old = cat.parent_id;
        let parent_new = parent_old.and_then(|p| category_map.get(&p).cloned());
        let res = categories_coll
            .insert_one(Category {
                id: None,
                company_id: company_id.clone(),
                name: cat.name,
                flow_type: cat.flow_type,
                parent_id: parent_new,
                created_at: cat.created_at,
                updated_at: cat.updated_at,
                notes: cat.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("category insert missing _id")?;
        category_map.insert(old_id, new_id);
    }

    let mut contact_map = HashMap::new();
    for contact in contacts_seed {
        let old_id = contact.id.unwrap_or_else(ObjectId::new);
        let res = contacts_coll
            .insert_one(Contact {
                id: None,
                company_id: company_id.clone(),
                name: contact.name,
                contact_type: contact.contact_type,
                email: contact.email,
                phone: contact.phone,
                created_at: contact.created_at,
                updated_at: contact.updated_at,
                notes: contact.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("contact insert missing _id")?;
        contact_map.insert(old_id, new_id);
    }

    let mut plan_map = HashMap::new();
    for plan in plans_seed {
        let old_id = plan.id.unwrap_or_else(ObjectId::new);
        let category_id = remap_id(&category_map, &plan.category_id)?;
        let account_expected_id = remap_id(&account_map, &plan.account_expected_id)?;
        let contact_id = plan.contact_id.and_then(|c| contact_map.get(&c).cloned());

        let res = plans_coll
            .insert_one(RecurringPlan {
                id: None,
                company_id: company_id.clone(),
                name: plan.name,
                flow_type: plan.flow_type,
                category_id,
                account_expected_id,
                contact_id,
                amount_estimated: plan.amount_estimated,
                frequency: plan.frequency,
                day_of_month: plan.day_of_month,
                start_date: plan.start_date,
                end_date: plan.end_date,
                is_active: plan.is_active,
                version: plan.version,
                created_at: plan.created_at,
                updated_at: plan.updated_at,
                notes: plan.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("recurring plan insert missing _id")?;
        plan_map.insert(old_id, new_id);
    }

    let mut planned_map = HashMap::new();
    for pe in planned_seed {
        let old_id = pe.id.unwrap_or_else(ObjectId::new);
        let category_id = remap_id(&category_map, &pe.category_id)?;
        let account_expected_id = remap_id(&account_map, &pe.account_expected_id)?;
        let contact_id = pe.contact_id.and_then(|c| contact_map.get(&c).cloned());
        let recurring_plan_id = pe.recurring_plan_id.and_then(|p| plan_map.get(&p).cloned());

        let res = planned_coll
            .insert_one(PlannedEntry {
                id: None,
                company_id: company_id.clone(),
                recurring_plan_id,
                recurring_plan_version: pe.recurring_plan_version,
                name: pe.name,
                flow_type: pe.flow_type,
                category_id,
                account_expected_id,
                contact_id,
                amount_estimated: pe.amount_estimated,
                due_date: pe.due_date,
                status: pe.status,
                created_at: pe.created_at,
                updated_at: pe.updated_at,
                notes: pe.notes,
            })
            .await?;
        let new_id = res
            .inserted_id
            .as_object_id()
            .context("planned entry insert missing _id")?;
        planned_map.insert(old_id, new_id);
    }

    for tx in tx_seed {
        let category_id = remap_id(&category_map, &tx.category_id)?;
        let account_from_id = tx
            .account_from_id
            .and_then(|a| account_map.get(&a).cloned());
        let account_to_id = tx.account_to_id.and_then(|a| account_map.get(&a).cloned());
        let planned_entry_id = tx
            .planned_entry_id
            .and_then(|p| planned_map.get(&p).cloned());

        let _ = tx_coll
            .insert_one(Transaction {
                id: None,
                company_id: company_id.clone(),
                date: tx.date,
                description: tx.description,
                transaction_type: tx.transaction_type,
                category_id,
                account_from_id,
                account_to_id,
                amount: tx.amount,
                planned_entry_id,
                is_confirmed: tx.is_confirmed,
                created_at: tx.created_at,
                updated_at: tx.updated_at,
                notes: tx.notes,
            })
            .await?;
    }

    for fc in forecasts_seed {
        let _ = forecast_coll
            .insert_one(Forecast {
                id: None,
                company_id: company_id.clone(),
                generated_at: fc.generated_at,
                generated_by_user_id: fc.generated_by_user_id,
                start_date: fc.start_date,
                end_date: fc.end_date,
                currency: fc.currency,
                projected_income_total: fc.projected_income_total,
                projected_expense_total: fc.projected_expense_total,
                projected_net: fc.projected_net,
                initial_balance: fc.initial_balance,
                final_balance: fc.final_balance,
                details: fc.details,
                scenario_name: fc.scenario_name,
                notes: fc.notes,
            })
            .await?;
    }

    Ok(())
}

async fn is_collection_empty<T: Send + Sync>(coll: &Collection<T>) -> Result<bool> {
    let count = coll.estimated_document_count().await?;
    Ok(count == 0)
}

fn remap_id(map: &HashMap<ObjectId, ObjectId>, original: &ObjectId) -> Result<ObjectId> {
    map.get(original)
        .cloned()
        .context("missing referenced id during seeding")
}
