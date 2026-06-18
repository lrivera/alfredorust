use leptos::prelude::*;

use super::merge_classes;

const CARD_BASE: &str = "rounded-xl border border-slate-200 bg-white shadow-sm";

#[component]
pub fn Card(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! { <div class=merge_classes(CARD_BASE, &class)>{children()}</div> }
}

#[component]
pub fn CardHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! { <div class=merge_classes("flex flex-col space-y-1.5 p-6", &class)>{children()}</div> }
}

#[component]
pub fn CardTitle(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <h3 class=merge_classes("text-lg font-semibold leading-none tracking-tight", &class)>
            {children()}
        </h3>
    }
}

#[component]
pub fn CardContent(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! { <div class=merge_classes("p-6 pt-0", &class)>{children()}</div> }
}
