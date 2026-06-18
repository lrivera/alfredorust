use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{flow_label, load_category_options, Options};
use crate::api::{self, ApiError, Category, CategoryPayload, Me};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

#[component]
pub fn CategoriesPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Category>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<Category>>("/api/admin/categories").await));
        });
    };
    reload();

    // Options for the parent-category picker.
    let cat_opts = RwSignal::new(Options::new());
    load_category_options(cat_opts);

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let flow = RwSignal::new("expense".to_string());
    let parent = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        flow.set("expense".to_string());
        parent.set(String::new());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let notes_val = notes.get_untracked();
        let parent_val = parent.get_untracked();
        let payload = CategoryPayload {
            name: name.get_untracked().trim().to_string(),
            flow_type: flow.get_untracked(),
            parent_id: (!parent_val.is_empty()).then_some(parent_val),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/categories/{id}/update"), &payload).await
                }
                None => api::post_json("/api/admin/categories", &payload).await,
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
                Err(ApiError::Forbidden) => form_error.set(Some("No tienes permiso".into())),
                Err(_) => form_error.set(Some("No se pudo guardar la categoría".into())),
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
                api::get_json::<api::CategoryDetail>(&format!("/api/admin/categories/{id}")).await
            {
                name.set(d.name);
                flow.set(d.flow_type);
                parent.set(d.parent_id.unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/categories/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Categorías"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-2xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar categoría"
                                        } else {
                                            "Nueva categoría"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-2">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Nombre"
                                        </label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            placeholder="Servicios"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Flujo"
                                        </label>
                                        <Select value=flow>
                                            <option value="income">"Ingreso"</option>
                                            <option value="expense">"Egreso"</option>
                                        </Select>
                                    </div>
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Categoría padre (opcional)"
                                        </label>
                                        <Select value=parent>
                                            <option value="">"— Ninguna —"</option>
                                            {move || {
                                                cat_opts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| {
                                                        view! { <option value=id>{label}</option> }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear categoría"
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
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}

            {move || match items.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las categorías."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin categorías todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Flujo"</th>
                                        <th class="px-4 py-2 font-medium">"Padre"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|c| {
                                            let id = c.id.clone();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{c.name}</td>
                                                    <td class="px-4 py-2">
                                                        {flow_label(&c.flow_type).to_string()}
                                                    </td>
                                                    <td class="px-4 py-2 text-slate-500">{c.parent}</td>
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
