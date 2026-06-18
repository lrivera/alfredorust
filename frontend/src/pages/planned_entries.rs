use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    date_to_rfc3339, flow_label, load_account_options, load_category_options, load_contact_options,
    load_project_options, money, rfc3339_to_date, Options,
};
use crate::api::{
    self, ApiError, Me, PlannedEntry, PlannedEntryBulkPayPayload, PlannedEntryPayPayload,
    PlannedEntryPayload,
};
use crate::components::{Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Input, Select};

const STATUSES: &[(&str, &str)] = &[
    ("planned", "Planificado"),
    ("partially_covered", "Parcial"),
    ("covered", "Cubierto"),
    ("overdue", "Vencido"),
    ("cancelled", "Cancelado"),
];

#[component]
pub fn PlannedEntriesPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<PlannedEntry>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(
                api::get_json::<Vec<PlannedEntry>>("/api/admin/planned-entries").await,
            ));
        });
    };
    reload();

    let categories = RwSignal::new(Options::new());
    let accounts = RwSignal::new(Options::new());
    let contacts = RwSignal::new(Options::new());
    let projects = RwSignal::new(Options::new());
    load_category_options(categories);
    load_account_options(accounts);
    load_contact_options(contacts);
    load_project_options(projects);

    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let flow = RwSignal::new("expense".to_string());
    let category = RwSignal::new(String::new());
    let account = RwSignal::new(String::new());
    let contact = RwSignal::new(String::new());
    let project = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let due = RwSignal::new(String::new());
    let status = RwSignal::new("planned".to_string());
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        flow.set("expense".to_string());
        category.set(String::new());
        account.set(String::new());
        contact.set(String::new());
        project.set(String::new());
        amount.set(String::new());
        due.set(String::new());
        status.set("planned".to_string());
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let contact_val = contact.get_untracked();
        let project_val = project.get_untracked();
        let notes_val = notes.get_untracked();
        let payload = PlannedEntryPayload {
            name: name.get_untracked().trim().to_string(),
            flow_type: flow.get_untracked(),
            category_id: category.get_untracked(),
            account_expected_id: account.get_untracked(),
            contact_id: (!contact_val.is_empty()).then_some(contact_val),
            amount_estimated: amount.get_untracked().trim().parse().unwrap_or(0.0),
            due_date: date_to_rfc3339(&due.get_untracked()),
            status: status.get_untracked(),
            project_id: (!project_val.is_empty()).then_some(project_val),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => {
                    api::post_json(&format!("/api/admin/planned-entries/{id}/update"), &payload)
                        .await
                }
                None => api::post_json("/api/admin/planned-entries", &payload).await,
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
                Err(_) => form_error.set(Some("No se pudo guardar la entrada".into())),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if name.get().trim().is_empty()
            || category.get().is_empty()
            || account.get().is_empty()
            || due.get().trim().is_empty()
        {
            form_error.set(Some("Completa nombre, categoría, cuenta y vencimiento".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };
    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) = api::get_json::<api::PlannedEntryDetail>(&format!(
                "/api/admin/planned-entries/{id}"
            ))
            .await
            {
                name.set(d.name);
                flow.set(d.flow_type);
                category.set(d.category_id);
                account.set(d.account_expected_id);
                amount.set(format!("{}", d.amount_estimated));
                due.set(rfc3339_to_date(&d.due_date));
                status.set(d.status);
                contact.set(d.contact_id.unwrap_or_default());
                project.set(d.project_id.unwrap_or_default());
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };
    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::post_empty(&format!("/api/admin/planned-entries/{id}/delete")).await.is_ok() {
                reload();
            }
        });
    };

    // --- pay flow: pay a single planned entry ----------------------------
    let paying = RwSignal::new(None::<String>);
    let pay_date = RwSignal::new(String::new());
    let pay_amount = RwSignal::new(String::new());
    let pay_account = RwSignal::new(String::new());
    let pay_notes = RwSignal::new(String::new());
    let pay_error = RwSignal::new(None::<String>);

    let begin_pay = move |id: String| {
        paying.set(Some(id));
        pay_date.set(String::new());
        pay_amount.set(String::new());
        pay_account.set(String::new());
        pay_notes.set(String::new());
        pay_error.set(None);
    };
    let cancel_pay = move || paying.set(None);

    let pay_action = Action::new_local(move |_: &()| {
        let id = paying.get_untracked().unwrap_or_default();
        let notes_val = pay_notes.get_untracked();
        let payload = PlannedEntryPayPayload {
            paid_at: date_to_rfc3339(&pay_date.get_untracked()),
            amount: pay_amount.get_untracked().trim().parse().unwrap_or(0.0),
            account_id: pay_account.get_untracked(),
            project_id: None,
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        async move { api::post_json(&format!("/api/admin/planned-entries/{id}/pay"), &payload).await }
    });

    Effect::new(move |_| {
        if let Some(r) = pay_action.value().get() {
            match r {
                Ok(()) => {
                    paying.set(None);
                    reload();
                }
                Err(ApiError::Forbidden) => pay_error.set(Some("No tienes permiso".into())),
                Err(_) => pay_error.set(Some("No se pudo registrar el pago".into())),
            }
        }
    });

    let pay_pending = pay_action.pending();
    let pay_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if pay_date.get().trim().is_empty() || pay_account.get().is_empty() {
            pay_error.set(Some("Completa fecha y cuenta".into()));
            return;
        }
        pay_error.set(None);
        pay_action.dispatch(());
    };

    // --- bulk pay: select several entries and pay them together ----------
    let selected = RwSignal::new(Vec::<String>::new());
    let toggle_sel = move |id: String| {
        selected.update(|s| {
            if let Some(pos) = s.iter().position(|x| *x == id) {
                s.remove(pos);
            } else {
                s.push(id);
            }
        });
    };
    let bulk_open = RwSignal::new(false);
    let bulk_date = RwSignal::new(String::new());
    let bulk_account = RwSignal::new(String::new());
    let bulk_notes = RwSignal::new(String::new());
    let bulk_error = RwSignal::new(None::<String>);

    let bulk_action = Action::new_local(move |_: &()| {
        let notes_val = bulk_notes.get_untracked();
        let payload = PlannedEntryBulkPayPayload {
            entry_ids: selected.get_untracked(),
            paid_at: date_to_rfc3339(&bulk_date.get_untracked()),
            account_id: bulk_account.get_untracked(),
            project_id: None,
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        async move { api::post_json("/api/admin/planned-entries/bulk-pay", &payload).await }
    });

    Effect::new(move |_| {
        if let Some(r) = bulk_action.value().get() {
            match r {
                Ok(()) => {
                    bulk_open.set(false);
                    selected.set(Vec::new());
                    reload();
                }
                Err(ApiError::Forbidden) => bulk_error.set(Some("No tienes permiso".into())),
                Err(_) => bulk_error.set(Some("No se pudo registrar el pago".into())),
            }
        }
    });

    let bulk_pending = bulk_action.pending();
    let bulk_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if bulk_date.get().trim().is_empty() || bulk_account.get().is_empty() {
            bulk_error.set(Some("Completa fecha y cuenta".into()));
            return;
        }
        bulk_error.set(None);
        bulk_action.dispatch(());
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Entradas planificadas"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-4xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar entrada"
                                        } else {
                                            "Nueva entrada"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-3">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Nombre"
                                        </label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Flujo"
                                        </label>
                                        <Select value=flow>
                                            <option value="income">"Ingreso"</option>
                                            <option value="expense">"Egreso"</option>
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Estado"
                                        </label>
                                        <Select value=status>
                                            {STATUSES
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
                                            "Cuenta esperada"
                                        </label>
                                        <Select value=account>
                                            <option value="">"— Selecciona —"</option>
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
                                            "Monto estimado"
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
                                            "Vencimiento"
                                        </label>
                                        <Input
                                            value=due
                                            on_input=Callback::new(move |v| due.set(v))
                                            r#type="date"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Contacto (opcional)"
                                        </label>
                                        <Select value=contact>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                contacts
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
                                            "Proyecto (opcional)"
                                        </label>
                                        <Select value=project>
                                            <option value="">"— Ninguno —"</option>
                                            {move || {
                                                projects
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
                                    <div class="flex items-end gap-2">
                                        <Button disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear entrada"
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

            // Pay form, shown when paying a specific entry.
            {move || {
                paying
                    .get()
                    .map(|_| {
                        view! {
                            <Card class="max-w-2xl border-emerald-300">
                                <CardHeader>
                                    <CardTitle>"Registrar pago"</CardTitle>
                                </CardHeader>
                                <CardContent>
                                    <form on:submit=pay_submit class="grid gap-3 sm:grid-cols-2">
                                        <div class="space-y-1">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Fecha de pago"
                                            </label>
                                            <Input
                                                value=pay_date
                                                on_input=Callback::new(move |v| pay_date.set(v))
                                                r#type="date"
                                                required=true
                                            />
                                        </div>
                                        <div class="space-y-1">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Monto real pagado"
                                            </label>
                                            <Input
                                                value=pay_amount
                                                on_input=Callback::new(move |v| pay_amount.set(v))
                                                inputmode="decimal"
                                                placeholder="0.00"
                                            />
                                        </div>
                                        <div class="space-y-1">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Cuenta"
                                            </label>
                                            <Select value=pay_account>
                                                <option value="">"— Selecciona —"</option>
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
                                                "Notas"
                                            </label>
                                            <Input
                                                value=pay_notes
                                                on_input=Callback::new(move |v| pay_notes.set(v))
                                            />
                                        </div>
                                        <div class="flex items-end gap-2 sm:col-span-2">
                                            <Button disabled=pay_pending>
                                                {move || {
                                                    if pay_pending.get() { "Pagando…" } else { "Confirmar pago" }
                                                }}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                on:click=move |_| cancel_pay()
                                            >
                                                "Cancelar"
                                            </Button>
                                            {move || {
                                                pay_error
                                                    .get()
                                                    .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                                            }}
                                        </div>
                                    </form>
                                </CardContent>
                            </Card>
                        }
                    })
            }}

            // Bulk-pay bar: appears when rows are selected.
            {move || {
                let n = selected.get().len();
                (is_admin && n > 0 && !bulk_open.get())
                    .then(|| {
                        view! {
                            <div class="flex items-center gap-3 rounded-md bg-slate-100 px-4 py-2 text-sm">
                                <span>{format!("{n} seleccionadas")}</span>
                                <Button
                                    on:click=move |_| {
                                        bulk_date.set(String::new());
                                        bulk_account.set(String::new());
                                        bulk_notes.set(String::new());
                                        bulk_error.set(None);
                                        bulk_open.set(true);
                                    }
                                >
                                    "Pagar seleccionadas"
                                </Button>
                            </div>
                        }
                    })
            }}

            // Bulk-pay form.
            {move || {
                bulk_open
                    .get()
                    .then(|| {
                        view! {
                            <Card class="max-w-2xl border-emerald-300">
                                <CardHeader>
                                    <CardTitle>
                                        {format!("Pagar {} entradas", selected.get().len())}
                                    </CardTitle>
                                </CardHeader>
                                <CardContent>
                                    <form on:submit=bulk_submit class="grid gap-3 sm:grid-cols-2">
                                        <div class="space-y-1">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Fecha de pago"
                                            </label>
                                            <Input
                                                value=bulk_date
                                                on_input=Callback::new(move |v| bulk_date.set(v))
                                                r#type="date"
                                                required=true
                                            />
                                        </div>
                                        <div class="space-y-1">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Cuenta"
                                            </label>
                                            <Select value=bulk_account>
                                                <option value="">"— Selecciona —"</option>
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
                                        <div class="space-y-1 sm:col-span-2">
                                            <label class="block text-sm font-medium text-slate-700">
                                                "Notas"
                                            </label>
                                            <Input
                                                value=bulk_notes
                                                on_input=Callback::new(move |v| bulk_notes.set(v))
                                            />
                                        </div>
                                        <div class="flex items-end gap-2 sm:col-span-2">
                                            <Button disabled=bulk_pending>
                                                {move || {
                                                    if bulk_pending.get() { "Pagando…" } else { "Confirmar pago" }
                                                }}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                on:click=move |_| bulk_open.set(false)
                                            >
                                                "Cancelar"
                                            </Button>
                                            {move || {
                                                bulk_error
                                                    .get()
                                                    .map(|m| view! { <p class="text-sm text-red-600">{m}</p> })
                                            }}
                                        </div>
                                    </form>
                                </CardContent>
                            </Card>
                        }
                    })
            }}

            {move || match items.get() {
                None => view! { <p class="text-slate-500">"Cargando…"</p> }.into_any(),
                Some(Err(_)) => {
                    view! { <p class="text-red-600">"No se pudieron cargar las entradas."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin entradas todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2"></th>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Flujo"</th>
                                        <th class="px-4 py-2 font-medium">"Monto"</th>
                                        <th class="px-4 py-2 font-medium">"Vence"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|e| {
                                            let id = e.id.clone();
                                            let sid = e.id.clone();
                                            let status = if e.status_label.is_empty() {
                                                e.status.clone()
                                            } else {
                                                e.status_label.clone()
                                            };
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">
                                                        {move || {
                                                            if is_admin {
                                                                let sid = sid.clone();
                                                                let cid = sid.clone();
                                                                view! {
                                                                    <input
                                                                        type="checkbox"
                                                                        class="h-4 w-4 rounded border-slate-300"
                                                                        prop:checked=move || selected.get().contains(&cid)
                                                                        on:change=move |_| toggle_sel(sid.clone())
                                                                    />
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                ().into_any()
                                                            }
                                                        }}
                                                    </td>
                                                    <td class="px-4 py-2">{e.name}</td>
                                                    <td class="px-4 py-2">
                                                        {flow_label(&e.flow_type).to_string()}
                                                    </td>
                                                    <td class="px-4 py-2">{money(e.amount_estimated)}</td>
                                                    <td class="px-4 py-2 text-slate-500">{e.due_date}</td>
                                                    <td class="px-4 py-2">{status}</td>
                                                    <td class="px-4 py-2 text-right">
                                                        {move || {
                                                            if is_admin {
                                                                let eid = id.clone();
                                                                let did = id.clone();
                                                                let pid = id.clone();
                                                                view! {
                                                                    <Button
                                                                        variant=ButtonVariant::Ghost
                                                                        class="text-emerald-700 hover:bg-emerald-50"
                                                                        on:click=move |_| begin_pay(pid.clone())
                                                                    >
                                                                        "Pagar"
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
