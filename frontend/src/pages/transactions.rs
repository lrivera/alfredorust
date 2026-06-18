use leptos::prelude::*;
use leptos::task::spawn_local;

use std::collections::{BTreeMap, HashMap};

use super::charts::{donut, line_area_chart};
use super::{
    date_to_rfc3339, load_account_options, load_category_options, load_planned_entry_options, money,
    rfc3339_to_date, Options,
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

/// KPIs + monthly income/expense chart + donut + top-category bars, computed
/// from the loaded transactions (ported from the v1 charts).
fn tx_charts(list: &[Transaction]) -> impl IntoView {
    let mut monthly: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut cats: HashMap<String, (f64, f64, f64, u32)> = HashMap::new();
    let (mut total_income, mut total_expense) = (0.0_f64, 0.0_f64);
    for t in list {
        let month = t.date.get(..7).unwrap_or("").to_string();
        let m = monthly.entry(month).or_default();
        match t.tx_type.as_str() {
            "income" => {
                m.0 += t.amount;
                total_income += t.amount;
            }
            "expense" => {
                m.1 += t.amount;
                total_expense += t.amount;
            }
            _ => {}
        }
        let key = if t.category.is_empty() {
            "Sin categoría".to_string()
        } else {
            t.category.clone()
        };
        let c = cats.entry(key).or_insert((0.0, 0.0, 0.0, 0));
        match t.tx_type.as_str() {
            "income" => c.0 += t.amount,
            "expense" => c.1 += t.amount,
            _ => c.2 += t.amount,
        }
        c.3 += 1;
    }
    let series: Vec<(String, f64, f64)> = monthly.into_iter().map(|(m, (i, e))| (m, i, e)).collect();
    let line_svg = line_area_chart(&series);
    let net = total_income - total_expense;
    let donut_svg = donut(
        &[
            ("Ingresos".into(), total_income, "#10b981".into()),
            ("Egresos".into(), total_expense, "#f43f5e".into()),
        ],
        &money(net),
        "neto",
    );

    let mut cat_vec: Vec<(String, f64, f64, f64, u32)> =
        cats.into_iter().map(|(k, (i, e, tr, n))| (k, i, e, tr, n)).collect();
    cat_vec.sort_by(|a, b| {
        (b.1 + b.2 + b.3)
            .partial_cmp(&(a.1 + a.2 + a.3))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cat_vec.truncate(8);
    let max_amt = cat_vec.iter().map(|c| c.1 + c.2 + c.3).fold(1.0_f64, f64::max);

    let bars = cat_vec
        .into_iter()
        .map(|(name, inc, exp, tr, count)| {
            let total = inc + exp + tr;
            let pct = |v: f64| (v / max_amt) * 100.0;
            view! {
                <div class="mb-2">
                    <div class="flex justify-between text-xs">
                        <span class="truncate text-slate-600">{name}</span>
                        <span class="font-medium text-slate-800">{money(total)}</span>
                    </div>
                    <div class="mt-1 flex h-1.5 overflow-hidden rounded bg-slate-100">
                        <div style=format!("width:{}%;background:#10b981", pct(inc))></div>
                        <div style=format!("width:{}%;background:#f43f5e", pct(exp))></div>
                        <div style=format!("width:{}%;background:#2563eb", pct(tr))></div>
                    </div>
                    <span class="text-[10px] text-slate-400">{format!("{count} mov.")}</span>
                </div>
            }
        })
        .collect::<Vec<_>>();

    let kpi = |label: &str, value: String, color: &str| {
        view! {
            <div class="rounded-xl border border-slate-200 bg-white p-3">
                <p class="text-[11px] font-semibold uppercase tracking-wide text-slate-400">
                    {label.to_string()}
                </p>
                <p class="mt-1 text-2xl font-bold" style=format!("color:{color}")>
                    {value}
                </p>
            </div>
        }
    };

    view! {
        <div class="space-y-4">
            <div class="grid gap-3 sm:grid-cols-3">
                {kpi("Ingresos", money(total_income), "#10b981")}
                {kpi("Egresos", money(total_expense), "#f43f5e")}
                {kpi("Neto", money(net), if net >= 0.0 { "#0ea5e9" } else { "#f43f5e" })}
            </div>
            <div class="grid gap-3 lg:grid-cols-3">
                <div class="rounded-xl border border-slate-200 bg-white p-3 lg:col-span-2">
                    <p class="mb-2 text-xs font-semibold text-slate-500">"Mensual (ingresos vs egresos)"</p>
                    <div class="h-40" inner_html=line_svg></div>
                </div>
                <div class="rounded-xl border border-slate-200 bg-white p-3">
                    <p class="mb-2 text-xs font-semibold text-slate-500">"Distribución"</p>
                    <div class="mx-auto h-40 w-40" inner_html=donut_svg></div>
                </div>
            </div>
            <div class="rounded-xl border border-slate-200 bg-white p-3">
                <p class="mb-2 text-xs font-semibold text-slate-500">"Por categoría"</p>
                {bars}
            </div>
        </div>
    }
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
    let planned = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);
    load_planned_entry_options(planned);

    let editing = RwSignal::new(None::<String>);
    let date = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let tx_type = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account_from = RwSignal::new(String::new());
    let account_to = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let planned_entry = RwSignal::new(String::new());
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
        planned_entry.set(String::new());
        is_confirmed.set(true);
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let notes_val = notes.get_untracked();
        let planned_val = planned_entry.get_untracked();
        let payload = TransactionPayload {
            date: date_to_rfc3339(&date.get_untracked()),
            description: description.get_untracked().trim().to_string(),
            transaction_type: tx_type.get_untracked(),
            category_id: category.get_untracked(),
            account_from_id: opt_id(account_from.get_untracked()),
            account_to_id: opt_id(account_to.get_untracked()),
            amount: amount.get_untracked().trim().parse().unwrap_or(0.0),
            planned_entry_id: (!planned_val.is_empty()).then_some(planned_val),
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
                planned_entry.set(d.planned_entry_id.unwrap_or_default());
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
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Compromiso ligado (opcional)"
                                        </label>
                                        <Select value=planned_entry>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                planned
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
                                        <Button r#type="submit" disabled=pending>
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

            {move || {
                match items.get() {
                    Some(Ok(list)) if !list.is_empty() => tx_charts(&list).into_any(),
                    _ => ().into_any(),
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
