// routes/mod.rs
// Public re-exports of all route handlers.

pub mod setup;
pub mod qrcode;
pub mod login;
pub mod secret;
pub mod home;
pub mod logout;

pub use setup::setup;
pub use qrcode::qrcode;
pub use login::login;
pub use secret::secret_generate;
pub use home::home;
pub use logout::logout;
