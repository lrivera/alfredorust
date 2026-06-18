use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, flow_label, load_account_options, load_category_options, money,
    rfc3339_to_date, Options,
};
use crate::api::{self, ApiError, Me, RecurringPlan, RecurringPlanPayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

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

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let flow = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let frequency = RwSignal::new("monthly".to_string());
    let start = RwSignal::new(String::new());
    // Hidden fields preserved across edits.
    let contact_id = RwSignal::new(None::<String>);
    let day_of_month = RwSignal::new(None::<i32>);
    let end_date = RwSignal::new(None::<String>);
    let is_active = RwSignal::new(true);
    let version = RwSignal::new(1i32);
    let notes = RwSignal::new(None::<String>);
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        flow.set("expense".to_string());
        category.set(String::new());
        account.set(String::new());
        amount.set(String::new());
        frequency.set("monthly".to_string());
        start.set(String::new());
        contact_id.set(None);
        day_of_month.set(None);
        end_date.set(None);
        is_active.set(true);
        version.set(1);
        notes.set(None);
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let payload = RecurringPlanPayload {
            name: name.get_untracked().trim().to_string(),
            flow_type: flow.get_untracked(),
            category_id: category.get_untracked(),
            account_expected_id: account.get_untracked(),
            contact_id: contact_id.get_untracked(),
            amount_estimated: amount.get_untracked().trim().parse().unwrap_or(0.0),
            frequency: frequency.get_untracked(),
            day_of_month: day_of_month.get_untracked(),
            start_date: date_to_rfc3339(&start.get_untracked()),
            end_date: end_date.get_untracked(),
            is_active: is_active.get_untracked(),
            version: version.get_untracked(),
            notes: notes.get_untracked(),
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
                Err(ApiError::Forbidden) => form_error.set(Some("No tienes permiso".into())),
                Err(_) => form_error.set(Some("No se pudo guardar el plan".into())),
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
                contact_id.set(d.contact_id);
                day_of_month.set(d.day_of_month);
                end_date.set(d.end_date);
                is_active.set(d.is_active);
                version.set(d.version);
                notes.set(d.notes);
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

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Planes recurrentes"</h1>

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
                                            "Frecuencia"
                                        </label>
                                        <Select value=frequency>
                                            <option value="monthly">"Mensual"</option>
                                            <option value="weekly">"Semanal"</option>
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
                                            "Inicio"
                                        </label>
                                        <Input
                                            value=start
                                            on_input=Callback::new(move |v| start.set(v))
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
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar los planes."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin planes todavía."</p> }.into_any()
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
                                                <tr class="border-t border-slate-100">
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
