use leptos::prelude::*;

/// Controlled checkbox bound to a `bool` signal, with an inline label.
#[component]
pub fn Checkbox(
    checked: RwSignal<bool>,
    #[prop(into)] label: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    view! {
        <label class=format!("inline-flex items-center gap-2 text-sm text-foreground {class}")>
            <input
                type="checkbox"
                class="h-4 w-4 rounded border-input"
                prop:checked=move || checked.get()
                on:change=move |ev| checked.set(event_target_checked(&ev))
            />
            {label}
        </label>
    }
}
