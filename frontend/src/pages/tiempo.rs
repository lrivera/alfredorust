use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{date_to_rfc3339, money, rfc3339_to_date};
use crate::api::{self, ApiError, Me, TimelineBucket};
use crate::components::{Button, Input, Select};

const MODES: &[(&str, &str)] = &[
    ("day", "Día"),
    ("week", "Semana"),
    ("month", "Mes"),
    ("year", "Año"),
];

fn current_year() -> i32 {
    js_sys::Date::new_0().get_full_year() as i32
}

#[component]
pub fn TiempoPage() -> impl IntoView {
    // Any authenticated user; the ViewTimeline permission is enforced server-side.
    let _me = use_context::<Me>();
    let year = current_year();

    let mode = RwSignal::new("month".to_string());
    let from = RwSignal::new(format!("{year}-01-01"));
    let to = RwSignal::new(format!("{year}-12-31"));
    let data = RwSignal::new(None::<Result<Vec<TimelineBucket>, ApiError>>);

    let load = move || {
        let url = format!(
            "/api/tiempo?mode={}&from={}&to={}",
            mode.get_untracked(),
            date_to_rfc3339(&from.get_untracked()),
            date_to_rfc3339(&to.get_untracked()),
        );
        data.set(None);
        spawn_local(async move {
            data.set(Some(api::get_json::<Vec<TimelineBucket>>(&url).await));
        });
    };
    load();

    let apply = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        load();
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Tiempo"</h1>

            <form on:submit=apply class="flex flex-wrap items-end gap-3">
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Agrupar por"</label>
                    <Select value=mode>
                        {MODES
                            .iter()
                            .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                            .collect::<Vec<_>>()}
                    </Select>
                </div>
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Desde"</label>
                    <Input value=from on_input=Callback::new(move |v| from.set(v)) r#type="date" />
                </div>
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Hasta"</label>
                    <Input value=to on_input=Callback::new(move |v| to.set(v)) r#type="date" />
                </div>
                <Button r#type="submit">"Actualizar"</Button>
            </form>

            {move || match data.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudo cargar la línea de tiempo."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin datos en el rango."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Periodo"</th>
                                        <th class="px-4 py-2 font-medium">"Ingreso real"</th>
                                        <th class="px-4 py-2 font-medium">"Gasto real"</th>
                                        <th class="px-4 py-2 font-medium">"Neto real"</th>
                                        <th class="px-4 py-2 font-medium">"Acum. real"</th>
                                        <th class="px-4 py-2 font-medium">"Neto plan"</th>
                                        <th class="px-4 py-2 font-medium">"Acum. plan"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|b| {
                                            let period = format!(
                                                "{} → {}",
                                                rfc3339_to_date(&b.start),
                                                rfc3339_to_date(&b.end),
                                            );
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2 text-slate-500">{period}</td>
                                                    <td class="px-4 py-2">{money(b.real_income)}</td>
                                                    <td class="px-4 py-2">{money(b.real_expense)}</td>
                                                    <td class="px-4 py-2">{money(b.net_real)}</td>
                                                    <td class="px-4 py-2 font-medium">
                                                        {money(b.cumulative_real)}
                                                    </td>
                                                    <td class="px-4 py-2 text-slate-500">
                                                        {money(b.net_planned)}
                                                    </td>
                                                    <td class="px-4 py-2 text-slate-500">
                                                        {money(b.cumulative_planned)}
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
