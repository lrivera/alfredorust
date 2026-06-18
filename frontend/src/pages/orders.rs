use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, load_account_options, load_category_options, load_contact_options, money,
    rfc3339_to_date, Options,
};
use crate::api::{self, ApiError, Me, Order, OrderItem, OrderPayload};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

const STATUSES: &[(&str, &str)] = &[
    ("pending", "Pendiente"),
    ("confirmed", "Confirmada"),
    ("in_progress", "En proceso"),
    ("completed", "Completada"),
    ("cancelled", "Cancelada"),
];

/// One editable order line; per-field signals keep focus stable under <For>.
#[derive(Clone)]
struct ItemRow {
    id: usize,
    desc: RwSignal<String>,
    qty: RwSignal<String>,
    price: RwSignal<String>,
}

#[component]
pub fn OrdersPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items_data = RwSignal::new(None::<Result<Vec<Order>, ApiError>>);
    let reload = move || {
        items_data.set(None);
        spawn_local(async move {
            items_data.set(Some(api::get_json::<Vec<Order>>("/api/admin/orders").await));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let accounts = RwSignal::new(Options::new());
    let contacts = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);
    load_contact_options(contacts);

    let editing = RwSignal::new(None::<String>);
    let title = RwSignal::new(String::new());
    let contact = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let account = RwSignal::new(String::new());
    let status = RwSignal::new("pending".to_string());
    let amount = RwSignal::new(String::new());
    let scheduled = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let lines = RwSignal::new(Vec::<ItemRow>::new());
    let line_seq = RwSignal::new(0usize);
    let form_error = RwSignal::new(None::<String>);

    let add_line = move || {
        let id = line_seq.get_untracked();
        line_seq.set(id + 1);
        lines.update(|v| {
            v.push(ItemRow {
                id,
                desc: RwSignal::new(String::new()),
                qty: RwSignal::new("1".to_string()),
                price: RwSignal::new(String::new()),
            })
        });
    };
    let remove_line = move |id: usize| lines.update(|v| v.retain(|r| r.id != id));

    let reset_form = move || {
        editing.set(None);
        title.set(String::new());
        contact.set(String::new());
        category.set(String::new());
        account.set(String::new());
        status.set("pending".to_string());
        amount.set(String::new());
        scheduled.set(String::new());
        notes.set(String::new());
        lines.set(Vec::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let opt = |s: String| (!s.trim().is_empty()).then_some(s);
        let sched = scheduled.get_untracked();
        let items: Vec<OrderItem> = lines
            .get_untracked()
            .into_iter()
            .filter_map(|r| {
                let description = r.desc.get_untracked().trim().to_string();
                (!description.is_empty()).then(|| OrderItem {
                    description,
                    quantity: r.qty.get_untracked().trim().parse().unwrap_or(0.0),
                    unit_price: r.price.get_untracked().trim().parse().unwrap_or(0.0),
                })
            })
            .collect();
        let payload = OrderPayload {
            title: title.get_untracked().trim().to_string(),
            contact_id: opt(contact.get_untracked()),
            category_id: opt(category.get_untracked()),
            account_id: opt(account.get_untracked()),
            status: status.get_untracked(),
            amount: amount.get_untracked().trim().parse().unwrap_or(0.0),
            scheduled_at: (!sched.trim().is_empty()).then(|| date_to_rfc3339(&sched)),
            items,
            notes: opt(notes.get_untracked()),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::post_json(&format!("/api/admin/orders/{id}/update"), &payload).await,
                None => api::post_json("/api/admin/orders", &payload).await,
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
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar la orden"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if title.get().trim().is_empty() {
            form_error.set(Some("El título es obligatorio".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) = api::get_json::<api::OrderDetail>(&format!("/api/admin/orders/{id}")).await
            {
                title.set(d.title);
                contact.set(d.contact_id.unwrap_or_default());
                category.set(d.category_id.unwrap_or_default());
                account.set(d.account_id.unwrap_or_default());
                status.set(d.status);
                amount.set(format!("{}", d.amount));
                scheduled.set(d.scheduled_at.as_deref().map(rfc3339_to_date).unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                let mut seq = 0usize;
                let rows = d
                    .items
                    .into_iter()
                    .map(|it| {
                        let id = seq;
                        seq += 1;
                        ItemRow {
                            id,
                            desc: RwSignal::new(it.description),
                            qty: RwSignal::new(it.quantity.to_string()),
                            price: RwSignal::new(it.unit_price.to_string()),
                        }
                    })
                    .collect::<Vec<_>>();
                line_seq.set(seq);
                lines.set(rows);
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/orders/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };
    let complete = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/orders/{id}/complete")).await.is_ok() {
                reload();
            }
        });
    };

    // Resolve a contact id to its name for the list (contacts are loaded for the form).
    let contact_name = move |cid: &Option<String>| {
        cid.as_ref()
            .and_then(|id| contacts.get().into_iter().find(|(cid, _)| cid == id).map(|(_, n)| n))
            .unwrap_or_default()
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Órdenes de servicio"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() { "Editar orden" } else { "Nueva orden" }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1 sm:col-span-2">
                                        <label class="block text-sm font-medium text-foreground">"Título"</label>
                                        <Input
                                            value=title
                                            on_input=Callback::new(move |v| title.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">"Estado"</label>
                                        <Select value=status>
                                            {STATUSES
                                                .iter()
                                                .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Cliente (opcional)"
                                        </label>
                                        <Select value=contact>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                contacts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Categoría (opcional)"
                                        </label>
                                        <Select value=category>
                                            <option value="">"— Ninguna —"</option>
                                            {move || {
                                                categories
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Cuenta destino (opcional)"
                                        </label>
                                        <Select value=account>
                                            <option value="">"— Ninguna —"</option>
                                            {move || {
                                                accounts
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Monto total"
                                        </label>
                                        <Input
                                            value=amount
                                            on_input=Callback::new(move |v| amount.set(v))
                                            inputmode="decimal"
                                            placeholder="0.00"
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-foreground">
                                            "Fecha (cita/entrega)"
                                        </label>
                                        <Input
                                            value=scheduled
                                            on_input=Callback::new(move |v| scheduled.set(v))
                                            r#type="date"
                                        />
                                    </div>

                                    // Line items.
                                    <div class="space-y-2 sm:col-span-3">
                                        <div class="flex items-center justify-between">
                                            <label class="text-sm font-medium text-foreground">"Conceptos"</label>
                                            <Button variant=ButtonVariant::Outline on:click=move |_| add_line()>
                                                "+ Agregar línea"
                                            </Button>
                                        </div>
                                        <For each=move || lines.get() key=|r| r.id let:row>
                                            <div class="grid grid-cols-12 gap-2">
                                                <div class="col-span-6">
                                                    <Input
                                                        value=row.desc
                                                        on_input=Callback::new(move |v| row.desc.set(v))
                                                        placeholder="Descripción"
                                                    />
                                                </div>
                                                <div class="col-span-2">
                                                    <Input
                                                        value=row.qty
                                                        on_input=Callback::new(move |v| row.qty.set(v))
                                                        inputmode="decimal"
                                                        placeholder="Cant."
                                                    />
                                                </div>
                                                <div class="col-span-3">
                                                    <Input
                                                        value=row.price
                                                        on_input=Callback::new(move |v| row.price.set(v))
                                                        inputmode="decimal"
                                                        placeholder="P. unit."
                                                    />
                                                </div>
                                                <div class="col-span-1 flex items-center">
                                                    <Button
                                                        variant=ButtonVariant::Ghost
                                                        class="text-red-600 hover:bg-red-50"
                                                        on:click=move |_| remove_line(row.id)
                                                    >
                                                        "✕"
                                                    </Button>
                                                </div>
                                            </div>
                                        </For>
                                    </div>

                                    <div class="space-y-1 sm:col-span-3">
                                        <label class="block text-sm font-medium text-foreground">"Notas"</label>
                                        <Input value=notes on_input=Callback::new(move |v| notes.set(v)) />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear orden"
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
                                        {move || {
                                            form_error
                                                .get()
                                                .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                                        }}
                                    </div>
                                </form>
                            </CardContent>
                        </Card>
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}

            {move || match items_data.get() {
                None => view! { <p class="text-muted-foreground">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las órdenes."</p> }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-muted-foreground">"Sin órdenes todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-border bg-card">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-muted text-muted-foreground">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Título"</th>
                                        <th class="px-4 py-2 font-medium">"Cliente"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2 font-medium">"Monto"</th>
                                        <th class="px-4 py-2 font-medium">"Fecha"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|o| {
                                            let id = o.id.clone();
                                            let st = if o.status_label.is_empty() {
                                                o.status.clone()
                                            } else {
                                                o.status_label.clone()
                                            };
                                            let cid = o.contact_id.clone();
                                            let sched = o.scheduled_at.as_deref().map(rfc3339_to_date).unwrap_or_default();
                                            view! {
                                                <tr class="border-t border-border">
                                                    <td class="px-4 py-2">{o.title}</td>
                                                    <td class="px-4 py-2 text-muted-foreground">
                                                        {move || contact_name(&cid)}
                                                    </td>
                                                    <td class="px-4 py-2">{st}</td>
                                                    <td class="px-4 py-2">{money(o.amount)}</td>
                                                    <td class="px-4 py-2 text-muted-foreground">{sched}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let cmpid = id.clone();
                                                                let eid = id.clone();
                                                                let did = id.clone();
                                                                view! {
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        class="text-emerald-700 hover:bg-emerald-50"
                                                                        on:click=move |_| complete(cmpid.clone())
                                                                    >
                                                                        "Completar"
                                                                    </Button>
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
