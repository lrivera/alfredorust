use leptos::prelude::*;
use leptos::task::spawn_local;

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
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

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
                // Seed the selection from the cells already marked selected.
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
                    if is_admin {
                        view! {
                            <Button disabled=saving on:click=move |_| { save.dispatch(()); }>
                                {move || if saving.get() { "Guardando…" } else { "Guardar grid" }}
                            </Button>
                        }
                            .into_any()
                    } else {
                        ().into_any()
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
                    view! { <p class="text-slate-500">"Sin conceptos para esta fecha/estado."</p> }
                        .into_any()
                }
                Some(Ok(view)) => {
                    view! {
                        <div class="overflow-x-auto rounded-xl border border-slate-200 bg-white">
                            <table class="text-left text-xs">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="sticky left-0 z-10 bg-slate-50 px-3 py-2 font-medium">
                                            "Concepto"
                                        </th>
                                        {(0..24)
                                            .map(|h| {
                                                view! {
                                                    <th class="px-2 py-2 text-center font-medium">
                                                        {format!("{h:02}")}
                                                    </th>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </tr>
                                </thead>
                                <tbody>
                                    {view
                                        .rows
                                        .into_iter()
                                        .map(|row| {
                                            let concept_id = row.concept_id.clone();
                                            view! {
                                                <tr class="border-t border-slate-100 align-top">
                                                    <td class="sticky left-0 z-10 bg-white px-3 py-2 whitespace-nowrap">
                                                        <div class="font-medium">{row.concept_name}</div>
                                                        <div class="text-slate-400">
                                                            {row.project_title} " · " {row.status_name}
                                                        </div>
                                                    </td>
                                                    {row
                                                        .cells
                                                        .into_iter()
                                                        .map(|cell| {
                                                            let cid = concept_id.clone();
                                                            let bg = if cell.is_work_hour { "" } else { "bg-slate-50" };
                                                            view! {
                                                                <td class=format!("px-2 py-1 {bg}")>
                                                                    {cell
                                                                        .resources
                                                                        .into_iter()
                                                                        .map(|r| {
                                                                            let key: Key = (
                                                                                cid.clone(),
                                                                                cell.hour,
                                                                                r.resource_id.clone(),
                                                                            );
                                                                            let k2 = key.clone();
                                                                            view! {
                                                                                <label class="flex items-center gap-1 whitespace-nowrap">
                                                                                    <input
                                                                                        type="checkbox"
                                                                                        class="h-3 w-3 rounded border-slate-300"
                                                                                        prop:checked=move || selected.get().contains(&key)
                                                                                        on:change=move |_| toggle(k2.clone())
                                                                                    />
                                                                                    <span>{r.label}</span>
                                                                                </label>
                                                                            }
                                                                        })
                                                                        .collect::<Vec<_>>()}
                                                                </td>
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()}
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
