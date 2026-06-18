//! Hourly resource-usage grid. The table itself is the v1 markup rendered from
//! the JSON grid endpoint, enhanced by the v1 vanilla JS (long-press menu,
//! short-press cycle that copies the previous hour, live cell labels). The
//! date/status filter and the Save action stay in Leptos; Save reads the
//! checked boxes straight from the DOM and POSTs them as JSON.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use super::{run_script, set_html};
use crate::api::{self, ApiError, GridSavePayload, GridSelection, GridView, Me};
use crate::components::{Button, Input, Select};

fn today_str() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .map(|s| s.get(..10).unwrap_or(&s).to_string())
        .unwrap_or_default()
}

/// Minimal HTML-escape for text interpolated into the grid markup.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn money(n: f64) -> String {
    format!("{n:.2}")
}

/// Build the v1 grid table markup from the JSON view. Cells carry the same
/// `data-*` attributes and `cell_<concept>_<hour>_<resource>` checkbox names the
/// v1 JS and save logic expect.
fn build_grid_html(view: &GridView) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        r#"<div data-resource-usage-editable="{}" class="hidden"></div>"#,
        if view.can_edit { "true" } else { "false" }
    ));
    out.push_str(r#"<div class="overflow-x-auto rounded-3xl border border-slate-200 bg-white shadow-sm"><table class="min-w-[1200px] table-fixed border-collapse text-sm"><thead><tr class="bg-slate-50">"#);
    out.push_str(&format!(
        r#"<th class="sticky left-0 z-20 w-64 border-b border-r border-slate-200 bg-slate-50 px-4 py-3 text-left align-bottom"><div class="text-2xl font-black text-rose-600">{}</div><div class="mt-1 text-xs font-semibold uppercase tracking-wider text-slate-500">Concepto</div></th>"#,
        esc(&view.date)
    ));
    for h in 0..24 {
        let work = (7..=22).contains(&h);
        let cls = if work {
            "font-black text-slate-950"
        } else {
            "font-semibold text-slate-300 bg-slate-50"
        };
        out.push_str(&format!(
            r#"<th class="w-24 border-b border-r border-slate-200 px-2 py-3 text-center text-2xl {cls}">{h}</th>"#
        ));
    }
    out.push_str("</tr></thead><tbody>");

    if view.rows.is_empty() {
        out.push_str(r#"<tr><td colspan="25" class="px-6 py-10 text-center text-slate-500">No hay conceptos activos en este estado.</td></tr>"#);
    }

    let mut last_project = String::new();
    for row in &view.rows {
        if row.project_id != last_project {
            last_project = row.project_id.clone();
            out.push_str(&format!(
                r#"<tr class="bg-slate-100/80"><td colspan="25" class="border-b border-slate-200 px-4 py-2 text-xs font-semibold uppercase tracking-wide text-slate-500"><a href="/v2/projects/{}" class="text-sky-700 hover:text-sky-900 hover:underline">{}</a></td></tr>"#,
                esc(&row.project_id),
                esc(&row.project_title)
            ));
        }
        out.push_str(r#"<tr class="hover:bg-slate-50/60">"#);
        out.push_str(&format!(
            r#"<th class="sticky left-0 z-10 border-b border-r border-slate-200 bg-white px-4 py-4 text-left align-middle"><div class="text-xs font-semibold uppercase tracking-wider text-slate-400"><a href="/v2/projects/{pid}" class="hover:text-sky-700 hover:underline">{ptitle}</a></div><div class="mt-1 flex flex-wrap items-center gap-2"><span class="font-semibold text-sky-900">{cname}</span><span class="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-slate-500">{status}</span></div><div class="mt-1 text-xs text-slate-500">{qty} {unit}</div></th>"#,
            pid = esc(&row.project_id),
            ptitle = esc(&row.project_title),
            cname = esc(&row.concept_name),
            status = esc(&row.status_name),
            qty = money(row.quantity),
            unit = esc(&row.unit),
        ));
        for cell in &row.cells {
            let cell_bg = if cell.is_work_hour { "" } else { "bg-slate-50/70" };
            let btn_cls = if cell.is_work_hour {
                "font-semibold text-slate-700"
            } else {
                "font-medium text-slate-400"
            };
            let labels: Vec<String> = cell
                .resources
                .iter()
                .filter(|r| r.selected)
                .map(|r| esc(&r.label))
                .collect();
            let label_text = if labels.is_empty() {
                "+".to_string()
            } else {
                labels.join(", ")
            };
            out.push_str(&format!(
                r#"<td class="h-20 border-b border-r border-slate-200 p-1 align-top {cell_bg}"><div class="resource-cell group relative h-full rounded-xl transition" data-resource-cell><button type="button" class="resource-cell-button flex h-full w-full cursor-pointer items-center justify-center rounded-xl px-1 text-center text-xs {btn_cls} hover:bg-slate-100" data-resource-cell-button><span data-resource-cell-label>{label_text}</span></button><div class="resource-cell-menu absolute left-0 top-full z-30 mt-1 hidden max-h-48 min-w-44 space-y-1 overflow-y-auto rounded-xl border border-slate-200 bg-white p-2 shadow-xl" data-resource-cell-menu>"#
            ));
            if cell.resources.is_empty() {
                out.push_str(r#"<div class="px-2 py-1 text-xs text-slate-400">Sin recursos</div>"#);
            } else {
                for r in &cell.resources {
                    let name = format!("cell_{}_{}_{}", row.concept_id, cell.hour, r.resource_id);
                    out.push_str(&format!(
                        r#"<label class="flex items-center gap-2 rounded-lg px-2 py-1 text-xs hover:bg-slate-50"><input type="checkbox" name="{name}" data-resource-id="{rid}" data-resource-label="{label}" {checked} {disabled} class="rounded border-slate-300 text-sky-600 focus:ring-sky-500"><span>{label}</span></label>"#,
                        name = esc(&name),
                        rid = esc(&r.resource_id),
                        label = esc(&r.label),
                        checked = if r.selected { "checked" } else { "" },
                        disabled = if view.can_edit { "" } else { "disabled" },
                    ));
                }
            }
            out.push_str("</div></div></td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    out
}

const GRID_JS: &str = r##"
  (() => {
    const LONG_PRESS_MS = 450;
    const updateCellLabel = (cell) => {
      const label = cell.querySelector("[data-resource-cell-label]");
      const checked = Array.from(cell.querySelectorAll("input[type='checkbox']:checked"));
      if (!label) return;
      label.textContent = checked.length ? checked.map((i) => i.dataset.resourceLabel || "").filter(Boolean).join(", ") : "+";
      label.classList.toggle("text-slate-300", checked.length === 0);
      cell.classList.toggle("bg-sky-50", checked.length > 0);
    };
    const closeMenus = (except) => {
      document.querySelectorAll("[data-resource-cell-menu]").forEach((menu) => { if (menu !== except) menu.classList.add("hidden"); });
    };
    const cycleSingleResource = (cell) => {
      const inputs = Array.from(cell.querySelectorAll("input[type='checkbox']"));
      if (!inputs.length) return;
      const currentIndex = inputs.findIndex((i) => i.checked);
      inputs.forEach((i) => { i.checked = false; });
      let nextIndex = currentIndex + 1;
      if (currentIndex === -1) {
        const previousCell = cell.closest("td")?.previousElementSibling?.querySelector("[data-resource-cell]");
        const previousResourceIds = new Set(Array.from(previousCell?.querySelectorAll("input[type='checkbox']:checked") || []).map((i) => i.dataset.resourceId).filter(Boolean));
        if (previousResourceIds.size > 0) {
          let copied = false;
          inputs.forEach((i) => { if (previousResourceIds.has(i.dataset.resourceId)) { i.checked = true; copied = true; } });
          updateCellLabel(cell);
          if (copied) return;
        }
        nextIndex = 0;
      }
      if (nextIndex < inputs.length) { inputs[nextIndex].checked = true; }
      updateCellLabel(cell);
    };
    const editable = document.querySelector("[data-resource-usage-editable]")?.dataset.resourceUsageEditable === "true";
    document.querySelectorAll("[data-resource-cell]").forEach((cell) => {
      const button = cell.querySelector("[data-resource-cell-button]");
      const menu = cell.querySelector("[data-resource-cell-menu]");
      let timer = null;
      let longPressed = false;
      updateCellLabel(cell);
      if (!editable) return;
      button?.addEventListener("pointerdown", (event) => {
        longPressed = false;
        timer = window.setTimeout(() => { longPressed = true; closeMenus(menu); menu?.classList.toggle("hidden"); }, LONG_PRESS_MS);
        event.preventDefault();
      });
      button?.addEventListener("pointerup", (event) => {
        if (timer) window.clearTimeout(timer);
        if (!longPressed) { closeMenus(null); cycleSingleResource(cell); }
        event.preventDefault();
      });
      button?.addEventListener("pointerleave", () => { if (timer) window.clearTimeout(timer); });
      cell.querySelectorAll("input[type='checkbox']").forEach((input) => { input.addEventListener("change", () => updateCellLabel(cell)); });
    });
    document.addEventListener("pointerdown", (event) => {
      if (!(event.target instanceof Element)) return;
      if (!event.target.closest("[data-resource-cell]")) { closeMenus(null); }
    });
  })();
"##;

#[component]
pub fn ResourceUsagesPage() -> impl IntoView {
    let _me = use_context::<Me>().expect("Me context");

    let date = RwSignal::new(today_str());
    let status = RwSignal::new("all".to_string());
    let meta = RwSignal::new(None::<GridView>);
    let saved_msg = RwSignal::new(None::<String>);
    let reload = RwSignal::new(0u32);
    let host = NodeRef::<leptos::html::Div>::new();

    // Fetch + render whenever the host is ready or a reload is requested.
    Effect::new(move |_| {
        reload.track();
        let Some(host_el) = host.get() else { return };
        let d = date.get_untracked();
        let s = status.get_untracked();
        let url = format!("/api/admin/resource_usages/grid?date={d}&status_id={s}");
        set_html(&host_el, r#"<p class="text-slate-500">Cargando…</p>"#);
        spawn_local(async move {
            match api::get_json::<GridView>(&url).await {
                Ok(view) => {
                    set_html(&host_el, &build_grid_html(&view));
                    run_script(GRID_JS);
                    meta.set(Some(view));
                }
                Err(_) => {
                    set_html(&host_el, r#"<p class="text-red-600">No se pudo cargar el grid.</p>"#);
                    meta.set(None);
                }
            }
        });
    });

    let save = Action::new_local(move |_: &()| {
        // Read the checked boxes straight from the rendered grid.
        let mut selections = Vec::new();
        if let Some(host_el) = host.get_untracked() {
            if let Ok(nodes) = host_el.query_selector_all("input[type='checkbox']:checked") {
                for i in 0..nodes.length() {
                    if let Some(node) = nodes.item(i) {
                        if let Ok(input) = node.dyn_into::<web_sys::HtmlInputElement>() {
                            if let Some(name) = input.get_attribute("name") {
                                if let Some(rest) = name.strip_prefix("cell_") {
                                    let parts: Vec<&str> = rest.splitn(3, '_').collect();
                                    if parts.len() == 3 {
                                        if let Ok(hour) = parts[1].parse::<i32>() {
                                            selections.push(GridSelection {
                                                concept_id: parts[0].to_string(),
                                                hour,
                                                resource_id: parts[2].to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let s = status.get_untracked();
        let payload = GridSavePayload {
            date: date.get_untracked(),
            status_id: (s != "all").then_some(s),
            selections,
        };
        async move { api::post_json("/api/admin/resource_usages/grid", &payload).await }
    });

    Effect::new(move |_| {
        if let Some(r) = save.value().get() {
            match r {
                Ok(()) => {
                    saved_msg.set(Some("Guardado".into()));
                    reload.update(|n| *n += 1);
                }
                Err(ApiError::Forbidden) => saved_msg.set(Some("No tienes permiso".into())),
                Err(_) => saved_msg.set(Some("No se pudo guardar".into())),
            }
        }
    });
    let saving = save.pending();

    let apply = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        saved_msg.set(None);
        reload.update(|n| *n += 1);
    };

    view! {
        <div class="space-y-6">
            <div class="rounded-3xl bg-slate-950 p-5 text-white shadow-sm md:p-6">
                <p class="text-xs font-semibold uppercase tracking-[0.25em] text-emerald-300">
                    "Captura diaria"
                </p>
                <h1 class="mt-2 text-3xl font-semibold">"Uso de recursos por hora"</h1>
                <p class="mt-2 max-w-3xl text-sm text-slate-300">
                    "Filtra por estado, selecciona recursos en cada hora (toca para ciclar, mantén para abrir el menú). Horario normal 7-22; las demás horas también se capturan."
                </p>
                <form on:submit=apply class="mt-4 flex flex-wrap items-end gap-3">
                    <div>
                        <label class="block text-xs font-semibold text-slate-300">"Fecha"</label>
                        <Input value=date on_input=Callback::new(move |v| date.set(v)) r#type="date" />
                    </div>
                    <div>
                        <label class="block text-xs font-semibold text-slate-300">"Estado"</label>
                        <Select value=status>
                            <option value="all">"Todos"</option>
                            {move || {
                                meta.get()
                                    .map(|v| {
                                        v.statuses
                                            .into_iter()
                                            .map(|s| view! { <option value=s.id>{s.name}</option> })
                                            .collect::<Vec<_>>()
                                    })
                            }}
                        </Select>
                    </div>
                    <Button r#type="submit">"Filtrar"</Button>
                    {move || {
                        let editable = meta.get().map(|v| v.can_edit).unwrap_or(false);
                        if editable {
                            view! {
                                <Button disabled=saving on:click=move |_| { save.dispatch(()); }>
                                    {move || if saving.get() { "Guardando…" } else { "Guardar captura" }}
                                </Button>
                            }
                                .into_any()
                        } else {
                            view! {
                                <span class="rounded-full bg-slate-100 px-4 py-2 text-sm font-semibold text-slate-500">
                                    "Solo lectura"
                                </span>
                            }
                                .into_any()
                        }
                    }}
                    {move || {
                        saved_msg
                            .get()
                            .map(|m| view! { <span class="text-sm text-emerald-300">{m}</span> })
                    }}
                </form>
            </div>

            <div node_ref=host></div>
        </div>
    }
}
