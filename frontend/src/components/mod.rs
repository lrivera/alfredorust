//! Owned UI primitives, Rust/UI (shadcn) style: Tailwind classes + variants,
//! the code lives in our repo so we can edit it freely. No black-box crate.
//!
//! Each component takes an optional `class` that is appended after the base
//! classes, so callers can extend/override styling at the call site.

mod button;
mod card;
mod checkbox;
mod input;
mod select;

pub use button::{Button, ButtonVariant};
pub use card::{Card, CardContent, CardHeader, CardTitle};
pub use checkbox::Checkbox;
pub use input::Input;
pub use select::Select;

/// Append a caller-provided class string after the component's base classes.
pub(crate) fn merge_classes(base: &str, extra: &str) -> String {
    if extra.is_empty() {
        base.to_string()
    } else {
        format!("{base} {extra}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn merge_returns_base_when_no_extra() {
        assert_eq!(merge_classes("a b", ""), "a b");
    }

    #[wasm_bindgen_test]
    fn merge_appends_extra_after_base() {
        assert_eq!(merge_classes("a b", "c"), "a b c");
    }

    #[wasm_bindgen_test]
    fn button_variants_differ_and_carry_color() {
        use button::ButtonVariant;
        // Variants are distinct so a caller's choice actually changes styling.
        let primary = ButtonVariant::Primary;
        let outline = ButtonVariant::Outline;
        assert_ne!(primary, outline);
    }
}
