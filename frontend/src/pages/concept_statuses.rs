use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ApiError, ConceptStatusFull, ConceptStatusPayload, Me};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input};

#[component]
pub fn ConceptStatusesPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<ConceptStatusFull>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(
                api::get_json::<Vec<ConceptStatusFull>>("/api/admin/concept_statuses").await,
            ));
        });
    };
    reload();

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let position = RwSignal::new("0".to_string());
    let color = RwSignal::new(String::new());
    let is_initial = RwSignal::new(false);
    let is_terminal = RwSignal::new(false);
    let is_cancelled = RwSignal::new(false);
    let is_active = RwSignal::new(true);
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        position.set("0".to_string());
        color.set(String::new());
        is_initial.set(false);
        is_terminal.set(false);
        is_cancelled.set(false);
        is_active.set(true);
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let color_val = color.get_untracked();
        let payload = ConceptStatusPayload {
            name: name.get_untracked().trim().to_string(),
            position: position.get_untracked().trim().parse().unwrap_or(0),
            color: (!color_val.trim().is_empty()).then_some(color_val),
            is_initial: is_initial.get_untracked(),
            is_terminal: is_terminal.get_untracked(),
            is_cancelled: is_cancelled.get_untracked(),
            is_active: is_active.get_untracked(),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/concept_statuses/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/concept_statuses", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el estado"))),
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
    let begin_edit = move |s: ConceptStatusFull| {
        name.set(s.name);
        position.set(s.position.to_string());
        color.set(s.color.unwrap_or_default());
        is_initial.set(s.is_initial);
        is_terminal.set(s.is_terminal);
        is_cancelled.set(s.is_cancelled);
        is_active.set(s.is_active);
        editing.set(Some(s.id));
        form_error.set(None);
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/concept_statuses/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Estados de concepto"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-3xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar estado" } else { "Nuevo estado" }
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
                                        <label class="block text-sm font-medium text-slate-700">"Posición"</label>
                                        <Input
                                            value=position
                                            on_input=Callback::new(move |v| position.set(v))
                                            inputmode="numeric"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Color (opcional)"
                                        </label>
                                        <Input value=color on_input=Callback::new(move |v| color.set(v)) />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_initial label="Inicial" />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_terminal label="Terminal" />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_cancelled label="Cancelado" />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_active label="Activo" />
                                    </div>
                                    <div class="flex items-end gap-2 sm:col-span-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear estado"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los estados."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin estados todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Posición"</th>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Inicial"</th>
                                        <th class="px-4 py-2 font-medium">"Terminal"</th>
                                        <th class="px-4 py-2 font-medium">"Activo"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|s| {
                                            let row = s.clone();
                                            let did = s.id.clone();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2 text-slate-500">{s.position}</td>
                                                    <td class="px-4 py-2">{s.name}</td>
                                                    <td class="px-4 py-2">{if s.is_initial { "Sí" } else { "" }}</td>
                                                    <td class="px-4 py-2">{if s.is_terminal { "Sí" } else { "" }}</td>
                                                    <td class="px-4 py-2">{if s.is_active { "Sí" } else { "No" }}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let row = row.clone();
                                                                let did = did.clone();
                                                                view! {
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        on:click=move |_| begin_edit(row.clone())
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
