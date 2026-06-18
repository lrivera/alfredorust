//! The logged-in user's own profile: edit email and TOTP secret. Available to
//! every authenticated user regardless of role (it's their own account).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ProfileData, ProfilePayload};
use crate::components::{Button, Card, CardContent, CardHeader, CardTitle, Input};

#[component]
pub fn AccountPage() -> impl IntoView {
    let email = RwSignal::new(String::new());
    let secret = RwSignal::new(String::new());
    let message = RwSignal::new(None::<String>);
    let form_error = RwSignal::new(None::<String>);

    spawn_local(async move {
        if let Ok(d) = api::get_json::<ProfileData>("/api/account").await {
            email.set(d.username);
        }
    });

    let save = Action::new_local(move |_: &()| {
        let payload = ProfilePayload {
            username: email.get_untracked().trim().to_string(),
            secret: secret.get_untracked().trim().to_string(),
        };
        async move { api::post_json("/api/account", &payload).await }
    });

    Effect::new(move |_| {
        if let Some(r) = save.value().get() {
            match r {
                Ok(()) => {
                    message.set(Some("Tu información se guardó correctamente".into()));
                    form_error.set(None);
                }
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar la información"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        message.set(None);
        if email.get().trim().is_empty() {
            form_error.set(Some("El nombre de usuario es obligatorio".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };

    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-xl font-semibold">"Mi cuenta"</h1>
                <p class="text-sm text-muted-foreground">"Actualiza los datos asociados a tu usuario."</p>
            </div>

            <Card class="max-w-lg">
                <CardHeader>
                    <CardTitle>"Perfil"</CardTitle>
                </CardHeader>
                <CardContent>
                    <form on:submit=submit class="space-y-4">
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">"Usuario"</label>
                            <Input
                                value=email
                                on_input=Callback::new(move |v| email.set(v))
                                autocomplete="username"
                                required=true
                            />
                        </div>
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-foreground">
                                "Secreto TOTP"
                            </label>
                            <Input
                                value=secret
                                on_input=Callback::new(move |v| secret.set(v))
                                class="font-mono"
                                placeholder="Déjalo vacío para conservar el actual"
                            />
                            <p class="text-xs text-muted-foreground">
                                "Escribe un nuevo secreto solo si quieres reconfigurar tu autenticador."
                            </p>
                        </div>

                        {move || {
                            message
                                .get()
                                .map(|m| view! { <p class="text-sm text-green-700">{m}</p> })
                        }}
                        {move || {
                            form_error
                                .get()
                                .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                        }}

                        <div class="flex items-center gap-2">
                            <Button r#type="submit" disabled=pending>
                                {move || if pending.get() { "Guardando…" } else { "Guardar cambios" }}
                            </Button>
                        </div>
                    </form>
                </CardContent>
            </Card>
        </div>
    }
}
