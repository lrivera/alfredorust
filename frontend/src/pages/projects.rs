use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{date_to_rfc3339, load_category_options, load_contact_options, money, rfc3339_to_date, Options};
use crate::api::{self, ApiError, Me, ProjectPayload, ProjectRow};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

const PRIORITIES: &[(&str, &str)] = &[
    ("low", "Baja"),
    ("medium", "Media"),
    ("high", "Alta"),
    ("urgent", "Urgente"),
];

fn opt_num(s: &str) -> Option<f64> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.parse::<f64>().ok()).flatten()
}

#[component]
pub fn ProjectsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";
    // Without `view_project_money` the budget column is hidden (v1 parity); the
    // backend also nulls the amount for these users.
    let can_money = is_admin || me.can("view_project_money");

    let items = RwSignal::new(None::<Result<Vec<ProjectRow>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<ProjectRow>>("/api/admin/projects").await));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let contacts = RwSignal::new(Options::new());
    load_category_options(categories);
    load_contact_options(contacts);

    let editing = RwSignal::new(None::<String>);
    let title = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let contact = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let priority = RwSignal::new("medium".to_string());
    let budget = RwSignal::new(String::new());
    let scheduled = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        title.set(String::new());
        description.set(String::new());
        contact.set(String::new());
        category.set(String::new());
        priority.set("medium".to_string());
        budget.set(String::new());
        scheduled.set(String::new());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let opt = |s: String| (!s.trim().is_empty()).then_some(s);
        let sched = scheduled.get_untracked();
        let payload = ProjectPayload {
            title: title.get_untracked().trim().to_string(),
            contact_id: opt(contact.get_untracked()),
            category_id: opt(category.get_untracked()),
            description: opt(description.get_untracked()),
            priority: priority.get_untracked(),
            total_budget: opt_num(&budget.get_untracked()),
            scheduled_at: (!sched.trim().is_empty()).then(|| date_to_rfc3339(&sched)),
            notes: opt(notes.get_untracked()),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/projects/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/projects", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el proyecto"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if title.get().trim().is_empty() {
            form_error.set(Some("El título es obligatorio".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::ProjectDetail>(&format!("/api/admin/projects/{id}")).await
            {
                title.set(d.title);
                description.set(d.description.unwrap_or_default());
                contact.set(d.contact_id.unwrap_or_default());
                category.set(d.category_id.unwrap_or_default());
                priority.set(d.priority);
                budget.set(d.total_budget.map(|v| v.to_string()).unwrap_or_default());
                scheduled.set(d.scheduled_at.as_deref().map(rfc3339_to_date).unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/projects/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };
    let advance = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/projects/{id}/advance")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Proyectos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar proyecto" } else { "Nuevo proyecto" }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">"Título"</label>
                                        <Input
                                            value=title
                                            on_input=Callback::new(move |v| title.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Prioridad"
                                        </label>
                                        <Select value=priority>
                                            {PRIORITIES
                                                .iter()
                                                .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Cliente (opcional)"
                                        </label>
                                        <Select value=contact>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                contacts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Categoría (opcional)"
                                        </label>
                                        <Select value=category>
                                            <option value="">"— Ninguna —"</option>
                                            {move || {
                                                categories
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Presupuesto (opcional)"
                                        </label>
                                        <Input
                                            value=budget
                                            on_input=Callback::new(move |v| budget.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Fecha límite (opcional)"
                                        </label>
                                        <Input
                                            value=scheduled
                                            on_input=Callback::new(move |v| scheduled.set(v))
                                            r#type="date"
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Descripción (opcional)"
                                        </label>
                                        <Input
                                            value=description
                                            on_input=Callback::new(move |v| description.set(v))
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">"Notas"</label>
                                        <Input value=notes on_input=Callback::new(move |v| notes.set(v)) />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear proyecto"
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
                                        {move || {
                                            form_error
                                                .get()
                                                .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                                        }}
                                    </div>
                                </form>
                            </CardContent>
                        </Card>
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}

            {move || match items.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar los proyectos."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin proyectos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Título"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2 font-medium">"Prioridad"</th>
                                        {can_money
                                            .then(|| {
                                                view! { <th class="px-4 py-2 font-medium">"Presupuesto"</th> }
                                            })}
                                        <th class="px-4 py-2 font-medium">"Fecha límite"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|p| {
                                            let id = p.id.clone();
                                            let status = if p.status_label.is_empty() {
                                                p.status.clone()
                                            } else {
                                                p.status_label.clone()
                                            };
                                            let prio = if p.priority_label.is_empty() {
                                                p.priority.clone()
                                            } else {
                                                p.priority_label.clone()
                                            };
                                            let budget = p.total_budget.map(money).unwrap_or_default();
                                            let sched = p.scheduled_at.as_deref().map(rfc3339_to_date).unwrap_or_default();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{p.title}</td>
                                                    <td class="px-4 py-2">{status}</td>
                                                    <td class="px-4 py-2">{prio}</td>
                                                    {can_money
                                                        .then(|| view! { <td class="px-4 py-2">{budget}</td> })}
                                                    <td class="px-4 py-2 text-slate-500">{sched}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {
                                                            let vid = id.clone();
                                                            view! {
                                                                <a
                                                                    href=format!("/v2/projects/{vid}")
                                                                    class="mr-1 inline-flex items-center rounded-md px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100"
                                                                >
                                                                    "Ver"
                                                                </a>
                                                            }
                                                        }
                                                        {move || {
                                                            if is_admin {
                                                                let aid = id.clone();
                                                                let eid = id.clone();
                                                                let did = id.clone();
                                                                view! {
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        class="text-emerald-700 hover:bg-emerald-50"
                                                                        on:click=move |_| advance(aid.clone())
                                                                    >
                                                                        "Avanzar"
                                                                    </Button>
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        on:click=move |_| begin_edit(eid.clone())
                                                                    >
                                                                        "Editar"
                                                                    </Button>
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        class="text-red-600 hover:bg-red-50"
                                                                        on:click=move |_| delete_one(did.clone())
                                                                    >
                                                                        "Eliminar"
                                                                    </Button>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                ().into_any()
                                                            }
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
