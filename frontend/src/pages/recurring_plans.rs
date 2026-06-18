use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, flow_label, load_account_options, load_category_options, load_contact_options,
    money, rfc3339_to_date, Options,
};
use crate::api::{self, ApiError, Me, RecurringPlan, RecurringPlanPayload};
use crate::components::{
    Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input, Select,
};

#[component]
pub fn RecurringPlansPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<RecurringPlan>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(
                api::get_json::<Vec<RecurringPlan>>("/api/admin/recurring-plans").await,
            ));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let accounts = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);
    let contacts = RwSignal::new(Options::new());
    load_contact_options(contacts);

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let flow = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account = RwSignal::new(String::new());
    let contact = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let frequency = RwSignal::new("monthly".to_string());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let dom = RwSignal::new(String::new());
    let is_active = RwSignal::new(true);
    let notes = RwSignal::new(String::new());
    // Preserved hidden across edits.
    let version = RwSignal::new(1i32);
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        flow.set("expense".to_string());
        category.set(String::new());
        account.set(String::new());
        contact.set(String::new());
        amount.set(String::new());
        frequency.set("monthly".to_string());
        start.set(String::new());
        end.set(String::new());
        dom.set(String::new());
        is_active.set(true);
        notes.set(String::new());
        version.set(1);
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let contact_val = contact.get_untracked();
        let end_val = end.get_untracked();
        let dom_val = dom.get_untracked();
        let notes_val = notes.get_untracked();
        let payload = RecurringPlanPayload {
            name: name.get_untracked().trim().to_string(),
            flow_type: flow.get_untracked(),
            category_id: category.get_untracked(),
            account_expected_id: account.get_untracked(),
            contact_id: (!contact_val.is_empty()).then_some(contact_val),
            amount_estimated: amount.get_untracked().trim().parse().unwrap_or(0.0),
            frequency: frequency.get_untracked(),
            day_of_month: dom_val.trim().parse::<i32>().ok(),
            start_date: date_to_rfc3339(&start.get_untracked()),
            end_date: (!end_val.trim().is_empty()).then(|| date_to_rfc3339(&end_val)),
            is_active: is_active.get_untracked(),
            version: version.get_untracked(),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/recurring-plans/{id}/update"), &payload)
                        .await
                }
                None => api::post_json("/api/admin/recurring-plans", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar el plan"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if name.get().trim().is_empty()
            || category.get().is_empty()
            || account.get().is_empty()
            || start.get().trim().is_empty()
        {
            form_error.set(Some("Completa nombre, categoría, cuenta y fecha".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) = api::get_json::<api::RecurringPlanDetail>(&format!(
                "/api/admin/recurring-plans/{id}"
            ))
            .await
            {
                name.set(d.name);
                flow.set(d.flow_type);
                category.set(d.category_id);
                account.set(d.account_expected_id);
                amount.set(format!("{}", d.amount_estimated));
                frequency.set(d.frequency);
                start.set(rfc3339_to_date(&d.start_date));
                contact.set(d.contact_id.unwrap_or_default());
                dom.set(d.day_of_month.map(|v| v.to_string()).unwrap_or_default());
                end.set(d.end_date.as_deref().map(rfc3339_to_date).unwrap_or_default());
                is_active.set(d.is_active);
                version.set(d.version);
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/recurring-plans/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };
    // Materialize planned entries from the plan (idempotent per its version).
    let generated = RwSignal::new(None::<String>);
    let generate_one = move |id: String| {
        spawn_local(async move {
            match api::post_empty(&format!("/api/admin/recurring-plans/{id}/generate")).await {
                Ok(()) => generated.set(Some("Entradas generadas".into())),
                // Surface the server's reason (e.g. "recurring plan is inactive").
                Err(e) => generated.set(Some(format!(
                    "No se pudo generar: {}",
                    api::humanize(&e, "intenta de nuevo")
                ))),
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Planes recurrentes"</h1>

            {move || {
                generated.get().map(|m| view! { <p class="text-sm text-green-700">{m}</p> })
            }}

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar plan"
                                        } else {
                                            "Nuevo plan"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Nombre"
                                        </label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Flujo"
                                        </label>
                                        <Select value=flow>
                                            <option value="income">"Ingreso"</option>
                                            <option value="expense">"Egreso"</option>
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Frecuencia"
                                        </label>
                                        <Select value=frequency>
                                            <option value="monthly">"Mensual"</option>
                                            <option value="weekly">"Semanal"</option>
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Categoría"
                                        </label>
                                        <Select value=category>
                                            <option value="">"— Selecciona —"</option>
                                            {move || {
                                                categories
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| {
                                                        view! { <option value=id>{label}</option> }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Cuenta esperada"
                                        </label>
                                        <Select value=account>
                                            <option value="">"— Selecciona —"</option>
                                            {move || {
                                                accounts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| {
                                                        view! { <option value=id>{label}</option> }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Monto estimado"
                                        </label>
                                        <Input
                                            value=amount
                                            on_input=Callback::new(move |v| amount.set(v))
                                            inputmode="decimal"
                                            placeholder="0.00"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Inicio"
                                        </label>
                                        <Input
                                            value=start
                                            on_input=Callback::new(move |v| start.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Término (opcional)"
                                        </label>
                                        <Input
                                            value=end
                                            on_input=Callback::new(move |v| end.set(v))
                                            r#type="date"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Día del mes (opcional)"
                                        </label>
                                        <Input
                                            value=dom
                                            on_input=Callback::new(move |v| dom.set(v))
                                            inputmode="numeric"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Contacto (opcional)"
                                        </label>
                                        <Select value=contact>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                contacts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| {
                                                        view! { <option value=id>{label}</option> }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Notas"
                                        </label>
                                        <Input
                                            value=notes
                                            on_input=Callback::new(move |v| notes.set(v))
                                        />
                                    </div>
                                    <div class="sm:col-span-1">
                                        <Checkbox checked=is_active label="Activo" />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear plan"
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
                                                    <p class="text-sm text-red-600 sm:col-span-3">{m}</p>
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
                None => view! { <p class="text-muted-foreground">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar los planes."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-muted-foreground">"Sin planes todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-border bg-card">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-muted text-muted-foreground">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Flujo"</th>
                                        <th class="px-4 py-2 font-medium">"Monto"</th>
                                        <th class="px-4 py-2 font-medium">"Frecuencia"</th>
                                        <th class="px-4 py-2 font-medium">"Activo"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|p| {
                                            let id = p.id.clone();
                                            view! {
                                                <tr class="border-t border-border">
                                                    <td class="px-4 py-2">{p.name}</td>
                                                    <td class="px-4 py-2">
                                                        {flow_label(&p.flow_type).to_string()}
                                                    </td>
                                                    <td class="px-4 py-2">{money(p.amount_estimated)}</td>
                                                    <td class="px-4 py-2">{p.frequency}</td>
                                                    <td class="px-4 py-2">
                                                        {if p.is_active { "Sí" } else { "No" }}
                                                    </td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let eid = id.clone();
                                                                let gid = id.clone();
                                                                let did = id.clone();
                                                                view! {
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        on:click=move |_| generate_one(gid.clone())
                                                                    >
                                                                        "Generar"
                                                                    </Button>
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
