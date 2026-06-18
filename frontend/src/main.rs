mod api;
mod app;
mod components;
mod pages;
mod theme;

use app::App;

fn main() {
    console_error_panic_hook::set_once();
    // Apply the persisted theme before mounting so there's no flash of the
    // default palette.
    theme::apply_theme(theme::stored_theme());
    leptos::mount::mount_to_body(App);
}
