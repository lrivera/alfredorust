use leptos::prelude::*;

use super::merge_classes;

/// Color intent for a [`Badge`]. Tones are token-based so they read on both
/// themes; the colored tones use Tailwind palettes that work light and dark.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

impl BadgeTone {
    fn classes(self) -> &'static str {
        match self {
            BadgeTone::Neutral => "bg-muted text-muted-foreground ring-border",
            BadgeTone::Success => "bg-emerald-500/15 text-emerald-600 ring-emerald-500/30",
            BadgeTone::Warning => "bg-amber-500/15 text-amber-600 ring-amber-500/30",
            BadgeTone::Danger => "bg-rose-500/15 text-rose-600 ring-rose-500/30",
            BadgeTone::Info => "bg-sky-500/15 text-sky-600 ring-sky-500/30",
        }
    }
}

const BADGE_BASE: &str = "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs \
     font-semibold ring-1 ring-inset whitespace-nowrap";

/// A small pill for statuses, types, roles. Defaults to a neutral token tone.
#[component]
pub fn Badge(
    #[prop(optional)] tone: BadgeTone,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let classes = merge_classes(&format!("{BADGE_BASE} {}", tone.classes()), &class);
    view! { <span class=classes>{children()}</span> }
}
