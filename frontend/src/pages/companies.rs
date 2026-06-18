//! Companies admin: list the tenants the user administers, create/edit them,
//! and delete non-active ones. Slug is validated client-side for fast feedback;
//! the server stays the authority (uniqueness, reserved names).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ApiError, CompanyData, CompanyPayload};
use crate::components::{
    Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input,
};

/// Slugs the backend reserves; mirror them for a fast client-side check.
const RESERVED_SLUGS: &[&str] = &["app", "www", "api", "admin", "mail", "static"];

/// Client-side mirror of the backend slug rule: empty (auto-derived) is fine;
/// otherwise lowercase ASCII letters, digits and hyphens, no leading/trailing
/// hyphen, ≤64 chars, and not a reserved name.
fn slug_problem(slug: &str) -> Option<&'static str> {
    let s = slug.trim();
    if s.is_empty() {
        return None;
    }
    let well_formed = s.len() <= 64
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !well_formed {
        return Some(
            "Slug inválido. Usa solo minúsculas, números y guiones, sin iniciar ni terminar con guion (máx 64).",
        );
    }
    if RESERVED_SLUGS.contains(&s) {
        return Some("Ese slug está reservado y no puede usarse.");
    }
    None
}

fn opt(value: String) -> Option<String> {
    let v = value.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[component]
pub fn CompaniesPage() -> impl IntoView {
    let items = RwSignal::new(None::<Result<Vec<CompanyData>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<CompanyData>>("/api/admin/companies").await));
        });
    };
    reload();

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let currency = RwSignal::new("MXN".to_string());
    let notes = RwSignal::new(String::new());
    let is_active = RwSignal::new(true);
    let form_error = RwSignal::new(None::<String>);
    let danger_msg = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        slug.set(String::new());
        currency.set("MXN".to_string());
        notes.set(String::new());
        is_active.set(true);
        form_error.set(None);
        danger_msg.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let payload = CompanyPayload {
            name: name.get_untracked().trim().to_string(),
            slug: opt(slug.get_untracked()),
            default_currency: opt(currency.get_untracked()),
            is_active: Some(is_active.get_untracked()),
            notes: opt(notes.get_untracked()),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/companies/{id}/update"), &payload).await
                }
                None => api::post_json("/api/admin/companies", &payload).await,
            }
        }
    });

    Effect::new(move |_| {
        if let Some(r) = save.value().get() {
            match r {
                Ok(()) => {
                    reset_form();
                    reload();
                }
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar la compañía"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if name.get().trim().is_empty() {
            form_error.set(Some("El nombre es obligatorio".into()));
            return;
        }
        if let Some(msg) = slug_problem(&slug.get()) {
            form_error.set(Some(msg.into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };

    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<CompanyData>(&format!("/api/admin/companies/{id}")).await
            {
                name.set(d.name);
                slug.set(d.slug);
                currency.set(d.default_currency);
                notes.set(d.notes.unwrap_or_default());
                is_active.set(d.is_active);
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/companies/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };
    // Danger zone: bulk-delete the editing company's CFDIs or transactions,
    // behind a confirm. `suffix` is the endpoint tail.
    let run_danger = move |suffix: &'static str, confirm_msg: &'static str, ok_msg: &'static str| {
        let Some(id) = editing.get_untracked() else { return };
        if !window().confirm_with_message(confirm_msg).unwrap_or(false) {
            return;
        }
        danger_msg.set(None);
        spawn_local(async move {
            match api::post_empty(&format!("/api/admin/companies/{id}/{suffix}")).await {
                Ok(()) => danger_msg.set(Some(ok_msg.to_string())),
                Err(_) => danger_msg.set(Some("No se pudo completar".into())),
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Compañías"</h1>

            <Card class="max-w-2xl">
                <CardHeader>
                    <CardTitle>
                        {move || {
                            if editing.get().is_some() {
                                "Editar compañía"
                            } else {
                                "Nueva compañía"
                            }
                        }}
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    <form on:submit=submit class="grid gap-3 sm:grid-cols-2">
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-slate-700">"Nombre"</label>
                            <Input
                                value=name
                                on_input=Callback::new(move |v| name.set(v))
                                placeholder="Mi Empresa S.A."
                                required=true
                            />
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-slate-700">
                                "Slug (subdominio)"
                            </label>
                            <Input
                                value=slug
                                on_input=Callback::new(move |v| slug.set(v))
                                placeholder="ej. research"
                            />
                            <p class="text-xs text-slate-500">
                                "Se genera del nombre si lo dejas vacío."
                            </p>
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-slate-700">
                                "Moneda por defecto"
                            </label>
                            <Input
                                value=currency
                                on_input=Callback::new(move |v| currency.set(v))
                                placeholder="MXN"
                            />
                        </div>
                        <div class="flex items-end">
                            <Checkbox checked=is_active label="Activa" />
                        </div>
                        <div class="space-y-1 sm:col-span-2">
                            <label class="block text-sm font-medium text-slate-700">"Notas"</label>
                            <Input
                                value=notes
                                on_input=Callback::new(move |v| notes.set(v))
                                placeholder="Opcional"
                            />
                        </div>
                        <div class="flex items-end gap-2">
                            <Button r#type="submit" disabled=pending>
                                {move || {
                                    if pending.get() {
                                        "Guardando…"
                                    } else if editing.get().is_some() {
                                        "Guardar cambios"
                                    } else {
                                        "Crear compañía"
                                    }
                                }}
                            </Button>
                            {move || {
                                editing
                                    .get()
                                    .is_some()
                                    .then(|| {
                                        view! {
                                            <Button
                                                variant=ButtonVariant::Outline
                                                on:click=move |_| reset_form()
                                            >
                                                "Cancelar"
                                            </Button>
                                        }
                                    })
                            }}
                        </div>
                        {move || {
                            form_error
                                .get()
                                .map(|m| {
                                    view! {
                                        <p class="text-sm text-red-600 sm:col-span-2">{m}</p>
                                    }
                                })
                        }}
                    </form>
                </CardContent>
            </Card>

            {move || {
                if editing.get().is_none() {
                    return ().into_any();
                }
                view! {
                    <Card class="max-w-2xl border-rose-200">
                        <CardHeader>
                            <CardTitle>"Zona de pruebas"</CardTitle>
                        </CardHeader>
                        <CardContent>
                            <div class="flex flex-wrap gap-3">
                                <button
                                    type="button"
                                    class="inline-flex items-center rounded-md border border-rose-200 bg-rose-50 px-3 py-1.5 text-sm font-medium text-rose-700 hover:bg-rose-100"
                                    on:click=move |_| {
                                        run_danger(
                                            "cfdis/delete_all",
                                            "¿Borrar TODOS los CFDIs de esta compañía? Esta acción no se puede deshacer.",
                                            "CFDIs borrados",
                                        )
                                    }
                                >
                                    "Borrar todos los CFDIs"
                                </button>
                                <button
                                    type="button"
                                    class="inline-flex items-center rounded-md border border-rose-200 bg-rose-50 px-3 py-1.5 text-sm font-medium text-rose-700 hover:bg-rose-100"
                                    on:click=move |_| {
                                        run_danger(
                                            "transactions/delete_all",
                                            "¿Borrar TODAS las transacciones de esta compañía? Esta acción no se puede deshacer.",
                                            "Transacciones borradas",
                                        )
                                    }
                                >
                                    "Borrar todas las transacciones"
                                </button>
                            </div>
                            {move || {
                                danger_msg
                                    .get()
                                    .map(|m| view! { <p class="mt-2 text-sm text-rose-700">{m}</p> })
                            }}
                        </CardContent>
                    </Card>
                }
                    .into_any()
            }}

            {move || match items.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las compañías."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin compañías todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Moneda"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|c| {
                                            let eid = c.id.clone();
                                            let did = c.id.clone();
                                            let is_current = c.is_current;
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{c.name}</td>
                                                    <td class="px-4 py-2 text-slate-500">
                                                        {c.default_currency}
                                                    </td>
                                                    <td class="px-4 py-2">
                                                        {if c.is_active { "Activa" } else { "Inactiva" }}
                                                    </td>
                                                    <td class="px-4 py-2 text-right">
                                                        <Button
                                                            variant=ButtonVariant::Ghost
                                                            on:click=move |_| begin_edit(eid.clone())
                                                        >
                                                            "Editar"
                                                        </Button>
                                                        {if is_current {
                                                            view! {
                                                                <span class="ml-1 rounded bg-slate-100 px-2 py-1 text-xs text-slate-500">
                                                                    "Actual"
                                                                </span>
                                                            }
                                                                .into_any()
                                                        } else {
                                                            view! {
                                                                <Button
                                                                    variant=ButtonVariant::Ghost
                                                                    class="text-red-600 hover:bg-red-50"
                                                                    on:click=move |_| delete_one(did.clone())
                                                                >
                                                                    "Eliminar"
                                                                </Button>
                                                            }
                                                                .into_any()
                                                        }}
                                                    </td>
                                                </tr>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn slug_problem_accepts_empty_and_valid() {
        assert!(slug_problem("").is_none());
        assert!(slug_problem("  ").is_none());
        assert!(slug_problem("acme-123").is_none());
        assert!(slug_problem("research").is_none());
    }

    #[wasm_bindgen_test]
    fn slug_problem_rejects_bad_and_reserved() {
        assert!(slug_problem("Acme").is_some());
        assert!(slug_problem("-acme").is_some());
        assert!(slug_problem("acme-").is_some());
        assert!(slug_problem("acme_test").is_some());
        assert!(slug_problem(&"a".repeat(65)).is_some());
        assert!(slug_problem("app").is_some());
        assert!(slug_problem("admin").is_some());
    }
}
