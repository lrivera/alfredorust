//! Routed screens. Each page reads `Me` from context and talks to the backend
//! through the typed `api` client.

mod account;
mod accounts;
mod categories;
mod cfdi;
mod charts;
mod companies;
mod concept_statuses;
mod contacts;
mod dashboard;
mod forecasts;
mod orders;
mod planned_entries;
mod project_detail;
mod projects;
mod recurring_plans;
mod resource_logs;
mod resource_usages;
mod resources;
mod sat_configs;
mod tiempo;
mod transactions;
mod users;

pub use account::AccountPage;
pub use accounts::AccountsPage;
pub use categories::CategoriesPage;
pub use cfdi::CfdiPage;
pub use companies::CompaniesPage;
pub use concept_statuses::ConceptStatusesPage;
pub use contacts::ContactsPage;
pub use dashboard::Dashboard;
pub use forecasts::ForecastsPage;
pub use orders::OrdersPage;
pub use planned_entries::PlannedEntriesPage;
pub use project_detail::ProjectDetailPage;
pub use projects::ProjectsPage;
pub use recurring_plans::RecurringPlansPage;
pub use resource_logs::ResourceLogsPage;
pub use resource_usages::ResourceUsagesPage;
pub use resources::ResourcesPage;
pub use sat_configs::SatConfigsPage;
pub use tiempo::TiempoPage;
pub use transactions::TransactionsPage;
pub use users::UsersPage;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::{Badge, BadgeTone};

/// An `(id, label)` option for a `<select>`.
pub(crate) type Options = Vec<(String, String)>;

/// Replace an element's content with raw HTML. Used to host the v1 timeline /
/// resource-usage grid markup verbatim inside the SPA.
pub(crate) fn set_html(el: &web_sys::Element, html: &str) {
    el.set_inner_html(html);
}

/// Append an inline `<script>` to the document so it executes — runs the ported
/// v1 widget JS against the markup just rendered by [`set_html`].
pub(crate) fn run_script(src: &str) {
    let doc = document();
    if let Ok(script) = doc.create_element("script") {
        script.set_text_content(Some(src));
        if let Some(body) = doc.body() {
            let _ = body.append_child(&script);
        }
    }
}

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

// --- badge helpers --------------------------------------------------------
// Small reusable renderers so status/type/flow columns read as colored pills
// consistently across screens. Tone is chosen from the value, not the label,
// where possible.

/// Income (green) / expense (rose) flow as a badge.
pub(crate) fn flow_badge(value: &str) -> AnyView {
    let tone = match value {
        "income" => BadgeTone::Success,
        "expense" => BadgeTone::Danger,
        _ => BadgeTone::Neutral,
    };
    let label = flow_label(value).to_string();
    view! { <Badge tone=tone>{label}</Badge> }.into_any()
}

/// Boolean flag (active/confirmed/…) with caller-supplied labels so wording and
/// grammatical gender stay correct per screen. True → success, false → neutral.
pub(crate) fn bool_badge(on: bool, yes: &str, no: &str) -> AnyView {
    let (tone, label) = if on {
        (BadgeTone::Success, yes)
    } else {
        (BadgeTone::Neutral, no)
    };
    let label = label.to_string();
    view! { <Badge tone=tone>{label}</Badge> }.into_any()
}

/// A workflow status (planned/order/project). Tone is inferred from Spanish
/// keywords; unknown custom statuses fall back to neutral so they still render.
pub(crate) fn status_badge(label: &str) -> AnyView {
    let l = label.to_lowercase();
    let tone = if l.contains("cancel") {
        BadgeTone::Neutral
    } else if l.contains("vencid") || l.contains("atras") || l.contains("overdue") {
        BadgeTone::Danger
    } else if l.contains("parcial") || l.contains("partial") {
        BadgeTone::Warning
    } else if l.contains("cubiert")
        || l.contains("pagad")
        || l.contains("complet")
        || l.contains("termin")
        || l.contains("cerrad")
        || l.contains("listo")
    {
        BadgeTone::Success
    } else if l.contains("plan") || l.contains("pendiente") || l.contains("proceso") {
        BadgeTone::Info
    } else {
        BadgeTone::Neutral
    };
    let label = label.to_string();
    view! { <Badge tone=tone>{label}</Badge> }.into_any()
}

/// Project priority. Tone from the canonical value (low/medium/high); the label
/// is whatever the screen resolved (e.g. "Alta").
pub(crate) fn priority_badge(value: &str, label: &str) -> AnyView {
    let tone = match value {
        "high" | "urgent" => BadgeTone::Danger,
        "medium" => BadgeTone::Warning,
        "low" => BadgeTone::Neutral,
        _ => BadgeTone::Info,
    };
    let label = label.to_string();
    view! { <Badge tone=tone>{label}</Badge> }.into_any()
}

/// User role: admin highlighted (info), staff neutral.
pub(crate) fn role_badge(role: &str) -> AnyView {
    let tone = if role == "admin" {
        BadgeTone::Info
    } else {
        BadgeTone::Neutral
    };
    let role = role.to_string();
    view! {
        <Badge tone=tone class="uppercase">
            {role}
        </Badge>
    }
    .into_any()
}

/// A non-status classifier (account type, etc.) shown as a subtle info pill.
pub(crate) fn type_badge(label: &str) -> AnyView {
    let label = label.to_string();
    view! { <Badge tone=BadgeTone::Info>{label}</Badge> }.into_any()
}

/// Format a currency/value amount for display: `$1,232,543.90`, with thousands
/// separators, two decimals, and a leading `-$` for negatives. Reused
/// everywhere money/value is shown so it stays consistent.
pub(crate) fn money(n: f64) -> String {
    let neg = n < 0.0;
    let cents = (n.abs() * 100.0).round() as u64;
    let whole = (cents / 100).to_string();
    let frac = cents % 100;
    let bytes = whole.as_bytes();
    let len = bytes.len();
    let mut grouped = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }
    format!("{}${grouped}.{frac:02}", if neg { "-" } else { "" })
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

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn date_to_rfc3339_appends_midnight_utc() {
        assert_eq!(date_to_rfc3339("2026-01-02"), "2026-01-02T00:00:00Z");
        // Already a datetime or empty -> untouched.
        assert_eq!(date_to_rfc3339("2026-01-02T10:00:00Z"), "2026-01-02T10:00:00Z");
        assert_eq!(date_to_rfc3339("  "), "");
    }

    #[wasm_bindgen_test]
    fn rfc3339_to_date_takes_date_prefix() {
        assert_eq!(rfc3339_to_date("2026-01-02T10:30:00Z"), "2026-01-02");
        assert_eq!(rfc3339_to_date("2026-01-02"), "2026-01-02");
    }

    #[wasm_bindgen_test]
    fn local_dt_roundtrip() {
        // datetime-local (no zone) -> RFC3339.
        assert_eq!(local_dt_to_rfc3339("2026-01-02T10:30"), "2026-01-02T10:30:00Z");
        assert_eq!(local_dt_to_rfc3339("2026-01-02T10:30:00Z"), "2026-01-02T10:30:00Z");
        assert_eq!(local_dt_to_rfc3339(""), "");
        // RFC3339 -> datetime-local input value.
        assert_eq!(rfc3339_to_local_dt("2026-01-02T10:30:00Z"), "2026-01-02T10:30");
    }

    #[wasm_bindgen_test]
    fn flow_label_and_money() {
        assert_eq!(flow_label("income"), "Ingreso");
        assert_eq!(flow_label("expense"), "Egreso");
        assert_eq!(flow_label("weird"), "weird");
        assert_eq!(money(1234.5), "$1,234.50");
        assert_eq!(money(0.0), "$0.00");
        assert_eq!(money(1232543.9), "$1,232,543.90");
        assert_eq!(money(-2100.4), "-$2,100.40");
        assert_eq!(money(999.0), "$999.00");
    }

}
