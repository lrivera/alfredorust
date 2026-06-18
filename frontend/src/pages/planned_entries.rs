use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, flow_label, load_account_options, load_category_options, money,
    rfc3339_to_date, Options,
};
use crate::api::{self, ApiError, Me, PlannedEntry, PlannedEntryPayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

const STATUSES: &[(&str, &str)] = &[
    ("planned", "Planificado"),
    ("partially_covered", "Parcial"),
    ("covered", "Cubierto"),
    ("overdue", "Vencido"),
    ("cancelled", "Cancelado"),
];

#[component]
pub fn PlannedEntriesPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<PlannedEntry>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(
                api::get_json::<Vec<PlannedEntry>>("/api/admin/planned-entries").await,
            ));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let accounts = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let flow = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let due = RwSignal::new(String::new());
    let status = RwSignal::new("planned".to_string());
    let contact_id = RwSignal::new(None::<String>);
    let notes = RwSignal::new(None::<String>);
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        flow.set("expense".to_string());
        category.set(String::new());
        account.set(String::new());
        amount.set(String::new());
        due.set(String::new());
        status.set("planned".to_string());
        contact_id.set(None);
        notes.set(None);
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let payload = PlannedEntryPayload {
            name: name.get_untracked().trim().to_string(),
            flow_type: flow.get_untracked(),
            category_id: category.get_untracked(),
            account_expected_id: account.get_untracked(),
            contact_id: contact_id.get_untracked(),
            amount_estimated: amount.get_untracked().trim().parse().unwrap_or(0.0),
            due_date: date_to_rfc3339(&due.get_untracked()),
            status: status.get_untracked(),
            notes: notes.get_untracked(),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/planned-entries/{id}/update"), &payload)
                        .await
                }
                None => api::post_json("/api/admin/planned-entries", &payload).await,
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
                Err(_) => form_error.set(Some("No se pudo guardar la entrada".into())),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if name.get().trim().is_empty()
            || category.get().is_empty()
            || account.get().is_empty()
            || due.get().trim().is_empty()
        {
            form_error.set(Some("Completa nombre, categoría, cuenta y vencimiento".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) = api::get_json::<api::PlannedEntryDetail>(&format!(
                "/api/admin/planned-entries/{id}"
            ))
            .await
            {
                name.set(d.name);
                flow.set(d.flow_type);
                category.set(d.category_id);
                account.set(d.account_expected_id);
                amount.set(format!("{}", d.amount_estimated));
                due.set(rfc3339_to_date(&d.due_date));
                status.set(d.status);
                contact_id.set(d.contact_id);
                notes.set(d.notes);
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/planned-entries/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Entradas planificadas"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar entrada"
                                        } else {
                                            "Nueva entrada"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Nombre"
                                        </label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
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
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Estado"
                                        </label>
                                        <Select value=status>
                                            {STATUSES
                                                .iter()
                                                .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
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
                                        <label class="block text-sm font-medium text-slate-700">
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
                                        <label class="block text-sm font-medium text-slate-700">
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
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Vencimiento"
                                        </label>
                                        <Input
                                            value=due
                                            on_input=Callback::new(move |v| due.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear entrada"
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
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las entradas."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin entradas todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Flujo"</th>
                                        <th class="px-4 py-2 font-medium">"Monto"</th>
                                        <th class="px-4 py-2 font-medium">"Vence"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|e| {
                                            let id = e.id.clone();
                                            let status = if e.status_label.is_empty() {
                                                e.status.clone()
                                            } else {
                                                e.status_label.clone()
                                            };
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{e.name}</td>
                                                    <td class="px-4 py-2">
                                                        {flow_label(&e.flow_type).to_string()}
                                                    </td>
                                                    <td class="px-4 py-2">{money(e.amount_estimated)}</td>
                                                    <td class="px-4 py-2 text-slate-500">{e.due_date}</td>
                                                    <td class="px-4 py-2">{status}</td>
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
