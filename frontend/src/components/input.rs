use leptos::prelude::*;

use super::merge_classes;

const INPUT_BASE: &str = "w-full rounded-md border border-slate-300 px-3 py-2 text-sm \
     outline-none transition-colors focus:border-blue-500 focus:ring-2 focus:ring-blue-200 \
     disabled:cursor-not-allowed disabled:opacity-50";

/// Controlled text input. `value`/`on_input` make it controlled; the remaining
/// optional props map straight to the matching HTML attributes.
#[component]
pub fn Input(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] r#type: String,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional, into)] autocomplete: String,
    #[prop(optional, into)] inputmode: String,
    #[prop(optional, into)] maxlength: String,
    #[prop(optional, into)] required: bool,
) -> impl IntoView {
    let classes = merge_classes(INPUT_BASE, &class);
    let ty = if r#type.is_empty() {
        "text".to_string()
    } else {
        r#type
    };
    view! {
        <input
            class=classes
            type=ty
            placeholder=placeholder
            autocomplete=autocomplete
            inputmode=inputmode
            maxlength=maxlength
            required=required
            prop:value=move || value.get()
            on:input=move |ev| on_input.run(event_target_value(&ev))
        />
    }
}
