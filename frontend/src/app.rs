//! App shell: bootstrap the session, then render either the login view or the
//! authenticated app (client-side routed via leptos_router).

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes, A};
use leptos_router::path;

use crate::api::{self, ApiError, Me};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input};
use crate::pages::{
    AccountsPage, CategoriesPage, CfdiPage, ConceptStatusesPage, ContactsPage, Dashboard,
    ForecastsPage, OrdersPage, PlannedEntriesPage, ProjectDetailPage, ProjectsPage,
    RecurringPlansPage, ResourceLogsPage, ResourceUsagesPage, ResourcesPage, TiempoPage,
    TransactionsPage,
};

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
                Auth::Loading => {
                    view! { <p class="p-8 text-slate-500">"Cargando…"</p> }.into_any()
                }
                Auth::Anon => view! { <LoginView auth=auth /> }.into_any(),
                Auth::Authed(me) => view! { <AuthedApp me=me auth=auth /> }.into_any(),
            }}
        </div>
    }
}

/// What the login action resolves to. Kept as one outcome type so the action
/// owns the whole login→bootstrap sequence and the UI just reacts to it.
#[derive(Clone)]
enum LoginOutcome {
    Redirect(String),
    Authed(Me),
    BadCreds,
    Error(String),
}

#[component]
fn LoginView(auth: RwSignal<Auth>) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);

    // Action drives the async login + bootstrap; gives us pending() for free.
    // `new_local` because the web fetch future (JsFuture) is not `Send`.
    let login_action = Action::new_local(move |input: &(String, String)| {
        let (email, code) = input.clone();
        async move {
            match api::login(&email, &code).await {
                Ok(ok) => match ok.redirect_url {
                    // Full navigation to the tenant subdomain re-bootstraps the
                    // SPA under the correct host.
                    Some(url) => LoginOutcome::Redirect(url),
                    None => match api::get_me().await {
                        Ok(me) => LoginOutcome::Authed(me),
                        Err(_) => LoginOutcome::Error("No se pudo cargar el perfil".into()),
                    },
                },
                // Generic message — never reveal whether the email exists.
                Err(ApiError::Unauthorized) => LoginOutcome::BadCreds,
                Err(_) => LoginOutcome::Error("Error de autenticación. Intenta de nuevo.".into()),
            }
        }
    });

    let pending = login_action.pending();

    Effect::new(move |_| {
        match login_action.value().get() {
            Some(LoginOutcome::Redirect(url)) => {
                let _ = window().location().set_href(&url);
            }
            Some(LoginOutcome::Authed(me)) => auth.set(Auth::Authed(me)),
            Some(LoginOutcome::BadCreds) => error.set(Some("Correo o código inválido".into())),
            Some(LoginOutcome::Error(msg)) => error.set(Some(msg)),
            None => {}
        }
    });

    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        error.set(None);
        login_action.dispatch((email.get(), code.get()));
    };

    view! {
        <div class="flex min-h-screen items-center justify-center p-4">
            <Card class="w-full max-w-sm">
                <CardHeader>
                    <CardTitle>"Iniciar sesión"</CardTitle>
                </CardHeader>
                <CardContent>
                    <form on:submit=submit class="space-y-4">
                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-slate-700">"Correo"</label>
                            <Input
                                value=email
                                on_input=Callback::new(move |v| email.set(v))
                                r#type="email"
                                autocomplete="email"
                                placeholder="tu@correo.com"
                                required=true
                            />
                        </div>

                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-slate-700">
                                "Código (6 dígitos)"
                            </label>
                            <Input
                                value=code
                                on_input=Callback::new(move |v| code.set(v))
                                inputmode="numeric"
                                maxlength="6"
                                class="tracking-widest"
                                required=true
                            />
                        </div>

                        {move || {
                            error
                                .get()
                                .map(|msg| view! { <p class="text-sm text-red-600">{msg}</p> })
                        }}

                        <Button r#type="submit" disabled=pending class="w-full">
                            {move || if pending.get() { "Entrando…" } else { "Entrar" }}
                        </Button>
                    </form>
                </CardContent>
            </Card>
        </div>
    }
}

/// Authenticated, client-side routed app: provides `Me`/`auth` via context and
/// renders the persistent layout (sidebar + topbar) around routed pages.
#[component]
fn AuthedApp(me: Me, auth: RwSignal<Auth>) -> impl IntoView {
    provide_context(me);
    provide_context(auth);

    view! {
        <Router base="/v2">
            <div class="flex min-h-screen">
                <Sidebar />
                <div class="flex flex-1 flex-col">
                    <Topbar />
                    <main class="flex-1 p-6">
                        <Routes fallback=|| view! { <p class="text-slate-500">"No encontrado"</p> }>
                            <Route path=path!("/") view=Dashboard />
                            <Route path=path!("/accounts") view=AccountsPage />
                            <Route path=path!("/categories") view=CategoriesPage />
                            <Route path=path!("/contacts") view=ContactsPage />
                            <Route path=path!("/transactions") view=TransactionsPage />
                            <Route path=path!("/recurring-plans") view=RecurringPlansPage />
                            <Route path=path!("/planned-entries") view=PlannedEntriesPage />
                            <Route path=path!("/forecasts") view=ForecastsPage />
                            <Route path=path!("/orders") view=OrdersPage />
                            <Route path=path!("/projects") view=ProjectsPage />
                            <Route path=path!("/projects/:id") view=ProjectDetailPage />
                            <Route path=path!("/concept-statuses") view=ConceptStatusesPage />
                            <Route path=path!("/resources") view=ResourcesPage />
                            <Route path=path!("/resource-logs") view=ResourceLogsPage />
                            <Route path=path!("/resource-usages") view=ResourceUsagesPage />
                            <Route path=path!("/tiempo") view=TiempoPage />
                            <Route path=path!("/cfdi") view=CfdiPage />
                        </Routes>
                    </main>
                </div>
            </div>
        </Router>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";
    let link = "block rounded-md px-3 py-2 text-sm text-slate-700 hover:bg-slate-100 \
        aria-[current=page]:bg-blue-50 aria-[current=page]:text-blue-700 \
        aria-[current=page]:font-semibold";

    view! {
        <aside class="w-56 shrink-0 border-r border-slate-200 bg-white p-3">
            <p class="px-3 pb-3 text-sm font-semibold text-slate-900">"alfredodev"</p>
            <nav class="space-y-1">
                // Absolute hrefs include the /v2 prefix: leptos_router passes
                // absolute links through unchanged (base only strips for route
                // matching), so the prefix must be explicit here.
                <A href="/v2/" attr:class=link>
                    "Inicio"
                </A>
                <A href="/v2/tiempo" attr:class=link>
                    "Tiempo"
                </A>
                {move || {
                    if is_admin {
                        view! {
                            <p class="px-3 pt-3 pb-1 text-xs font-semibold uppercase text-slate-400">
                                "Finanzas"
                            </p>
                            <A href="/v2/accounts" attr:class=link>
                                "Cuentas"
                            </A>
                            <A href="/v2/categories" attr:class=link>
                                "Categorías"
                            </A>
                            <A href="/v2/contacts" attr:class=link>
                                "Contactos"
                            </A>
                            <A href="/v2/transactions" attr:class=link>
                                "Movimientos"
                            </A>
                            <A href="/v2/recurring-plans" attr:class=link>
                                "Planes recurrentes"
                            </A>
                            <A href="/v2/planned-entries" attr:class=link>
                                "Entradas planificadas"
                            </A>
                            <A href="/v2/forecasts" attr:class=link>
                                "Pronósticos"
                            </A>
                            <p class="px-3 pt-3 pb-1 text-xs font-semibold uppercase text-slate-400">
                                "Operaciones"
                            </p>
                            <A href="/v2/orders" attr:class=link>
                                "Órdenes"
                            </A>
                            <A href="/v2/projects" attr:class=link>
                                "Proyectos"
                            </A>
                            <A href="/v2/concept-statuses" attr:class=link>
                                "Estados de concepto"
                            </A>
                            <A href="/v2/resources" attr:class=link>
                                "Recursos"
                            </A>
                            <A href="/v2/resource-logs" attr:class=link>
                                "Registros de recursos"
                            </A>
                            <A href="/v2/resource-usages" attr:class=link>
                                "Uso de recursos (grid)"
                            </A>
                            <p class="px-3 pt-3 pb-1 text-xs font-semibold uppercase text-slate-400">
                                "Fiscal"
                            </p>
                            <A href="/v2/cfdi" attr:class=link>
                                "CFDIs"
                            </A>
                        }
                            .into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </nav>
        </aside>
    }
}

#[component]
fn Topbar() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let auth = use_context::<RwSignal<Auth>>().expect("auth context");

    let do_logout = move |_| {
        spawn_local(async move {
            let _ = api::logout().await;
            auth.set(Auth::Anon);
        });
    };

    view! {
        <header class="flex items-center justify-between border-b border-slate-200 bg-white px-6 py-3">
            <div>
                <p class="text-sm text-slate-500">{me.email.clone()}</p>
                <p class="font-semibold">
                    {me.company.clone()} " · " <span class="text-slate-500">{me.role.clone()}</span>
                </p>
            </div>
            <Button variant=ButtonVariant::Outline on:click=do_logout>
                "Salir"
            </Button>
        </header>
    }
}
