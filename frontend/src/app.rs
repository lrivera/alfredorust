//! App shell: bootstrap the session, then render either the login view or the
//! authenticated shell. Client-side routing per screen (leptos_router) lands in
//! the per-category migration phases; the login slice uses auth-state switching.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, Me};

#[derive(Clone)]
enum Auth {
    Loading,
    Anon,
    Authed(Me),
}

#[component]
pub fn App() -> impl IntoView {
    let auth = RwSignal::new(Auth::Loading);

    // Single bootstrap call. 401 -> anonymous (show login).
    spawn_local(async move {
        match api::get_me().await {
            Ok(me) => auth.set(Auth::Authed(me)),
            Err(_) => auth.set(Auth::Anon),
        }
    });

    view! {
        <div class="min-h-screen bg-slate-50 text-slate-900">
            {move || match auth.get() {
                Auth::Loading => view! {
                    <p class="p-8 text-slate-500">"Cargando…"</p>
                }
                .into_any(),
                Auth::Anon => view! { <LoginView auth=auth /> }.into_any(),
                Auth::Authed(me) => view! { <Shell me=me auth=auth /> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn LoginView(auth: RwSignal<Auth>) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let pending = RwSignal::new(false);

    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        let (e, c) = (email.get(), code.get());
        pending.set(true);
        error.set(None);
        spawn_local(async move {
            match api::login(&e, &c).await {
                Ok(ok) => {
                    if let Some(url) = ok.redirect_url {
                        // Full navigation to the tenant subdomain re-bootstraps
                        // the SPA under the correct host.
                        let _ = window().location().set_href(&url);
                    } else {
                        match api::get_me().await {
                            Ok(me) => auth.set(Auth::Authed(me)),
                            Err(_) => error.set(Some("No se pudo cargar el perfil".into())),
                        }
                    }
                }
                Err(api::ApiError::Unauthorized) => {
                    // Generic message — never reveal whether the email exists.
                    error.set(Some("Correo o código inválido".into()))
                }
                Err(_) => error.set(Some("Error de autenticación. Intenta de nuevo.".into())),
            }
            pending.set(false);
        });
    };

    view! {
        <div class="flex min-h-screen items-center justify-center p-4">
            <form
                on:submit=submit
                class="w-full max-w-sm space-y-4 rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
            >
                <h1 class="text-xl font-semibold">"Iniciar sesión"</h1>

                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Correo"</label>
                    <input
                        type="email"
                        autocomplete="email"
                        required
                        class="w-full rounded-md border border-slate-300 px-3 py-2 outline-none focus:border-slate-500"
                        prop:value=move || email.get()
                        on:input=move |ev| email.set(event_target_value(&ev))
                    />
                </div>

                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Código (6 dígitos)"</label>
                    <input
                        type="text"
                        inputmode="numeric"
                        maxlength="6"
                        required
                        class="w-full rounded-md border border-slate-300 px-3 py-2 tracking-widest outline-none focus:border-slate-500"
                        prop:value=move || code.get()
                        on:input=move |ev| code.set(event_target_value(&ev))
                    />
                </div>

                {move || {
                    error
                        .get()
                        .map(|msg| view! { <p class="text-sm text-red-600">{msg}</p> })
                }}

                <button
                    type="submit"
                    disabled=move || pending.get()
                    class="w-full rounded-md bg-slate-900 px-3 py-2 font-medium text-white hover:bg-slate-700 disabled:opacity-50"
                >
                    {move || if pending.get() { "Entrando…" } else { "Entrar" }}
                </button>
            </form>
        </div>
    }
}

#[component]
fn Shell(me: Me, auth: RwSignal<Auth>) -> impl IntoView {
    let do_logout = move |_| {
        spawn_local(async move {
            let _ = api::logout().await;
            auth.set(Auth::Anon);
        });
    };

    let companies = me.companies.clone();
    let email = me.email.clone();
    let company = me.company.clone();
    let role = me.role.clone();

    view! {
        <header class="flex items-center justify-between border-b border-slate-200 bg-white px-6 py-3">
            <div>
                <p class="text-sm text-slate-500">{email}</p>
                <p class="font-semibold">{company} " · " <span class="text-slate-500">{role}</span></p>
            </div>
            <button
                on:click=do_logout
                class="rounded-md border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-100"
            >
                "Salir"
            </button>
        </header>

        <main class="p-6">
            <h2 class="mb-3 text-lg font-semibold">"Compañías"</h2>
            <ul class="space-y-1">
                {companies
                    .into_iter()
                    .map(|c| {
                        let href = switch_company_href(&c.slug);
                        view! {
                            <li>
                                <a
                                    href=href
                                    class=if c.active {
                                        "font-semibold text-slate-900"
                                    } else {
                                        "text-slate-600 hover:underline"
                                    }
                                >
                                    {c.name}
                                    {if c.active { " (activa)" } else { "" }}
                                </a>
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()}
            </ul>
        </main>
    }
}

/// Build the URL for another tenant by swapping the leftmost host label for the
/// company slug, preserving protocol and port.
fn switch_company_href(slug: &str) -> String {
    let loc = window().location();
    let proto = loc.protocol().unwrap_or_else(|_| "https:".to_string());
    let host = loc.host().unwrap_or_default();
    let rest = host.split_once('.').map(|(_, r)| r).unwrap_or(&host);
    format!("{proto}//{slug}.{rest}")
}
