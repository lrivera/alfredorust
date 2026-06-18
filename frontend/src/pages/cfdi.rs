use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{money, rfc3339_to_date};
use crate::api::{self, ApiError, CfdiList, Me};

#[component]
pub fn CfdiPage() -> impl IntoView {
    // Read-only; the ViewCfdi/admin permission is enforced server-side.
    let _me = use_context::<Me>();

    let data = RwSignal::new(None::<Result<CfdiList, ApiError>>);
    let reload = move || {
        data.set(None);
        spawn_local(async move {
            data.set(Some(api::get_json::<CfdiList>("/api/admin/cfdis/data").await));
        });
    };
    reload();

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"CFDIs"</h1>

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
