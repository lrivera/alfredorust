// routes/mod.rs
// Public re-exports of all route handlers.

pub mod admin;
pub mod home;
pub mod login;
pub mod logout;
pub mod pdf;
pub mod qrcode;
pub mod secret;
pub mod setup;
pub mod tiempo;

pub use admin::*;
pub use home::home;
pub use login::login;
pub use logout::logout;
pub use pdf::*;
pub use qrcode::qrcode;
pub use secret::secret_generate;
pub use setup::setup;
pub use tiempo::{tiempo_data, tiempo_page};
