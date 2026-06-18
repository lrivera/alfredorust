use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    load_project_options, load_resource_options, local_dt_to_rfc3339, rfc3339_to_local_dt, Options,
};
use crate::api::{self, ApiError, Me, ResourceLog, ResourceLogPayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

#[component]
pub fn ResourceLogsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<ResourceLog>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<ResourceLog>>("/api/admin/resource_logs").await));
        });
    };
    reload();

    let projects = RwSignal::new(Options::new());
    let resources = RwSignal::new(Options::new());
    load_project_options(projects);
    load_resource_options(resources);

    let editing = RwSignal::new(None::<String>);
    let project = RwSignal::new(String::new());
    let resource = RwSignal::new(String::new());
    let phase = RwSignal::new(String::new());
    let started = RwSignal::new(String::new());
    let ended = RwSignal::new(String::new());
    let operator = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        project.set(String::new());
        resource.set(String::new());
        phase.set(String::new());
        started.set(String::new());
        ended.set(String::new());
        operator.set(String::new());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let opt = |s: String| (!s.trim().is_empty()).then_some(s);
        let ended_val = ended.get_untracked();
        let payload = ResourceLogPayload {
            project_id: opt(project.get_untracked()),
            phase: opt(phase.get_untracked()),
            resource_id: opt(resource.get_untracked()),
            started_at: local_dt_to_rfc3339(&started.get_untracked()),
            ended_at: (!ended_val.trim().is_empty()).then(|| local_dt_to_rfc3339(&ended_val)),
            operator_name: opt(operator.get_untracked()),
            notes: opt(notes.get_untracked()),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/resource_logs/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/resource_logs", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el registro"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if started.get().trim().is_empty() {
            form_error.set(Some("El inicio es obligatorio".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::ResourceLogDetail>(&format!("/api/admin/resource_logs/{id}")).await
            {
                project.set(d.project_id.unwrap_or_default());
                resource.set(d.resource_id.unwrap_or_default());
                phase.set(d.phase.unwrap_or_default());
                started.set(rfc3339_to_local_dt(&d.started_at));
                ended.set(d.ended_at.as_deref().map(rfc3339_to_local_dt).unwrap_or_default());
                operator.set(d.operator_name.unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/resource_logs/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };
    let end_now = move |id: String| {
        spawn_local(async move {
            // Empty body: the server stamps `ended_at` to now.
            let payload = api::ResourceLogEndPayload { ended_at: None };
            if api::post_json(&format!("/api/admin/resource_logs/{id}/end"), &payload).await.is_ok() {
                reload();
            }
        });
    };

    let project_name = move |pid: &Option<String>| {
        pid.as_ref()
            .and_then(|id| projects.get().into_iter().find(|(p, _)| p == id).map(|(_, n)| n))
            .unwrap_or_default()
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Registros de recursos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar registro" } else { "Nuevo registro" }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Proyecto (opcional)"
                                        </label>
                                        <Select value=project>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                projects
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Recurso (opcional)"
                                        </label>
                                        <Select value=resource>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                resources
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Fase"</label>
                                        <Input value=phase on_input=Callback::new(move |v| phase.set(v)) />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Inicio"</label>
                                        <Input
                                            value=started
                                            on_input=Callback::new(move |v| started.set(v))
                                            r#type="datetime-local"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Fin (opcional)"
                                        </label>
                                        <Input
                                            value=ended
                                            on_input=Callback::new(move |v| ended.set(v))
                                            r#type="datetime-local"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">"Operador"</label>
                                        <Input
                                            value=operator
                                            on_input=Callback::new(move |v| operator.set(v))
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
                                                    "Crear registro"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los registros."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin registros todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Proyecto"</th>
                                        <th class="px-4 py-2 font-medium">"Fase"</th>
                                        <th class="px-4 py-2 font-medium">"Recurso"</th>
                                        <th class="px-4 py-2 font-medium">"Inicio"</th>
                                        <th class="px-4 py-2 font-medium">"Fin"</th>
                                        <th class="px-4 py-2 font-medium">"Horas"</th>
                                        <th class="px-4 py-2 font-medium">"Operador"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|l| {
                                            let id = l.id.clone();
                                            let pid = l.project_id.clone();
                                            let open = l.ended_at.is_none();
                                            let started = rfc3339_to_local_dt(&l.started_at);
                                            let ended = l.ended_at.as_deref().map(rfc3339_to_local_dt).unwrap_or_default();
                                            let hours = l.duration_hours.map(|h| format!("{h:.2}")).unwrap_or_default();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2 text-slate-500">
                                                        {move || project_name(&pid)}
                                                    </td>
                                                    <td class="px-4 py-2">{l.phase.unwrap_or_default()}</td>
                                                    <td class="px-4 py-2">{l.resource_name.unwrap_or_default()}</td>
                                                    <td class="px-4 py-2 text-slate-500">{started}</td>
                                                    <td class="px-4 py-2 text-slate-500">{ended}</td>
                                                    <td class="px-4 py-2">{hours}</td>
                                                    <td class="px-4 py-2">{l.operator_name.unwrap_or_default()}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let endid = id.clone();
                                                                let eid = id.clone();
                                                                let did = id.clone();
                                                                view! {
                                                                    {open
                                                                        .then(|| {
                                                                            view! {
                                                                                <Button
                                                                                    variant=ButtonVariant::Ghost
                                                                                    class="text-emerald-700 hover:bg-emerald-50"
                                                                                    on:click=move |_| end_now(endid.clone())
                                                                                >
                                                                                    "Terminar"
                                                                                </Button>
                                                                            }
                                                                        })}
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
