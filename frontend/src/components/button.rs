use leptos::prelude::*;

use super::merge_classes;

/// Visual variants, mirroring the shadcn/Rust-UI button styles.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            ButtonVariant::Primary => "bg-primary text-primary-foreground hover:bg-primary/90",
            ButtonVariant::Secondary => "bg-muted text-foreground hover:bg-muted/70",
            ButtonVariant::Outline => {
                "border border-border bg-transparent text-foreground hover:bg-muted"
            }
            ButtonVariant::Ghost => "bg-transparent text-foreground hover:bg-muted",
        }
    }
}

const BUTTON_BASE: &str = "inline-flex items-center justify-center rounded-md px-3 py-2 \
     text-sm font-medium transition-colors focus:outline-none focus:ring-2 \
     focus:ring-ring disabled:pointer-events-none disabled:opacity-50";

/// Button. The native `<button>` defaults to `submit` inside a form, so the
/// login submit button works without an explicit type; standalone buttons can
/// pass `on:click`. Reactive `disabled` is supported via `MaybeProp`.
#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] disabled: MaybeProp<bool>,
    /// Defaults to "button" so action buttons inside a `<form>` never submit it
    /// by accident. Set `r#type="submit"` on the form's submit button.
    #[prop(optional, into)] r#type: String,
    children: Children,
) -> impl IntoView {
    let classes = merge_classes(
        &format!("{BUTTON_BASE} {}", variant.classes()),
        &class,
    );
    let ty = if r#type.is_empty() { "button".to_string() } else { r#type };
    view! {
        <button type=ty class=classes disabled=move || disabled.get().unwrap_or(false)>
            {children()}
        </button>
    }
}
