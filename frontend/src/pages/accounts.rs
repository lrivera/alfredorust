use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, Account, AccountPayload, ApiError, Me};
use crate::components::{
    Button, ButtonVariant, Card, CardContent, CardHeader, CardTitle, Checkbox, Input, Select,
};

/// Account types and their Spanish labels. Values match the backend
/// (`bank|cash|credit_card|investment|other`).
const ACCOUNT_TYPES: &[(&str, &str)] = &[
    ("bank", "Banco"),
    ("cash", "Efectivo"),
    ("credit_card", "Tarjeta de crédito"),
    ("investment", "Inversión"),
    ("other", "Otro"),
];

pub(crate) fn account_type_label(value: &str) -> &str {
    ACCOUNT_TYPES
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, l)| *l)
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn label_maps_known_types() {
        assert_eq!(account_type_label("bank"), "Banco");
        assert_eq!(account_type_label("credit_card"), "Tarjeta de crédito");
    }

    #[wasm_bindgen_test]
    fn label_falls_back_to_raw_value() {
        assert_eq!(account_type_label("unknown_type"), "unknown_type");
    }
}

#[component]
pub fn AccountsPage() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    // Accounts are admin-only server-side; gate the UI to match (UX only).
    let is_admin = me.role == "admin";

    let items = RwSignal::new(None::<Result<Vec<Account>, ApiError>>);
    let reload = move || {
        items.set(None);
        spawn_local(async move {
            items.set(Some(api::list_accounts().await));
        });
    };
    reload();

    // --- create/edit form -------------------------------------------------
    // `editing` holds the id being edited (None = create). `is_active`/`notes`
    // are preserved hidden so editing never wipes them.
    let editing = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let acc_type = RwSignal::new("bank".to_string());
    let currency = RwSignal::new("MXN".to_string());
    let is_active = RwSignal::new(true);
    let notes = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);

    let reset_form = move || {
        editing.set(None);
        name.set(String::new());
        acc_type.set("bank".to_string());
        currency.set("MXN".to_string());
        is_active.set(true);
        notes.set(String::new());
        form_error.set(None);
    };

    let save = Action::new_local(move |_: &()| {
        let notes_val = notes.get_untracked();
        let payload = AccountPayload {
            name: name.get_untracked().trim().to_string(),
            account_type: acc_type.get_untracked(),
            currency: Some(currency.get_untracked()),
            is_active: is_active.get_untracked(),
            notes: (!notes_val.trim().is_empty()).then_some(notes_val),
        };
        let editing = editing.get_untracked();
        async move {
            match editing {
                Some(id) => api::update_account(&id, &payload).await,
                None => api::create_account(&payload).await,
            }
        }
    });

    Effect::new(move |_| {
        if let Some(result) = save.value().get() {
            match result {
                Ok(()) => {
                    reset_form();
                    reload();
                }
                Err(e) => form_error.set(Some(api::humanize(&e, "No se pudo guardar la cuenta"))),
            }
        }
    });

    let pending = save.pending();
    let submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        if name.get().trim().is_empty() {
            form_error.set(Some("El nombre es obligatorio".into()));
            return;
        }
        form_error.set(None);
        save.dispatch(());
    };

    let begin_edit = move |id: String| {
        spawn_local(async move {
            if let Ok(d) =
                api::get_json::<api::AccountDetail>(&format!("/api/admin/accounts/{id}")).await
            {
                name.set(d.name);
                acc_type.set(d.account_type);
                currency.set(d.currency);
                is_active.set(d.is_active);
                notes.set(d.notes.unwrap_or_default());
                editing.set(Some(id));
                form_error.set(None);
            }
        });
    };

    let delete_one = move |id: String| {
        spawn_local(async move {
            if api::delete_account(&id).await.is_ok() {
                reload();
            }
        });
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-xl font-semibold">"Cuentas"</h1>

            {move || {
                if is_admin {
                    view! {
                        <Card class="max-w-2xl">
                            <CardHeader>
                                <CardTitle>
                                    {move || {
                                        if editing.get().is_some() {
                                            "Editar cuenta"
                                        } else {
                                            "Nueva cuenta"
                                        }
                                    }}
                                </CardTitle>
                            </CardHeader>
                            <CardContent>
                                <form on:submit=submit class="grid gap-3 sm:grid-cols-2">
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Nombre"
                                        </label>
                                        <Input
                                            value=name
                                            on_input=Callback::new(move |v| name.set(v))
                                            placeholder="Cuenta principal"
                                            required=true
                                        />
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Tipo"
                                        </label>
                                        <Select value=acc_type>
                                            {ACCOUNT_TYPES
                                                .iter()
                                                .map(|(value, label)| {
                                                    view! { <option value=*value>{*label}</option> }
                                                })
                                                .collect::<Vec<_>>()}
                                        </Select>
                                    </div>
                                    <div class="space-y-1">
                                        <label class="block text-sm font-medium text-slate-700">
                                            "Moneda"
                                        </label>
                                        <Input
                                            value=currency
                                            on_input=Callback::new(move |v| currency.set(v))
                                            placeholder="MXN"
                                        />
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
                                    <div class="sm:col-span-2">
                                        <Checkbox checked=is_active label="Activa" />
                                    </div>
                                    <div class="flex items-end gap-2">
                                        <Button r#type="submit" disabled=pending>
                                            {move || {
                                                if pending.get() {
                                                    "Guardando…"
                                                } else if editing.get().is_some() {
                                                    "Guardar cambios"
                                                } else {
                                                    "Crear cuenta"
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
                                            .map(|msg| {
                                                view! {
                                                    <p class="text-sm text-red-600 sm:col-span-2">{msg}</p>
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
                    view! { <p class="text-red-600">"No se pudieron cargar las cuentas."</p> }
                        .into_any()
                }
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="text-slate-500">"Sin cuentas todavía."</p> }.into_any()
                }
                Some(Ok(list)) => {
                    view! {
                        <div class="overflow-hidden rounded-xl border border-slate-200 bg-white">
                            <table class="w-full text-left text-sm">
                                <thead class="bg-slate-50 text-slate-600">
                                    <tr>
                                        <th class="px-4 py-2 font-medium">"Nombre"</th>
                                        <th class="px-4 py-2 font-medium">"Tipo"</th>
                                        <th class="px-4 py-2 font-medium">"Moneda"</th>
                                        <th class="px-4 py-2 font-medium">"Estado"</th>
                                        <th class="px-4 py-2"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list
                                        .into_iter()
                                        .map(|acc| {
                                            let id = acc.id.clone();
                                            view! {
                                                <tr class="border-t border-slate-100">
                                                    <td class="px-4 py-2">{acc.name}</td>
                                                    <td class="px-4 py-2">
                                                        {account_type_label(&acc.account_type).to_string()}
                                                    </td>
                                                    <td class="px-4 py-2">{acc.currency}</td>
                                                    <td class="px-4 py-2">
                                                        {if acc.is_active { "Activa" } else { "Inactiva" }}
                                                    </td>
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
