//! CFDIs: the read-only list of imported invoices plus the SAT download panel.
//! Admins pick a SAT config and date range, start a download (the backend
//! splits it into one background job per month), and the page polls the job
//! list until every chunk finishes. Mirrors the v1 download UI.

use std::collections::BTreeMap;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::charts::{donut, line_area_chart};
use super::{money, rfc3339_to_date};
use crate::api::{self, ApiError, Cfdi, CfdiDownloadPayload, CfdiJob, CfdiList, Me, SatConfigData};
use crate::components::{Button, Card, CardContent, CardHeader, CardTitle, Checkbox, Input, Select};

fn today_str() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .map(|s| s.get(..10).unwrap_or(&s).to_string())
        .unwrap_or_default()
}

fn years_ago_jan(years: i32) -> String {
    let y = js_sys::Date::new_0().get_full_year() as i32 - years;
    format!("{y}-01-01")
}

/// Poll the job list every 4s while any chunk is still queued/running.
fn poll_jobs(cid: String, jobs: RwSignal<Vec<CfdiJob>>, busy: RwSignal<bool>) {
    spawn_local(async move {
        match api::get_json::<Vec<CfdiJob>>(&format!("/api/admin/companies/{cid}/cfdi/jobs")).await {
            Ok(list) => {
                let active = list
                    .iter()
                    .any(|j| matches!(j.status.status.as_str(), "queued" | "running"));
                jobs.set(list);
                busy.set(active);
                if active {
                    set_timeout(move || poll_jobs(cid, jobs, busy), Duration::from_secs(4));
                }
            }
            Err(_) => busy.set(false),
        }
    });
}

fn status_badge(status: &str) -> impl IntoView {
    let (label, cls) = match status {
        "queued" => ("En cola", "bg-slate-100 text-slate-600"),
        "running" => ("● Descargando", "bg-sky-100 text-sky-700"),
        "done" => ("✓ Listo", "bg-emerald-100 text-emerald-700"),
        "failed" => ("✗ Error", "bg-rose-100 text-rose-700"),
        other => (other, "bg-slate-100 text-slate-600"),
    };
    // Own the label so the returned view doesn't borrow the `status` argument.
    let label = label.to_string();
    view! { <span class=format!("rounded px-2 py-0.5 text-xs {cls}")>{label}</span> }
}

/// KPIs + monthly emitidos/recibidos chart + donut, computed from the CFDI list
/// (ported from the v1 CFDI charts).
fn cfdi_charts(items: &[Cfdi]) -> impl IntoView {
    let mut monthly: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let (mut emitidos, mut recibidos) = (0.0_f64, 0.0_f64);
    for c in items {
        // Skip payment complements (tipo P) and zero totals, like v1.
        if c.tipo == "P" || c.total <= 0.0 {
            continue;
        }
        let month = c.fecha.get(..7).unwrap_or("").to_string();
        let m = monthly.entry(month).or_default();
        if c.es_emitido {
            m.0 += c.total;
            emitidos += c.total;
        } else {
            m.1 += c.total;
            recibidos += c.total;
        }
    }
    let series: Vec<(String, f64, f64)> = monthly.into_iter().map(|(m, (i, e))| (m, i, e)).collect();
    let line_svg = line_area_chart(&series);
    let donut_svg = donut(
        &[
            ("Emitidos".into(), emitidos, "#10b981".into()),
            ("Recibidos".into(), recibidos, "#f43f5e".into()),
        ],
        &money(emitidos - recibidos),
        "neto",
    );
    let kpi = |label: &str, value: String, color: &str| {
        view! {
            <div class="rounded-xl border border-slate-200 bg-white p-3">
                <p class="text-[11px] font-semibold uppercase tracking-wide text-slate-400">
                    {label.to_string()}
                </p>
                <p class="mt-1 text-2xl font-bold" style=format!("color:{color}")>{value}</p>
            </div>
        }
    };
    view! {
        <div class="space-y-4">
            <div class="grid gap-3 sm:grid-cols-3">
                {kpi("Emitidos", money(emitidos), "#10b981")}
                {kpi("Recibidos", money(recibidos), "#f43f5e")}
                {kpi("Neto", money(emitidos - recibidos), "#0ea5e9")}
            </div>
            <div class="grid gap-3 lg:grid-cols-3">
                <div class="rounded-xl border border-slate-200 bg-white p-3 lg:col-span-2">
                    <p class="mb-2 text-xs font-semibold text-slate-500">"Mensual (emitidos vs recibidos)"</p>
                    <div class="h-40" inner_html=line_svg></div>
                </div>
                <div class="rounded-xl border border-slate-200 bg-white p-3">
                    <p class="mb-2 text-xs font-semibold text-slate-500">"Dirección"</p>
                    <div class="mx-auto h-40 w-40" inner_html=donut_svg></div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn CfdiPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";
    let company_id = me.active_company_id().unwrap_or_default();

    // --- imported CFDI list ---
    let data = RwSignal::new(None::<Result<CfdiList, ApiError>>);
    let reload = move || {
        data.set(None);
        spawn_local(async move {
            data.set(Some(api::get_json::<CfdiList>("/api/admin/cfdis/data").await));
        });
    };
    reload();

    // --- download panel state ---
    let configs = RwSignal::new(Vec::<SatConfigData>::new());
    let sat_config = RwSignal::new(String::new());
    let start = RwSignal::new(years_ago_jan(5));
    let end = RwSignal::new(today_str());
    let dl_type = RwSignal::new("both".to_string());
    let auto_pay = RwSignal::new(false);
    let jobs = RwSignal::new(Vec::<CfdiJob>::new());
    let busy = RwSignal::new(false);
    let dl_error = RwSignal::new(None::<String>);

    if is_admin {
        spawn_local(async move {
            if let Ok(list) = api::get_json::<Vec<SatConfigData>>("/api/admin/sat-configs").await {
                if let Some(first) = list.first() {
                    sat_config.set(first.id.clone());
                }
                configs.set(list);
            }
        });
        // Surface any jobs still running from a previous visit.
        let cid = company_id.clone();
        poll_jobs(cid, jobs, busy);
    }

    let cid_dl = company_id.clone();
    let download = Action::new_local(move |_: &()| {
        let payload = CfdiDownloadPayload {
            sat_config_id: sat_config.get_untracked(),
            start: start.get_untracked(),
            end: end.get_untracked(),
            download_type: dl_type.get_untracked(),
            auto_create_payments: auto_pay.get_untracked(),
        };
        let cid = cid_dl.clone();
        async move {
            api::post_json(&format!("/api/admin/companies/{cid}/cfdi/download"), &payload).await
        }
    });

    let cid_poll = company_id.clone();
    Effect::new(move |_| {
        if let Some(r) = download.value().get() {
            match r {
                Ok(()) => {
                    dl_error.set(None);
                    busy.set(true);
                    poll_jobs(cid_poll.clone(), jobs, busy);
                }
                Err(ApiError::Forbidden) => dl_error.set(Some("No tienes permiso".into())),
                Err(_) => dl_error.set(Some("No se pudo iniciar la descarga".into())),
            }
        }
    });

    let start_download = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if sat_config.get().is_empty() {
            dl_error.set(Some("Selecciona una configuración SAT".into()));
            return;
        }
        dl_error.set(None);
        download.dispatch(());
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"CFDIs"</h1>

            {move || {
                if !is_admin {
                    return ().into_any();
                }
                view! {
                    <Card class="max-w-3xl">
                        <CardHeader>
                            <CardTitle>"Descargar del SAT"</CardTitle>
                        </CardHeader>
                        <CardContent>
                            {move || {
                                if configs.get().is_empty() {
                                    view! {
                                        <p class="text-sm text-amber-700">
                                            "No hay configuraciones SAT. Agrega una en Config. SAT para descargar."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <form on:submit=start_download class="grid gap-3 sm:grid-cols-2">
                                            <div class="space-y-1">
                                                <label class="block text-sm font-medium text-slate-700">
                                                    "Configuración SAT"
                                                </label>
                                                <Select value=sat_config>
                                                    {move || {
                                                        configs
                                                            .get()
                                                            .into_iter()
                                                            .map(|c| {
                                                                let label = c
                                                                    .label
                                                                    .filter(|l| !l.is_empty())
                                                                    .unwrap_or_else(|| c.rfc.clone());
                                                                view! {
                                                                    <option value=c.id>
                                                                        {format!("{} ({})", label, c.rfc)}
                                                                    </option>
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()
                                                    }}
                                                </Select>
                                            </div>
                                            <div class="space-y-1">
                                                <label class="block text-sm font-medium text-slate-700">
                                                    "Tipo"
                                                </label>
                                                <Select value=dl_type>
                                                    <option value="both">"Emitidos y recibidos"</option>
                                                    <option value="issued">"Solo emitidos"</option>
                                                    <option value="received">"Solo recibidos"</option>
                                                </Select>
                                            </div>
                                            <div class="space-y-1">
                                                <label class="block text-sm font-medium text-slate-700">
                                                    "Desde"
                                                </label>
                                                <Input
                                                    value=start
                                                    on_input=Callback::new(move |v| start.set(v))
                                                    r#type="date"
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
                                                />
                                            </div>
                                            <div class="flex items-center sm:col-span-2">
                                                <Checkbox
                                                    checked=auto_pay
                                                    label="Crear pagos automáticamente"
                                                />
                                            </div>
                                            <div class="flex items-center gap-2 sm:col-span-2">
                                                <Button r#type="submit" disabled=busy>
                                                    {move || {
                                                        if busy.get() { "Descargando…" } else { "Descargar CFDIs" }
                                                    }}
                                                </Button>
                                                {move || {
                                                    dl_error
                                                        .get()
                                                        .map(|m| view! { <span class="text-sm text-red-600">{m}</span> })
                                                }}
                                            </div>
                                        </form>
                                    }
                                        .into_any()
                                }
                            }}

                            {move || {
                                let list = jobs.get();
                                if list.is_empty() {
                                    return ().into_any();
                                }
                                view! {
                                    <div class="mt-4 overflow-x-auto">
                                        <p class="mb-1 text-sm font-medium text-slate-700">
                                            "Descargas"
                                        </p>
                                        <table class="w-full text-left text-xs">
                                            <thead class="text-slate-500">
                                                <tr>
                                                    <th class="py-1 font-medium">"Período"</th>
                                                    <th class="py-1 font-medium">"Estado"</th>
                                                    <th class="py-1 font-medium">"Encontrados"</th>
                                                    <th class="py-1 font-medium">"Creados"</th>
                                                    <th class="py-1 font-medium">"Actualizados"</th>
                                                    <th class="py-1 font-medium">"Omitidos"</th>
                                                    <th class="py-1 font-medium">"Errores"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {list
                                                    .into_iter()
                                                    .map(|j| {
                                                        let s = j.status;
                                                        let err_count = s.errors.len()
                                                            + s.error.as_ref().map_or(0, |_| 1);
                                                        let err_title = s
                                                            .error
                                                            .clone()
                                                            .into_iter()
                                                            .chain(s.errors.clone())
                                                            .collect::<Vec<_>>()
                                                            .join("\n");
                                                        view! {
                                                            <tr class="border-t border-slate-100">
                                                                <td class="py-1">{j.label}</td>
                                                                <td class="py-1">{status_badge(&s.status)}</td>
                                                                <td class="py-1">{s.imported}</td>
                                                                <td class="py-1">{s.transactions_created}</td>
                                                                <td class="py-1">{s.transactions_updated}</td>
                                                                <td class="py-1">{s.transactions_skipped}</td>
                                                                <td class="py-1" title=err_title>{err_count}</td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </tbody>
                                        </table>
                                    </div>
                                }
                                    .into_any()
                            }}
                        </CardContent>
                    </Card>
                }
                    .into_any()
            }}

            {move || {
                match data.get() {
                    Some(Ok(list)) if !list.items.is_empty() => cfdi_charts(&list.items).into_any(),
                    _ => ().into_any(),
                }
            }}

            {move || match data.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar los CFDIs."</p> }.into_any()
                }
                Some(Ok(list)) if list.items.is_empty() => {
                    view! { <p class="text-slate-500">"Sin CFDIs."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Folio"</th>
                                        <th class="px-4 py-2 font-medium">"Tipo"</th>
                                        <th class="px-4 py-2 font-medium">"Fecha"</th>
                                        <th class="px-4 py-2 font-medium">"Emisor"</th>
                                        <th class="px-4 py-2 font-medium">"Receptor"</th>
                                        <th class="px-4 py-2 font-medium">"Total"</th>
                                        <th class="px-4 py-2 font-medium">"Dirección"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .items
                                        .into_iter()
                                        .map(|c| {
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{c.folio}</td>
                                                    <td class="px-4 py-2">{c.tipo}</td>
                                                    <td class="px-4 py-2 text-slate-500">
                                                        {rfc3339_to_date(&c.fecha)}
                                                    </td>
                                                    <td class="px-4 py-2 text-slate-500">{c.emisor_nombre}</td>
                                                    <td class="px-4 py-2 text-slate-500">{c.receptor_nombre}</td>
                                                    <td class="px-4 py-2">
                                                        {format!("{} {}", money(c.total), c.moneda)}
                                                    </td>
                                                    <td class="px-4 py-2">
                                                        {if c.es_emitido { "Emitido" } else { "Recibido" }}
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
