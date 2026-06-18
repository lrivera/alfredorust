//! Users admin: list users with their companies and role, and create/edit them
//! with per-company memberships. Each membership carries a role (staff/admin)
//! and, for staff, a set of permission grants. Mirrors the v1 compound form.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    self, ApiError, CompanyData, Me, UserMembershipPayload, UserPayload, UserRowData,
};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input};

/// Staff permission grants: `(api_key, label)`. Order is the display order.
const PERMISSIONS: &[(&str, &str)] = &[
    ("view_projects", "Ver proyectos"),
    ("edit_resource_usage_today", "Editar uso recursos hoy"),
    ("view_resource_usage_history", "Ver historial uso recursos"),
    ("view_project_money", "Ver montos de proyectos"),
    ("view_timeline", "Ver timeline financiero"),
];

/// One assignable company membership in the edit form.
#[derive(Clone)]
struct MembershipState {
    company_id: String,
    company_name: String,
    included: bool,
    role: String,
    /// One flag per entry in [`PERMISSIONS`], same order.
    perms: Vec<bool>,
}

fn fresh_rows(companies: &[CompanyData]) -> Vec<MembershipState> {
    companies
        .iter()
        .map(|c| MembershipState {
            company_id: c.id.clone(),
            company_name: c.name.clone(),
            included: false,
            role: "staff".to_string(),
            perms: vec![false; PERMISSIONS.len()],
        })
        .collect()
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
pub fn UsersPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let my_username = me.username.clone();

    let items = RwSignal::new(None::<Result<Vec<UserRowData>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<UserRowData>>("/api/admin/users").await));
        });
    };
    reload();

    let editing = RwSignal::new(None::<String>);
    let email = RwSignal::new(String::new());
    let secret = RwSignal::new(String::new());
    // The existing secret of the user being edited, shown (read-only) under the
    // QR so it can be copied. Empty when creating.
    let secret_view = RwSignal::new(String::new());
    let memberships = RwSignal::new(Vec::<MembershipState>::new());
    let form_error = RwSignal::new(None::<String>);

    // Load the universe of assignable companies once.
    spawn_local(async move {
        if let Ok(list) = api::get_json::<Vec<CompanyData>>("/api/admin/companies").await {
            memberships.set(fresh_rows(&list));
        }
    });

    let reset_form = move || {
        editing.set(None);
        email.set(String::new());
        secret.set(String::new());
        secret_view.set(String::new());
        memberships.update(|rows| {
            for r in rows.iter_mut() {
                r.included = false;
                r.role = "staff".to_string();
                r.perms = vec![false; PERMISSIONS.len()];
            }
        });
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let memberships: Vec<UserMembershipPayload> = memberships
            .get_untracked()
            .into_iter()
            .filter(|m| m.included)
            .map(|m| {
                let permissions = PERMISSIONS
                    .iter()
                    .zip(m.perms.iter())
                    .filter(|(_, on)| **on)
                    .map(|((key, _), _)| key.to_string())
                    .collect();
                UserMembershipPayload {
                    company_id: m.company_id,
                    role: Some(m.role),
                    permissions,
                }
            })
            .collect();
        let payload = UserPayload {
            username: email.get_untracked().trim().to_string(),
            secret: opt(secret.get_untracked()),
            memberships,
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/users/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/users", &payload).await,
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
                // The backend sends "El nombre de usuario ya existe" on 409, which
                // humanize surfaces; other errors fall back.
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el usuario"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if email.get().trim().is_empty() {
            form_error.set(Some("El nombre de usuario es obligatorio".into()));
            return;
        }
        if !memberships.get().iter().any(|m| m.included) {
            form_error.set(Some("Selecciona al menos una compañía".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };

    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) = api::get_json::<UserRowData>(&format!("/api/admin/users/{id}")).await {
                email.set(d.username);
                secret.set(String::new());
                secret_view.set(d.secret.clone());
                memberships.update(|rows| {
                    for r in rows.iter_mut() {
                        if let Some(m) =
                            d.memberships.iter().find(|m| m.company_id == r.company_id)
                        {
                            r.included = true;
                            r.role = m.role.clone();
                            r.perms = PERMISSIONS
                                .iter()
                                .map(|(key, _)| m.permissions.iter().any(|p| p == key))
                                .collect();
                        } else {
                            r.included = false;
                            r.role = "staff".to_string();
                            r.perms = vec![false; PERMISSIONS.len()];
                        }
                    }
                });
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/users/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Usuarios"</h1>

            <Card class="max-w-2xl">
                <CardHeader>
                    <CardTitle>
                        {move || {
                            if editing.get().is_some() { "Editar usuario" } else { "Nuevo usuario" }
                        }}
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    <form on:submit=submit class="space-y-4">
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">
                                "Nombre de usuario"
                            </label>
                            <Input
                                value=email
                                on_input=Callback::new(move |v| email.set(v))
                                autocomplete="username"
                                placeholder="ej. alfredo@example.com"
                                required=true
                            />
                            <p class="text-xs text-muted-foreground">
                                "Identificador de acceso. Debe ser único."
                            </p>
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">
                                "Secreto TOTP"
                            </label>
                            <Input
                                value=secret
                                on_input=Callback::new(move |v| secret.set(v))
                                class="font-mono"
                                placeholder="Se genera automáticamente si lo dejas vacío"
                            />
                            <p class="text-xs text-muted-foreground">
                                "Comparte este valor con el usuario para configurar su autenticador."
                            </p>
                        </div>

                        <div class="space-y-2">
                            <p class="text-sm font-medium text-foreground">"Compañías y rol"</p>
                            {move || {
                                let rows = memberships.get();
                                if rows.is_empty() {
                                    return view! {
                                        <p class="text-sm text-muted-foreground">"Sin compañías disponibles."</p>
                                    }
                                        .into_any();
                                }
                                rows.into_iter()
                                    .enumerate()
                                    .map(|(i, m)| membership_row(memberships, i, m))
                                    .collect::<Vec<_>>()
                                    .into_any()
                            }}
                        </div>

                        {move || {
                            editing
                                .get()
                                .map(|id| {
                                    view! {
                                        <div class="space-y-1">
                                            <p class="text-sm font-medium text-foreground">
                                                "Código QR (autenticador)"
                                            </p>
                                            <img
                                                src=format!("/admin/users/{id}/qrcode")
                                                alt="Código QR TOTP"
                                                class="h-40 w-40 rounded border border-border bg-card p-1"
                                            />
                                            <p class="text-xs text-muted-foreground">
                                                "El usuario escanea este código para configurar su autenticador."
                                            </p>
                                            {move || {
                                                let s = secret_view.get();
                                                (!s.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <div class="space-y-1">
                                                                <label class="block text-xs font-medium text-muted-foreground">
                                                                    "Secreto TOTP (texto)"
                                                                </label>
                                                                <code class="block select-all rounded border border-border bg-muted px-2 py-1 font-mono text-xs break-all text-foreground">
                                                                    {s}
                                                                </code>
                                                            </div>
                                                        }
                                                    })
                                            }}
                                        </div>
                                    }
                                })
                        }}

                        {move || {
                            form_error
                                .get()
                                .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                        }}

                        <div class="flex items-center gap-2">
                            <Button r#type="submit" disabled=pending>
                                {move || {
                                    if pending.get() {
                                        "Guardando…"
                                    } else if editing.get().is_some() {
                                        "Guardar cambios"
                                    } else {
                                        "Crear usuario"
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
                    </form>
                </CardContent>
            </Card>

            {move || {
                let my_username = my_username.clone();
                match items.get() {
                    None => view! { <p class="text-muted-foreground">"Cargando…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="text-red-600">"No se pudieron cargar los usuarios."</p> }
                            .into_any()
                    }
                    Some(Ok(list)) if list.is_empty() => {
                        view! { <p class="text-muted-foreground">"Sin usuarios todavía."</p> }.into_any()
                    }
                    Some(Ok(list)) => {
                        view! {
                            <div class="overflow-hidden rounded-xl border border-border bg-card">
                                <table class="w-full text-left text-sm">
                                    <thead class="bg-muted text-muted-foreground">
                                        <tr>
                                            <th class="px-4 py-2 font-medium">"Usuario"</th>
                                            <th class="px-4 py-2 font-medium">"Compañías"</th>
                                            <th class="px-4 py-2 font-medium">"Rol"</th>
                                            <th class="px-4 py-2"></th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {list
                                            .into_iter()
                                            .map(|u| {
                                                let eid = u.id.clone();
                                                let did = u.id.clone();
                                                let is_self = u.username == my_username;
                                                view! {
                                                    <tr class="border-t border-border">
                                                        <td class="px-4 py-2">{u.username}</td>
                                                        <td class="px-4 py-2 text-muted-foreground">
                                                            {u.companies.join(", ")}
                                                        </td>
                                                        <td class="px-4 py-2 uppercase">{u.role}</td>
                                                        <td class="px-4 py-2 text-right">
                                                            <Button
                                                                variant=ButtonVariant::Ghost
                                                                on:click=move |_| begin_edit(eid.clone())
                                                            >
                                                                "Editar"
                                                            </Button>
                                                            {(!is_self)
                                                                .then(|| {
                                                                    view! {
                                                                        <Button
                                                                            variant=ButtonVariant::Ghost
                                                                            class="text-red-600 hover:bg-red-50"
                                                                            on:click=move |_| delete_one(did.clone())
                                                                        >
                                                                            "Eliminar"
                                                                        </Button>
                                                                    }
                                                                })}
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
                }
            }}
        </div>
    }
}

/// One company membership row: include checkbox, role select, and (for staff)
/// permission checkboxes. Mutations write back into the shared `memberships`
/// vec by index.
fn membership_row(
    memberships: RwSignal<Vec<MembershipState>>,
    i: usize,
    m: MembershipState,
) -> impl IntoView {
    let included = m.included;
    let role = m.role.clone();
    let show_perms = included && role == "staff";

    view! {
        <div class="rounded-md border border-border p-3">
            <div class="flex items-center justify-between gap-3">
                <label class="inline-flex items-center gap-2 text-sm text-foreground">
                    <input
                        type="checkbox"
                        class="h-4 w-4 rounded border-border"
                        prop:checked=included
                        on:change=move |ev| {
                            let on = event_target_checked(&ev);
                            memberships.update(|rows| rows[i].included = on);
                        }
                    />
                    {m.company_name.clone()}
                </label>
                {included
                    .then(|| {
                        let current = role.clone();
                        view! {
                            <select
                                class="rounded-md border border-border px-2 py-1 text-sm"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    memberships.update(|rows| rows[i].role = v);
                                }
                            >
                                <option value="staff" selected=current == "staff">
                                    "Staff"
                                </option>
                                <option value="admin" selected=current == "admin">
                                    "Admin"
                                </option>
                            </select>
                        }
                    })}
            </div>
            {show_perms
                .then(|| {
                    view! {
                        <div class="mt-2 grid gap-1 pl-6 sm:grid-cols-2">
                            <p class="text-xs font-semibold uppercase text-muted-foreground sm:col-span-2">
                                "Permisos staff"
                            </p>
                            {PERMISSIONS
                                .iter()
                                .enumerate()
                                .map(|(pi, (_, plabel))| {
                                    let on = m.perms.get(pi).copied().unwrap_or(false);
                                    view! {
                                        <label class="inline-flex items-center gap-2 text-sm text-foreground">
                                            <input
                                                type="checkbox"
                                                class="h-4 w-4 rounded border-border"
                                                prop:checked=on
                                                on:change=move |ev| {
                                                    let checked = event_target_checked(&ev);
                                                    memberships.update(|rows| rows[i].perms[pi] = checked);
                                                }
                                            />
                                            {*plabel}
                                        </label>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                    }
                })}
        </div>
    }
}
