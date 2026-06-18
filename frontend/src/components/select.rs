use leptos::prelude::*;

use super::merge_classes;

const SELECT_BASE: &str = "w-full rounded-md border border-input bg-background text-foreground \
     px-3 py-2 text-sm outline-none transition-colors focus:border-ring focus:ring-2 \
     focus:ring-ring/30";

/// Controlled `<select>`; `value` is the bound signal, `<option>`s are children.
#[component]
pub fn Select(
    value: RwSignal<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    view! {
        <select
            class=merge_classes(SELECT_BASE, &class)
            prop:value=move || value.get()
            on:change=move |ev| value.set(event_target_value(&ev))
        >
            {children()}
        </select>
    }
}
