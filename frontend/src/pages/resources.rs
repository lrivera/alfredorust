use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{load_concept_status_options, money, Options};
use crate::api::{self, ApiError, Me, Resource, ResourcePayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input, Select};

const TYPES: &[(&str, &str)] = &[
    ("machinery", "Maquinaria"),
    ("vehicle", "Vehículo"),
    ("equipment", "Equipo"),
    ("other", "Otro"),
];

fn type_label(value: &str) -> &str {
    TYPES.iter().find(|(v, _)| *v == value).map(|(_, l)| *l).unwrap_or(value)
}

#[component]
pub fn ResourcesPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Resource>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<Resource>>("/api/admin/resources").await));
        });
    };
    reload();

    let statuses = RwSignal::new(Options::new());
    load_concept_status_options(statuses);

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let rtype = RwSignal::new("machinery".to_string());
    let cost = RwSignal::new(String::new());
    let currency = RwSignal::new("MXN".to_string());
    let is_active = RwSignal::new(true);
    let allowed = RwSignal::new(Vec::<String>::new());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let toggle_status = move |id: String| {
        allowed.update(|v| {
            if let Some(p) = v.iter().position(|x| *x == id) {
                v.remove(p);
            } else {
                v.push(id);
            }
        });
    };

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        rtype.set("machinery".to_string());
        cost.set(String::new());
        currency.set("MXN".to_string());
        is_active.set(true);
        allowed.set(Vec::new());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let notes_val = notes.get_untracked();
        let payload = ResourcePayload {
            name: name.get_untracked().trim().to_string(),
            resource_type: rtype.get_untracked(),
            is_active: is_active.get_untracked(),
            hourly_cost: cost.get_untracked().trim().parse().unwrap_or(0.0),
            currency: Some(currency.get_untracked()),
            allowed_status_ids: allowed.get_untracked(),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/resources/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/resources", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el recurso"))),
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
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::ResourceDetail>(&format!("/api/admin/resources/{id}")).await
            {
                name.set(d.name);
                rtype.set(d.resource_type);
                cost.set(format!("{}", d.hourly_cost));
                currency.set(d.currency);
                is_active.set(d.is_active);
                allowed.set(d.allowed_status_ids);
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/resources/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Recursos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-3xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar recurso" } else { "Nuevo recurso" }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Nombre"</label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Tipo"</label>
                                        <Select value=rtype>
                                            {TYPES
                                                .iter()
                                                .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Costo por hora"
                                        </label>
                                        <Input
                                            value=cost
                                            on_input=Callback::new(move |v| cost.set(v))
                                            inputmode="decimal"
                                            placeholder="0.00"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Moneda"</label>
                                        <Input
                                            value=currency
                                            on_input=Callback::new(move |v| currency.set(v))
                                        />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_active label="Activo" />
                                    </div>
                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Mostrar en estados"
                                        </label>
                                        <div class="flex flex-wrap gap-3 rounded-md border border-slate-200 p-3">
                                            {move || {
                                                let sel = allowed.get();
                                                statuses
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| {
                                                        let checked = sel.contains(&id);
                                                        let tid = id.clone();
                                                        view! {
                                                            <label class="inline-flex items-center gap-2 text-sm">
                                                                <input
                                                                    type="checkbox"
                                                                    class="h-4 w-4 rounded border-slate-300"
                                                                    prop:checked=checked
                                                                    on:change=move |_| toggle_status(tid.clone())
                                                                />
                                                                {label}
                                                            </label>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </div>
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
                                                    "Crear recurso"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los recursos."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin recursos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Tipo"</th>
                                        <th class="px-4 py-2 font-medium">"Costo/hora"</th>
                                        <th class="px-4 py-2 font-medium">"Moneda"</th>
                                        <th class="px-4 py-2 font-medium">"Estados"</th>
                                        <th class="px-4 py-2 font-medium">"Activo"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|r| {
                                            let id = r.id.clone();
                                            let ty = if r.resource_type_label.is_empty() {
                                                type_label(&r.resource_type).to_string()
                                            } else {
                                                r.resource_type_label.clone()
                                            };
                                            let n_status = r.allowed_status_ids.len();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{r.name}</td>
                                                    <td class="px-4 py-2">{ty}</td>
                                                    <td class="px-4 py-2">{money(r.hourly_cost)}</td>
                                                    <td class="px-4 py-2">{r.currency}</td>
                                                    <td class="px-4 py-2 text-slate-500">{n_status}</td>
                                                    <td class="px-4 py-2">
                                                        {if r.is_active { "Sí" } else { "No" }}
                                                    </td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let eid = id.clone();
                                                                let did = id.clone();
                                                                view! {
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
