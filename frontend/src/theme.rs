//! Light/dark theme handling. The dark "night" palette is the default (see
//! `style/input.css`); choosing light sets `data-theme="light"` on the root
//! `<html>` element. The choice is persisted in `localStorage` so it survives
//! reloads and navigations between tenants.

use leptos::prelude::*;

const STORAGE_KEY: &str = "alfredodev-theme";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    fn from_str(s: &str) -> Option<Theme> {
        match s {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    pub fn toggled(self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// The persisted choice, defaulting to dark when nothing is stored (the user
/// prefers dark by default).
pub fn stored_theme() -> Theme {
    local_storage()
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|v| Theme::from_str(&v))
        .unwrap_or(Theme::Dark)
}

/// Reflect the theme onto `<html data-theme>`: dark is the default palette so we
/// remove the attribute, light sets it explicitly.
pub fn apply_theme(theme: Theme) {
    let doc = document();
    if let Some(root) = doc.document_element() {
        match theme {
            Theme::Dark => {
                let _ = root.remove_attribute("data-theme");
            }
            Theme::Light => {
                let _ = root.set_attribute("data-theme", "light");
            }
        }
    }
}

/// Persist and apply a theme choice.
pub fn set_theme(theme: Theme) {
    if let Some(s) = local_storage() {
        let _ = s.set_item(STORAGE_KEY, theme.as_str());
    }
    apply_theme(theme);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn toggle_is_an_involution() {
        assert_eq!(Theme::Dark.toggled(), Theme::Light);
        assert_eq!(Theme::Light.toggled(), Theme::Dark);
        assert_eq!(Theme::Dark.toggled().toggled(), Theme::Dark);
    }

    #[wasm_bindgen_test]
    fn parse_roundtrips_and_rejects_unknown() {
        assert_eq!(Theme::from_str("dark"), Some(Theme::Dark));
        assert_eq!(Theme::from_str("light"), Some(Theme::Light));
        assert_eq!(Theme::from_str("solarized"), None);
        assert_eq!(Theme::from_str(Theme::Dark.as_str()), Some(Theme::Dark));
        assert_eq!(Theme::from_str(Theme::Light.as_str()), Some(Theme::Light));
    }
}
