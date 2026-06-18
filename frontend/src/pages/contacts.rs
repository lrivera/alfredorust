use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ApiError, Contact, ContactPayload, Me};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

const CONTACT_TYPES: &[(&str, &str)] = &[
    ("customer", "Cliente"),
    ("supplier", "Proveedor"),
    ("service", "Servicio"),
    ("other", "Otro"),
];

fn opt(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[component]
pub fn ContactsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Contact>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<Contact>>("/api/admin/contacts").await));
        });
    };
    reload();

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let kind = RwSignal::new("customer".to_string());
    let email = RwSignal::new(String::new());
    let rfc = RwSignal::new(String::new());
    let phone = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        kind.set("customer".to_string());
        email.set(String::new());
        rfc.set(String::new());
        phone.set(String::new());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let payload = ContactPayload {
            name: name.get_untracked().trim().to_string(),
            contact_type: kind.get_untracked(),
            rfc: opt(Some(rfc.get_untracked())),
            email: opt(Some(email.get_untracked())),
            phone: opt(Some(phone.get_untracked())),
            notes: opt(Some(notes.get_untracked())),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/contacts/{id}/update"), &payload).await
                }
                None => api::post_json("/api/admin/contacts", &payload).await,
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
                Err(_) => form_error.set(Some("No se pudo guardar el contacto".into())),
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
                api::get_json::<api::ContactDetail>(&format!("/api/admin/contacts/{id}")).await
            {
                name.set(d.name);
                kind.set(d.contact_type);
                email.set(d.email.unwrap_or_default());
                rfc.set(d.rfc.unwrap_or_default());
                phone.set(d.phone.unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/contacts/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Contactos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-2xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar contacto"
                                        } else {
                                            "Nuevo contacto"
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
                                            placeholder="ACME S.A."
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Tipo"
                                        </label>
                                        <Select value=kind>
                                            {CONTACT_TYPES
                                                .iter()
                                                .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Correo"
                                        </label>
                                        <Input
                                            value=email
                                            on_input=Callback::new(move |v| email.set(v))
                                            r#type="email"
                                            placeholder="contacto@acme.com"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "RFC"
                                        </label>
                                        <Input value=rfc on_input=Callback::new(move |v| rfc.set(v)) />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Teléfono"
                                        </label>
                                        <Input
                                            value=phone
                                            on_input=Callback::new(move |v| phone.set(v))
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Notas"
                                        </label>
                                        <Input
                                            value=notes
                                            on_input=Callback::new(move |v| notes.set(v))
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
                                                    "Crear contacto"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los contactos."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin contactos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Tipo"</th>
                                        <th class="px-4 py-2 font-medium">"Correo"</th>
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
                                                    <td class="px-4 py-2">{c.kind}</td>
                                                    <td class="px-4 py-2 text-slate-500">{c.email}</td>
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
