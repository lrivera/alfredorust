//! Hourly resource-usage grid: rows are project concepts (grouped by project),
//! columns are the 24 hours of a day, and each cell holds the resources
//! assigned to that concept at that hour. Mirrors the v1 grid — work-hour
//! emphasis, per-cell multi-resource selection (via a dropdown, the SPA's
//! equivalent of v1's long-press menu), a read-only state when the date is
//! outside the staff edit window, and a status filter.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::money;
use crate::api::{self, ApiError, GridSavePayload, GridSelection, GridView, Me};
use crate::components::{Button, Input, Select};

fn today_str() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .map(|s| s.get(..10).unwrap_or(&s).to_string())
        .unwrap_or_default()
}

type Key = (String, i32, String);

#[component]
pub fn ResourceUsagesPage() -> impl IntoView {
    // Edit affordances follow the API's `can_edit` (it already accounts for the
    // staff "edit today" window); admin role isn't sufficient on its own here.
    let _me = use_context::<Me>().expect("Me context");

    let date = RwSignal::new(today_str());
    let status = RwSignal::new("all".to_string());
    let grid = RwSignal::new(None::<Result<GridView, ApiError>>);
    let selected = RwSignal::new(Vec::<Key>::new());
    let saved_msg = RwSignal::new(None::<String>);

    let load = move || {
        let d = date.get_untracked();
        let s = status.get_untracked();
        let url = format!("/api/admin/resource_usages/grid?date={d}&status_id={s}");
        grid.set(None);
        saved_msg.set(None);
        spawn_local(async move {
            let result = api::get_json::<GridView>(&url).await;
            if let Ok(view) = &result {
                let mut sel = Vec::new();
                for row in &view.rows {
                    for cell in &row.cells {
                        for r in &cell.resources {
                            if r.selected {
                                sel.push((row.concept_id.clone(), cell.hour, r.resource_id.clone()));
                            }
                        }
                    }
                }
                selected.set(sel);
            }
            grid.set(Some(result));
        });
    };
    load();

    let toggle = move |key: Key| {
        selected.update(|v| {
            if let Some(p) = v.iter().position(|k| k == &key) {
                v.remove(p);
            } else {
                v.push(key);
            }
        });
    };

    let save = Action::new_local(move |_: &()| {
        let s = status.get_untracked();
        let payload = GridSavePayload {
            date: date.get_untracked(),
            status_id: (s != "all").then_some(s),
            selections: selected
                .get_untracked()
                .into_iter()
                .map(|(concept_id, hour, resource_id)| GridSelection { concept_id, hour, resource_id })
                .collect(),
        };
        async move { api::post_json("/api/admin/resource_usages/grid", &payload).await }
    });

    Effect::new(move |_| {
        if let Some(r) = save.value().get() {
            match r {
                Ok(()) => {
                    saved_msg.set(Some("Guardado".into()));
                    load();
                }
                Err(ApiError::Forbidden) => saved_msg.set(Some("No tienes permiso".into())),
                Err(_) => saved_msg.set(Some("No se pudo guardar".into())),
            }
        }
    });
    let saving = save.pending();

    let apply = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        load();
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Uso de recursos (grid)"</h1>

            <form on:submit=apply class="flex flex-wrap items-end gap-3">
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Fecha"</label>
                    <Input value=date on_input=Callback::new(move |v| date.set(v)) r#type="date" />
                </div>
                <div class="space-y-1">
                    <label class="block text-sm font-medium text-slate-700">"Estado"</label>
                    <Select value=status>
                        <option value="all">"Todos"</option>
                        {move || {
                            grid.get()
                                .and_then(|r| r.ok())
                                .map(|v| {
                                    v.statuses
                                        .into_iter()
                                        .map(|s| view! { <option value=s.id>{s.name}</option> })
                                        .collect::<Vec<_>>()
                                })
                        }}
                    </Select>
                </div>
                <Button r#type="submit">"Cargar"</Button>
                {move || {
                    let editable = grid.get().and_then(|r| r.ok()).map(|v| v.can_edit).unwrap_or(false);
                    if editable {
                        view! {
                            <Button disabled=saving on:click=move |_| { save.dispatch(()); }>
                                {move || if saving.get() { "Guardando…" } else { "Guardar captura" }}
                            </Button>
                        }
                            .into_any()
                    } else {
                        view! {
                            <span class="rounded bg-slate-100 px-2 py-1 text-sm text-slate-500">
                                "Solo lectura"
                            </span>
                        }
                            .into_any()
                    }
                }}
                {move || {
                    saved_msg.get().map(|m| view! { <span class="text-sm text-emerald-700">{m}</span> })
                }}
            </form>

            {move || match grid.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudo cargar el grid."</p> }.into_any()
                }
                Some(Ok(view)) if view.rows.is_empty() => {
                    view! {
                        <p class="text-slate-500">"No hay conceptos activos para esta fecha/estado."</p>
                    }
                        .into_any()
                }
                Some(Ok(view)) => {
                    let editable = view.can_edit;
                    view! {
                        <div class="overflow-x-auto rounded-xl border border-slate-200 bg-white">
                            <table class="min-w-[1200px] text-left text-xs">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="sticky left-0 z-10 bg-slate-50 px-3 py-2 font-medium">
                                            "Concepto"
                                        </th>
                                        {(0..24)
                                            .map(|h| {
                                                let work = (7..=22).contains(&h);
                                                let cls = if work {
                                                    "px-2 py-2 text-center font-black text-slate-950"
                                                } else {
                                                    "px-2 py-2 text-center font-medium text-slate-300 bg-slate-50"
                                                };
                                                view! { <th class=cls>{format!("{h:02}")}</th> }
                                            })
                                            .collect::<Vec<_>>()}
                                    </tr>
                                </thead>
                                <tbody>{grid_rows(view.rows, selected, toggle, editable)}</tbody>
                            </table>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

/// Build the table body: a project header row before each new project group,
/// then one row per concept.
fn grid_rows(
    rows: Vec<api::GridRow>,
    selected: RwSignal<Vec<Key>>,
    toggle: impl Fn(Key) + Copy + 'static,
    editable: bool,
) -> Vec<AnyView> {
    let mut out = Vec::new();
    let mut last_project = String::new();
    for row in rows {
        if row.project_id != last_project {
            last_project = row.project_id.clone();
            let pid = row.project_id.clone();
            let title = row.project_title.clone();
            out.push(
                view! {
                    <tr class="bg-slate-100/80">
                        <td colspan="25" class="px-3 py-1">
                            <a
                                href=format!("/v2/projects/{pid}")
                                class="font-semibold text-sky-700 hover:underline"
                            >
                                {title}
                            </a>
                        </td>
                    </tr>
                }
                .into_any(),
            );
        }
        out.push(concept_row(row, selected, toggle, editable));
    }
    out
}

/// One concept row: a sticky label cell (concept + status + quantity) and 24
/// hour cells, each a dropdown of allowed resources with the selected ones
/// summarized in the cell.
fn concept_row(
    row: api::GridRow,
    selected: RwSignal<Vec<Key>>,
    toggle: impl Fn(Key) + Copy + 'static,
    editable: bool,
) -> AnyView {
    let concept_id = row.concept_id.clone();
    let qty = format!("{} {}", money(row.quantity), row.unit);
    view! {
        <tr class="border-t border-slate-100 align-top">
            <td class="sticky left-0 z-10 bg-white px-3 py-2 whitespace-nowrap">
                <div class="font-medium">{row.concept_name}</div>
                <div class="text-slate-400">{row.status_name} " · " {qty}</div>
            </td>
            {row
                .cells
                .into_iter()
                .map(|cell| {
                    let cid = concept_id.clone();
                    let hour = cell.hour;
                    let resources = cell.resources.clone();
                    let res_for_summary = resources.clone();
                    let cid_sum = cid.clone();

                    // Reactive summary of the cell: comma-joined selected labels,
                    // or "+" when empty.
                    let summary = move || {
                        let sel = selected.get();
                        let chosen: Vec<String> = res_for_summary
                            .iter()
                            .filter(|r| {
                                sel.contains(&(cid_sum.clone(), hour, r.resource_id.clone()))
                            })
                            .map(|r| r.label.clone())
                            .collect();
                        if chosen.is_empty() { "+".to_string() } else { chosen.join(", ") }
                    };
                    let has_sel = {
                        let res = resources.clone();
                        let cid_h = cid.clone();
                        move || {
                            let sel = selected.get();
                            res.iter()
                                .any(|r| sel.contains(&(cid_h.clone(), hour, r.resource_id.clone())))
                        }
                    };
                    let work = cell.is_work_hour;
                    let td_cls = move || {
                        let bg = if has_sel() {
                            "bg-sky-50"
                        } else if work {
                            ""
                        } else {
                            "bg-slate-50/70"
                        };
                        format!("px-1 py-1 align-top {bg}")
                    };

                    view! {
                        <td class=td_cls>
                            {if resources.is_empty() {
                                view! { <span class="text-slate-300">"—"</span> }.into_any()
                            } else {
                                view! {
                                    <details class="group">
                                        <summary class="cursor-pointer list-none whitespace-nowrap text-slate-700">
                                            {summary}
                                        </summary>
                                        <div class="mt-1 space-y-1 rounded border border-slate-200 bg-white p-1 shadow">
                                            {resources
                                                .into_iter()
                                                .map(|r| {
                                                    let key: Key = (cid.clone(), hour, r.resource_id.clone());
                                                    let k2 = key.clone();
                                                    view! {
                                                        <label class="flex items-center gap-1 whitespace-nowrap">
                                                            <input
                                                                type="checkbox"
                                                                class="h-3 w-3 rounded border-slate-300"
                                                                prop:checked=move || selected.get().contains(&key)
                                                                prop:disabled=!editable
                                                                on:change=move |_| toggle(k2.clone())
                                                            />
                                                            <span>{r.label}</span>
                                                        </label>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </details>
                                }
                                    .into_any()
                            }}
                        </td>
                    }
                })
                .collect::<Vec<_>>()}
        </tr>
    }
    .into_any()
}
