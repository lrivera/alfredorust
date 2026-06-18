//! SAT fiscal configs for the active company: list stored certificates, upload
//! a new one (.cer + .key + password via multipart), and delete. Editing the
//! stored certificate files isn't exposed here — re-upload instead, matching v1.

use leptos::prelude::*;
use leptos::task::spawn_local;
use web_sys::File;

use crate::api::{self, ApiError, SatConfigData};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input};

#[component]
pub fn SatConfigsPage() -> impl IntoView {
    let items = RwSignal::new(None::<Result<Vec<SatConfigData>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<SatConfigData>>("/api/admin/sat-configs").await));
        });
    };
    reload();

    let label = RwSignal::new(String::new());
    let rfc = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let cer_ref = NodeRef::<leptos::html::Input>::new();
    let key_ref = NodeRef::<leptos::html::Input>::new();

    let reset_form = move || {
        label.set(String::new());
        rfc.set(String::new());
        password.set(String::new());
        if let Some(el) = cer_ref.get_untracked() {
            el.set_value("");
        }
        if let Some(el) = key_ref.get_untracked() {
            el.set_value("");
        }
    };

    // (rfc, label, password, cer_file, key_file)
    let upload = Action::new_local(move |args: &(String, String, String, File, File)| {
        let (rfc, label, password, cer, key) = args.clone();
        async move { api::upload_sat_config(&rfc, &label, &password, &cer, &key).await }
    });

    Effect::new(move |_| {
        if let Some(r) = upload.value().get() {
            match r {
                Ok(()) => {
                    reset_form();
                    form_error.set(None);
                    reload();
                }
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo subir la configuración"))),
            }
        }
    });

    let pending = upload.pending();
    let first_file = |node: NodeRef<leptos::html::Input>| -> Option<File> {
        node.get_untracked()
            .and_then(|el| el.files())
            .and_then(|list| list.get(0))
    };
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        let (Some(cer), Some(key)) = (first_file(cer_ref), first_file(key_ref)) else {
            form_error.set(Some("Selecciona los archivos .cer y .key".into()));
            return;
        };
        if rfc.get().trim().is_empty() || password.get().trim().is_empty() {
            form_error.set(Some("RFC, contraseña y ambos archivos son obligatorios".into()));
            return;
        }
        form_error.set(None);
        upload.dispatch((
            rfc.get().trim().to_string(),
            label.get().trim().to_string(),
            password.get(),
            cer,
            key,
        ));
    };

    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/sat-configs/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    let file_class =
        "block w-full text-sm text-foreground file:mr-3 file:rounded-md file:border-0 \
         file:bg-muted file:px-3 file:py-1.5 file:text-sm file:font-medium";

    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-xl font-semibold">"Configuraciones SAT"</h1>
                <p class="text-sm text-muted-foreground">
                    "Firmas (CSD) de la compañía actual para timbrado y descarga de CFDIs."
                </p>
            </div>

            <Card class="max-w-2xl">
                <CardHeader>
                    <CardTitle>"Agregar configuración"</CardTitle>
                </CardHeader>
                <CardContent>
                    <form on:submit=submit class="grid gap-3 sm:grid-cols-2">
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">"Etiqueta"</label>
                            <Input
                                value=label
                                on_input=Callback::new(move |v| label.set(v))
                                placeholder="Ej. Principal, Persona Moral…"
                            />
                            <p class="text-xs text-muted-foreground">
                                "Útil para identificar esta firma cuando hay varias."
                            </p>
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">"RFC"</label>
                            <Input
                                value=rfc
                                on_input=Callback::new(move |v| rfc.set(v))
                                class="uppercase"
                                placeholder="XAXX010101000"
                                required=true
                            />
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">
                                "Certificado (.cer)"
                            </label>
                            <input type="file" accept=".cer" node_ref=cer_ref class=file_class />
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">
                                "Llave privada (.key)"
                            </label>
                            <input type="file" accept=".key" node_ref=key_ref class=file_class />
                        </div>
                        <div class="space-y-1 sm:col-span-2">
                            <label class="block text-sm font-medium text-foreground">
                                "Contraseña de la llave privada"
                            </label>
                            <Input
                                value=password
                                on_input=Callback::new(move |v| password.set(v))
                                r#type="password"
                                required=true
                            />
                        </div>
                        <div class="flex items-end gap-2 sm:col-span-2">
                            <Button r#type="submit" disabled=pending>
                                {move || {
                                    if pending.get() { "Subiendo…" } else { "Guardar configuración" }
                                }}
                            </Button>
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

            {move || match items.get() {
                None => view! { <p class="text-muted-foreground">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las configuraciones."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-muted-foreground">"Sin configuraciones todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-border bg-card">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-muted text-muted-foreground">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"RFC"</th>
                                        <th class="px-4 py-2 font-medium">"Etiqueta"</th>
                                        <th class="px-4 py-2 font-medium">"Creada"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|s| {
                                            let did = s.id.clone();
                                            view! {
                                                <tr class="border-t border-border">
                                                    <td class="px-4 py-2 font-medium">{s.rfc}</td>
                                                    <td class="px-4 py-2 text-muted-foreground">
                                                        {s.label.unwrap_or_default()}
                                                    </td>
                                                    <td class="px-4 py-2 text-muted-foreground">
                                                        {s.created_at.get(..10).unwrap_or("").to_string()}
                                                    </td>
                                                    <td class="px-4 py-2 text-right">
                                                        <Button
                                                            variant=ButtonVariant::Ghost
                                                            class="text-red-600 hover:bg-red-50"
                                                            on:click=move |_| delete_one(did.clone())
                                                        >
                                                            "Eliminar"
                                                        </Button>
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
