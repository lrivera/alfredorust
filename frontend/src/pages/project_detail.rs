use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use super::{load_concept_status_options, money, Options};
use crate::api::{self, ApiError, Me, ProjectConcept, ProjectConceptPayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

#[component]
pub fn ProjectDetailPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let params = use_params_map();
    let pid = move || params.read().get("id").unwrap_or_default();

    let title = RwSignal::new(String::new());
    let concepts = RwSignal::new(None::<Result<Vec<ProjectConcept>, ApiError>>);
    let statuses = RwSignal::new(Options::new());
    load_concept_status_options(statuses);

    let reload = move || {
        let id = pid();
        concepts.set(None);
        spawn_local(async move {
            if let Ok(p) = api::get_json::<api::ProjectDetail>(&format!("/api/admin/projects/{id}")).await {
                title.set(p.title);
            }
            concepts.set(Some(
                api::get_json::<Vec<ProjectConcept>>(&format!("/api/admin/projects/{id}/concepts")).await,
            ));
        });
    };
    reload();

    // --- add/edit concept form -------------------------------------------
    let editing = RwSignal::new(None::<String>);
    let status = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let quantity = RwSignal::new("1".to_string());
    let unit = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let est_hours = RwSignal::new(String::new());
    let est_cost = RwSignal::new(String::new());
    let position = RwSignal::new("0".to_string());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        status.set(String::new());
        name.set(String::new());
        quantity.set("1".to_string());
        unit.set(String::new());
        description.set(String::new());
        est_hours.set(String::new());
        est_cost.set(String::new());
        position.set("0".to_string());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let opt = |s: String| (!s.trim().is_empty()).then_some(s);
        let num = |s: String| s.trim().parse::<f64>().ok();
        let id = pid();
        let status_val = status.get_untracked();
        let payload = ProjectConceptPayload {
            status_id: (!status_val.is_empty()).then_some(status_val),
            name: name.get_untracked().trim().to_string(),
            quantity: quantity.get_untracked().trim().parse().unwrap_or(0.0),
            unit: opt(unit.get_untracked()),
            description: opt(description.get_untracked()),
            estimated_hours: num(est_hours.get_untracked()),
            estimated_cost: num(est_cost.get_untracked()),
            notes: None,
            position: position.get_untracked().trim().parse().unwrap_or(0),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(cid) => api::post_json(&format!("/api/admin/project_concepts/{cid}/update"), &payload).await,
                None => api::post_json(&format!("/api/admin/projects/{id}/concepts"), &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el concepto"))),
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
    let begin_edit = move |c: ProjectConcept| {
        status.set(c.status_id);
        name.set(c.name);
        quantity.set(c.quantity.to_string());
        unit.set(c.unit.unwrap_or_default());
        description.set(c.description.unwrap_or_default());
        est_hours.set(c.estimated_hours.map(|v| v.to_string()).unwrap_or_default());
        est_cost.set(c.estimated_cost.map(|v| v.to_string()).unwrap_or_default());
        position.set(c.position.to_string());
        editing.set(Some(c.id));
        form_error.set(None);
    };
    let advance = move |cid: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/project_concepts/{cid}/advance")).await.is_ok() {
                reload();
            }
        });
    };
    let delete_one = move |cid: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/project_concepts/{cid}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    let status_name = move |sid: &str| {
        statuses.get().into_iter().find(|(id, _)| id == sid).map(|(_, n)| n).unwrap_or_default()
    };

    view! {
        <div class="space-y-6">
            <a href="/v2/projects" class="text-sm text-muted-foreground hover:underline">"← Proyectos"</a>
            <h1 class="text-xl font-semibold">{move || title.get()}</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar concepto" } else { "Agregar concepto" }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-foreground">"Concepto"</label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">"Estado"</label>
                                        <Select value=status>
                                            <option value="">"— Inicial —"</option>
                                            {move || {
                                                statuses
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">"Cantidad"</label>
                                        <Input
                                            value=quantity
                                            on_input=Callback::new(move |v| quantity.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">"Unidad"</label>
                                        <Input value=unit on_input=Callback::new(move |v| unit.set(v)) />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">"Orden"</label>
                                        <Input
                                            value=position
                                            on_input=Callback::new(move |v| position.set(v))
                                            inputmode="numeric"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Horas estimadas"
                                        </label>
                                        <Input
                                            value=est_hours
                                            on_input=Callback::new(move |v| est_hours.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Costo estimado"
                                        </label>
                                        <Input
                                            value=est_cost
                                            on_input=Callback::new(move |v| est_cost.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Descripción"
                                        </label>
                                        <Input
                                            value=description
                                            on_input=Callback::new(move |v| description.set(v))
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
                                                    "Agregar concepto"
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

            {move || match concepts.get() {
                None => view! { <p class="text-muted-foreground">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar los conceptos."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-muted-foreground">"Sin conceptos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-border bg-card">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-muted text-muted-foreground">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Orden"</th>
                                        <th class="px-4 py-2 font-medium">"Concepto"</th>
                                        <th class="px-4 py-2 font-medium">"Cantidad"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2 font-medium">"Estimado (h)"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|c| {
                                            let row = c.clone();
                                            let aid = c.id.clone();
                                            let did = c.id.clone();
                                            let sid = c.status_id.clone();
                                            let qty = format!(
                                                "{} {}",
                                                money(c.quantity),
                                                c.unit.clone().unwrap_or_default(),
                                            );
                                            let hrs = c.estimated_hours.map(|h| format!("{h:.1}")).unwrap_or_default();
                                            view! {
                                                <tr class="border-t border-border">
                                                    <td class="px-4 py-2 text-muted-foreground">{c.position}</td>
                                                    <td class="px-4 py-2">{c.name}</td>
                                                    <td class="px-4 py-2">{qty}</td>
                                                    <td class="px-4 py-2">{move || status_name(&sid)}</td>
                                                    <td class="px-4 py-2">{hrs}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let row = row.clone();
                                                                let aid = aid.clone();
                                                                let did = did.clone();
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
