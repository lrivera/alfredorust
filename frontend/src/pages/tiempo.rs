//! Financial timeline: buckets the user's real and planned cash flow by
//! day/week/month/year. Mirrors the v1 "tiempo" screen — a cumulative
//! real-vs-planned chart, per-bucket Real and Plan rows (income / expense /
//! net / cumulative), and a drill-down into the transactions and planned
//! entries that fall in each period. The horizontal infinite-scroll of v1 is
//! presented here as a granularity toggle plus an explicit date range.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{date_to_rfc3339, peso, rfc3339_to_date};
use crate::api::{self, ApiError, Me, TimelineBucket};
use crate::components::{Button, ButtonVariant, Input};

const MODES: &[(&str, &str)] = &[
    ("day", "Día"),
    ("week", "Semana"),
    ("month", "Mes"),
    ("year", "Año"),
];

fn current_year() -> i32 {
    js_sys::Date::new_0().get_full_year() as i32
}

/// Tailwind text color for a signed amount: positive green, negative red, zero
/// muted.
fn sign_class(n: f64) -> &'static str {
    if n > 0.0 {
        "text-emerald-700"
    } else if n < 0.0 {
        "text-rose-700"
    } else {
        "text-slate-400"
    }
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

    let set_mode = move |m: &str| {
        mode.set(m.to_string());
        load();
    };
    let go_today = move |_| {
        let y = current_year();
        from.set(format!("{y}-01-01"));
        to.set(format!("{y}-12-31"));
        load();
    };
    let apply = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        load();
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Tiempo"</h1>

            <div class="flex flex-wrap items-end gap-3">
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Agrupar por"</label>
                    <div class="inline-flex overflow-hidden rounded-md border border-slate-300">
                        {MODES
                            .iter()
                            .map(|(v, l)| {
                                let val = *v;
                                let cls = move || {
                                    let active = mode.get() == val;
                                    let base = "px-3 py-1.5 text-sm border-r border-slate-200 last:border-r-0";
                                    if active {
                                        format!("{base} bg-sky-100 font-semibold text-sky-700")
                                    } else {
                                        format!("{base} bg-white text-slate-600 hover:bg-slate-50")
                                    }
                                };
                                view! {
                                    <button type="button" class=cls on:click=move |_| set_mode(val)>
                                        {*l}
                                    </button>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                </div>
                <form on:submit=apply class="flex flex-wrap items-end gap-3">
                    <div class="space-y-1">
                        <label class="block text-sm font-medium text-slate-700">"Desde"</label>
                        <Input value=from on_input=Callback::new(move |v| from.set(v)) r#type="date" />
                    </div>
                    <div class="space-y-1">
                        <label class="block text-sm font-medium text-slate-700">"Hasta"</label>
                        <Input value=to on_input=Callback::new(move |v| to.set(v)) r#type="date" />
                    </div>
                    <Button r#type="submit">"Actualizar"</Button>
                    <Button variant=ButtonVariant::Outline on:click=go_today>
                        "Ir a hoy"
                    </Button>
                </form>
            </div>

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
                        <div class="space-y-4">
                            {cumulative_chart(&list)}
                            <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                                {list.into_iter().map(bucket_card).collect::<Vec<_>>()}
                            </div>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

/// SVG line chart of cumulative real vs planned across the buckets.
fn cumulative_chart(buckets: &[TimelineBucket]) -> AnyView {
    let n = buckets.len();
    if n < 2 {
        return ().into_any();
    }
    let reals: Vec<f64> = buckets.iter().map(|b| b.cumulative_real).collect();
    let plans: Vec<f64> = buckets.iter().map(|b| b.cumulative_planned).collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for v in reals.iter().chain(plans.iter()) {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    if (hi - lo).abs() < 1.0 {
        hi = lo + 1.0;
    }
    let step = 60.0;
    let pad = 24.0;
    let h = 180.0;
    let w = pad * 2.0 + (n as f64 - 1.0) * step;
    let x = |i: usize| pad + (i as f64) * step;
    let y = |v: f64| h - pad - (v - lo) / (hi - lo) * (h - 2.0 * pad);
    let points = |series: &[f64]| {
        series
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{:.1},{:.1}", x(i), y(*v)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let real_pts = points(&reals);
    let plan_pts = points(&plans);

    view! {
        <div class="overflow-x-auto rounded-xl border border-slate-200 bg-white p-3">
            <div class="mb-2 flex gap-4 text-xs text-slate-600">
                <span class="inline-flex items-center gap-1">
                    <span class="inline-block h-2 w-3 rounded-sm bg-sky-500"></span>
                    "Real acumulado"
                </span>
                <span class="inline-flex items-center gap-1">
                    <span class="inline-block h-2 w-3 rounded-sm bg-purple-500"></span>
                    "Plan acumulado"
                </span>
            </div>
            <svg width=w height=h viewBox=format!("0 0 {w} {h}") class="block">
                <polyline points=plan_pts fill="none" stroke="#a855f7" stroke-width="2" stroke-dasharray="4 3" />
                <polyline points=real_pts fill="none" stroke="#0ea5e9" stroke-width="2" />
            </svg>
        </div>
    }
    .into_any()
}

/// One bucket card: period label, a Real and a Plan metric row, and the
/// transactions / planned entries that fall in the period.
fn bucket_card(b: TimelineBucket) -> AnyView {
    let period = format!("{} → {}", rfc3339_to_date(&b.start), rfc3339_to_date(&b.end));
    view! {
        <div class="rounded-xl border border-slate-200 bg-white p-3">
            <p class="mb-2 text-sm font-semibold text-slate-700">{period}</p>
            <table class="w-full text-right text-xs">
                <thead class="text-slate-500">
                    <tr>
                        <th class="py-1 text-left font-medium">"Tipo"</th>
                        <th class="py-1 font-medium">"Ingresos"</th>
                        <th class="py-1 font-medium">"Gastos"</th>
                        <th class="py-1 font-medium">"Total"</th>
                        <th class="py-1 font-medium">"Acum."</th>
                    </tr>
                </thead>
                <tbody>
                    <tr class="border-t border-slate-100">
                        <td class="py-1 text-left font-medium">"Real"</td>
                        <td class="py-1 text-emerald-700">{peso(b.real_income)}</td>
                        <td class="py-1 text-rose-700">{peso(-b.real_expense)}</td>
                        <td class=format!("py-1 font-medium {}", sign_class(b.net_real))>
                            {peso(b.net_real)}
                        </td>
                        <td class="py-1 font-medium">{peso(b.cumulative_real)}</td>
                    </tr>
                    <tr class="border-t border-slate-100 text-slate-500">
                        <td class="py-1 text-left font-medium">"Plan"</td>
                        <td class="py-1">{peso(b.planned_income)}</td>
                        <td class="py-1">{peso(-b.planned_expense)}</td>
                        <td class=format!("py-1 font-medium {}", sign_class(b.net_planned))>
                            {peso(b.net_planned)}
                        </td>
                        <td class="py-1 font-medium">{peso(b.cumulative_planned)}</td>
                    </tr>
                </tbody>
            </table>

            {(!b.transactions.is_empty() || !b.planned_entries.is_empty())
                .then(|| {
                    let txs = b.transactions.clone();
                    let plans = b.planned_entries.clone();
                    view! {
                        <details class="mt-2 text-xs">
                            <summary class="cursor-pointer text-slate-500">
                                {format!(
                                    "{} movimientos · {} planificadas",
                                    txs.len(),
                                    plans.len(),
                                )}
                            </summary>
                            <ul class="mt-1 space-y-0.5">
                                {txs
                                    .into_iter()
                                    .map(|t| {
                                        let income = t.tx_type == "income";
                                        let cls = if income { "text-emerald-700" } else { "text-rose-700" };
                                        let sign = if income { "+" } else { "-" };
                                        view! {
                                            <li class="rounded bg-slate-50 px-2 py-0.5">
                                                <span class=cls>{format!("{sign}{}", peso(t.amount))}</span>
                                                " " {t.description}
                                            </li>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                                {plans
                                    .into_iter()
                                    .map(|p| {
                                        let income = p.flow_type == "income";
                                        let cls = if income { "text-emerald-700" } else { "text-rose-700" };
                                        let sign = if income { "+" } else { "-" };
                                        view! {
                                            <li class="rounded border border-dashed border-slate-200 px-2 py-0.5">
                                                <span class=cls>
                                                    {format!("{sign}{}", peso(p.amount_estimated))}
                                                </span>
                                                " " {p.name} " · " <span class="text-slate-400">{p.status}</span>
                                            </li>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </ul>
                        </details>
                    }
                })}
        </div>
    }
    .into_any()
}
