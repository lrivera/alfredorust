use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, load_account_options, load_category_options, money, rfc3339_to_date, Options,
};
use crate::api::{self, ApiError, Me, Transaction, TransactionPayload};
use crate::components::{
    Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input, Select,
};

const TX_TYPES: &[(&str, &str)] = &[
    ("income", "Ingreso"),
    ("expense", "Egreso"),
    ("transfer", "Transferencia"),
];

fn tx_label(value: &str) -> &str {
    TX_TYPES.iter().find(|(v, _)| *v == value).map(|(_, l)| *l).unwrap_or(value)
}

fn opt_id(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[component]
pub fn TransactionsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Transaction>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(
                api::get_json::<Vec<Transaction>>("/api/admin/transactions/data").await,
            ));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let accounts = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);

    let editing = RwSignal::new(None::<String>);
    let date = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let tx_type = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account_from = RwSignal::new(String::new());
    let account_to = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let is_confirmed = RwSignal::new(true);
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        date.set(String::new());
        description.set(String::new());
        tx_type.set("expense".to_string());
        category.set(String::new());
        account_from.set(String::new());
        account_to.set(String::new());
        amount.set(String::new());
        is_confirmed.set(true);
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let notes_val = notes.get_untracked();
        let payload = TransactionPayload {
            date: date_to_rfc3339(&date.get_untracked()),
            description: description.get_untracked().trim().to_string(),
            transaction_type: tx_type.get_untracked(),
            category_id: category.get_untracked(),
            account_from_id: opt_id(account_from.get_untracked()),
            account_to_id: opt_id(account_to.get_untracked()),
            amount: amount.get_untracked().trim().parse().unwrap_or(0.0),
            is_confirmed: is_confirmed.get_untracked(),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/transactions/{id}/update"), &payload).await
                }
                None => api::post_json("/api/admin/transactions", &payload).await,
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
                Err(_) => form_error.set(Some("No se pudo guardar el movimiento".into())),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if date.get().trim().is_empty()
            || description.get().trim().is_empty()
            || category.get().is_empty()
        {
            form_error.set(Some("Completa fecha, descripción y categoría".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::TransactionDetail>(&format!("/api/admin/transactions/{id}"))
                    .await
            {
                date.set(rfc3339_to_date(&d.date));
                description.set(d.description);
                tx_type.set(d.transaction_type);
                category.set(d.category_id);
                account_from.set(d.account_from_id.unwrap_or_default());
                account_to.set(d.account_to_id.unwrap_or_default());
                amount.set(format!("{}", d.amount));
                is_confirmed.set(d.is_confirmed);
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/transactions/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Movimientos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar movimiento"
                                        } else {
                                            "Nuevo movimiento"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Fecha"
                                        </label>
                                        <Input
                                            value=date
                                            on_input=Callback::new(move |v| date.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Descripción"
                                        </label>
                                        <Input
                                            value=description
                                            on_input=Callback::new(move |v| description.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Tipo"
                                        </label>
                                        <Select value=tx_type>
                                            {TX_TYPES
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
                                            "Monto"
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
                                            "Cuenta origen"
                                        </label>
                                        <Select value=account_from>
                                            <option value="">"— Ninguna —"</option>
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
                                            "Cuenta destino"
                                        </label>
                                        <Select value=account_to>
                                            <option value="">"— Ninguna —"</option>
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
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Notas"
                                        </label>
                                        <Input
                                            value=notes
                                            on_input=Callback::new(move |v| notes.set(v))
                                        />
                                    </div>
                                    <div class="flex items-end">
                                        <Checkbox checked=is_confirmed label="Confirmado" />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear movimiento"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los movimientos."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin movimientos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Fecha"</th>
                                        <th class="px-4 py-2 font-medium">"Descripción"</th>
                                        <th class="px-4 py-2 font-medium">"Tipo"</th>
                                        <th class="px-4 py-2 font-medium">"Monto"</th>
                                        <th class="px-4 py-2 font-medium">"Categoría"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|t| {
                                            let id = t.id.clone();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2 text-slate-500">{t.date}</td>
                                                    <td class="px-4 py-2">{t.description}</td>
                                                    <td class="px-4 py-2">{tx_label(&t.tx_type).to_string()}</td>
                                                    <td class="px-4 py-2">{money(t.amount)}</td>
                                                    <td class="px-4 py-2 text-slate-500">{t.category}</td>
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
