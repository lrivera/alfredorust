use leptos::prelude::*;

use super::switch_company_href;
use crate::api::Me;

#[component]
pub fn Dashboard() -> impl IntoView {
    let me = use_context::<Me>().expect("Me context");
    let companies = me.companies.clone();

    view! {
        <h1 class="mb-4 text-xl font-semibold">"Inicio"</h1>
        <h2 class="mb-2 text-lg font-semibold">"Compañías"</h2>
        <ul class="space-y-1">
            {companies
                .into_iter()
                .map(|c| {
                    let href = switch_company_href(&c.slug);
                    let cls = if c.active {
                        "font-semibold text-foreground"
                    } else {
                        "text-muted-foreground hover:underline"
                    };
                    let suffix = if c.active { " (activa)" } else { "" };
                    view! {
                        <li>
                            <a href=href class=cls>
                                {c.name}
                                {suffix}
                            </a>
                        </li>
                    }
                })
                .collect::<Vec<_>>()}
        </ul>
    }
}
