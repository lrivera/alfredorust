//! Routed screens. Each page reads `Me` from context and talks to the backend
//! through the typed `api` client.

mod accounts;
mod categories;
mod contacts;
mod dashboard;
mod forecasts;
mod orders;
mod planned_entries;
mod projects;
mod recurring_plans;
mod resource_logs;
mod resources;
mod transactions;

pub use accounts::AccountsPage;
pub use categories::CategoriesPage;
pub use contacts::ContactsPage;
pub use dashboard::Dashboard;
pub use forecasts::ForecastsPage;
pub use orders::OrdersPage;
pub use planned_entries::PlannedEntriesPage;
pub use projects::ProjectsPage;
pub use recurring_plans::RecurringPlansPage;
pub use resource_logs::ResourceLogsPage;
pub use resources::ResourcesPage;
pub use transactions::TransactionsPage;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

/// An `(id, label)` option for a `<select>`.
pub(crate) type Options = Vec<(String, String)>;

/// Load account options (id → name) into `target` on mount.
pub(crate) fn load_account_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::Account>>("/api/admin/accounts").await {
            target.set(v.into_iter().map(|a| (a.id, a.name)).collect());
        }
    });
}

/// Load category options (id → name) into `target` on mount.
pub(crate) fn load_category_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::Category>>("/api/admin/categories").await {
            target.set(v.into_iter().map(|c| (c.id, c.name)).collect());
        }
    });
}

/// Load contact options (id → name) into `target` on mount.
pub(crate) fn load_contact_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::Contact>>("/api/admin/contacts").await {
            target.set(v.into_iter().map(|c| (c.id, c.name)).collect());
        }
    });
}

/// Load project options (id → title) into `target` on mount.
pub(crate) fn load_project_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::Project>>("/api/admin/projects").await {
            target.set(v.into_iter().map(|p| (p.id, p.title)).collect());
        }
    });
}

/// Load planned-entry options (id → name) into `target` on mount.
pub(crate) fn load_planned_entry_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::PlannedEntry>>("/api/admin/planned-entries").await {
            target.set(v.into_iter().map(|e| (e.id, e.name)).collect());
        }
    });
}

/// URL for another tenant: swap the leftmost host label for the company slug,
/// preserving protocol and port. Company switching is a full navigation.
pub(crate) fn switch_company_href(slug: &str) -> String {
    let loc = window().location();
    let proto = loc.protocol().unwrap_or_else(|_| "https:".to_string());
    let host = loc.host().unwrap_or_default();
    let rest = host.split_once('.').map(|(_, r)| r).unwrap_or(&host);
    // Land on the SPA (/v2) of the destination tenant, not the legacy root.
    format!("{proto}//{slug}.{rest}/v2/")
}

// --- shared label / formatting helpers ------------------------------------

pub(crate) fn flow_label(value: &str) -> &str {
    match value {
        "income" => "Ingreso",
        "expense" => "Egreso",
        other => other,
    }
}

pub(crate) fn money(n: f64) -> String {
    format!("{n:.2}")
}

/// Backend datetime fields are parsed as RFC3339. `<input type="date">` yields
/// `YYYY-MM-DD`, so append a midnight-UTC time when needed.
pub(crate) fn date_to_rfc3339(date: &str) -> String {
    let d = date.trim();
    if d.is_empty() || d.contains('T') {
        d.to_string()
    } else {
        format!("{d}T00:00:00Z")
    }
}

/// Inverse for edit forms: take the `YYYY-MM-DD` prefix of an RFC3339 datetime
/// so it fits an `<input type="date">`.
pub(crate) fn rfc3339_to_date(value: &str) -> String {
    value.get(..10).unwrap_or(value).to_string()
}

/// `<input type="datetime-local">` yields `YYYY-MM-DDTHH:MM` (no zone). Make it
/// RFC3339 (treat as UTC) for the backend.
pub(crate) fn local_dt_to_rfc3339(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() || v.ends_with('Z') {
        v.to_string()
    } else if v.len() == 16 {
        format!("{v}:00Z")
    } else {
        format!("{v}Z")
    }
}

/// Inverse: RFC3339 -> `YYYY-MM-DDTHH:MM` for a datetime-local input.
pub(crate) fn rfc3339_to_local_dt(value: &str) -> String {
    value.get(..16).unwrap_or(value).to_string()
}

/// Load resource options (id → name) into `target` on mount.
pub(crate) fn load_resource_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::Resource>>("/api/admin/resources").await {
            target.set(v.into_iter().map(|r| (r.id, r.name)).collect());
        }
    });
}

/// Load concept-status options (id → name) into `target` on mount.
pub(crate) fn load_concept_status_options(target: RwSignal<Options>) {
    spawn_local(async move {
        if let Ok(v) = api::get_json::<Vec<api::ConceptStatus>>("/api/admin/concept_statuses").await
        {
            target.set(v.into_iter().map(|s| (s.id, s.name)).collect());
        }
    });
}
