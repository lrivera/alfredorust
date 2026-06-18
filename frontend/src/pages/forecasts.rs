use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{date_to_rfc3339, money, rfc3339_to_date};
use crate::api::{self, ApiError, Forecast, ForecastPayload, Me};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input};

fn num(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse an optional money field: empty -> None.
fn opt_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

#[component]
pub fn ForecastsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Forecast>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::get_json::<Vec<Forecast>>("/api/admin/forecasts").await));
        });
    };
    reload();

    let editing = RwSignal::new(None::<String>);
    let scenario = RwSignal::new(String::new());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let currency = RwSignal::new("MXN".to_string());
    let income = RwSignal::new(String::new());
    let expense = RwSignal::new(String::new());
    let initial = RwSignal::new(String::new());
    let final_b = RwSignal::new(String::new());
    let details = RwSignal::new(String::new());
    let gen_at = RwSignal::new(None::<String>);
    let notes = RwSignal::new(None::<String>);
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        scenario.set(String::new());
        start.set(String::new());
        end.set(String::new());
        currency.set("MXN".to_string());
        income.set(String::new());
        expense.set(String::new());
        initial.set(String::new());
        final_b.set(String::new());
        details.set(String::new());
        gen_at.set(None);
        notes.set(None);
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let inc = num(&income.get_untracked());
        let exp = num(&expense.get_untracked());
        let start_iso = date_to_rfc3339(&start.get_untracked());
        let scenario_name = scenario.get_untracked();
        let details_val = details.get_untracked();
        let payload = ForecastPayload {
            generated_at: gen_at.get_untracked().unwrap_or_else(|| start_iso.clone()),
            start_date: start_iso,
            end_date: date_to_rfc3339(&end.get_untracked()),
            currency: currency.get_untracked(),
            projected_income_total: inc,
            projected_expense_total: exp,
            projected_net: inc - exp,
            initial_balance: opt_num(&initial.get_untracked()),
            final_balance: opt_num(&final_b.get_untracked()),
            details: (!details_val.trim().is_empty()).then_some(details_val),
            scenario_name: (!scenario_name.trim().is_empty()).then_some(scenario_name),
            notes: notes.get_untracked(),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/forecasts/{id}/update"), &payload).await
                }
                None => api::post_json("/api/admin/forecasts", &payload).await,
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
                Err(_) => form_error.set(Some("No se pudo guardar el pronóstico".into())),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if start.get().trim().is_empty() || end.get().trim().is_empty() {
            form_error.set(Some("Las fechas son obligatorias".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::ForecastDetail>(&format!("/api/admin/forecasts/{id}")).await
            {
                scenario.set(d.scenario_name.unwrap_or_default());
                start.set(rfc3339_to_date(&d.start_date));
                end.set(rfc3339_to_date(&d.end_date));
                currency.set(d.currency);
                income.set(format!("{}", d.projected_income_total));
                expense.set(format!("{}", d.projected_expense_total));
                initial.set(d.initial_balance.map(|v| v.to_string()).unwrap_or_default());
                final_b.set(d.final_balance.map(|v| v.to_string()).unwrap_or_default());
                details.set(d.details.unwrap_or_default());
                gen_at.set(Some(d.generated_at));
                notes.set(d.notes);
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/forecasts/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Pronósticos"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-3xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar pronóstico"
                                        } else {
                                            "Nuevo pronóstico"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Escenario"
                                        </label>
                                        <Input
                                            value=scenario
                                            on_input=Callback::new(move |v| scenario.set(v))
                                            placeholder="Base 2026"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Desde"
                                        </label>
                                        <Input
                                            value=start
                                            on_input=Callback::new(move |v| start.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Hasta"
                                        </label>
                                        <Input
                                            value=end
                                            on_input=Callback::new(move |v| end.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Moneda"
                                        </label>
                                        <Input
                                            value=currency
                                            on_input=Callback::new(move |v| currency.set(v))
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Ingreso proyectado"
                                        </label>
                                        <Input
                                            value=income
                                            on_input=Callback::new(move |v| income.set(v))
                                            inputmode="decimal"
                                            placeholder="0.00"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Egreso proyectado"
                                        </label>
                                        <Input
                                            value=expense
                                            on_input=Callback::new(move |v| expense.set(v))
                                            inputmode="decimal"
                                            placeholder="0.00"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Saldo inicial (opcional)"
                                        </label>
                                        <Input
                                            value=initial
                                            on_input=Callback::new(move |v| initial.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Saldo final (opcional)"
                                        </label>
                                        <Input
                                            value=final_b
                                            on_input=Callback::new(move |v| final_b.set(v))
                                            inputmode="decimal"
                                        />
                                    </div>
                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Detalles (opcional)"
                                        </label>
                                        <Input
                                            value=details
                                            on_input=Callback::new(move |v| details.set(v))
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
                                                    "Crear pronóstico"
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
                    view! { <p class="text-red-600">"No se pudieron cargar los pronósticos."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin pronósticos todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Escenario"</th>
                                        <th class="px-4 py-2 font-medium">"Periodo"</th>
                                        <th class="px-4 py-2 font-medium">"Neto"</th>
                                        <th class="px-4 py-2 font-medium">"Moneda"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|f| {
                                            let id = f.id.clone();
                                            let period = format!("{} → {}", f.start_date, f.end_date);
                                            let scenario = f.scenario_name.unwrap_or_default();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{scenario}</td>
                                                    <td class="px-4 py-2 text-slate-500">{period}</td>
                                                    <td class="px-4 py-2">{money(f.projected_net)}</td>
                                                    <td class="px-4 py-2">{f.currency}</td>
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
