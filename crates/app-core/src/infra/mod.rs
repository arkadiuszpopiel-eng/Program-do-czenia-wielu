//! Kleje kompozycji: HTTP, pośrednicy modułów w rejestrze (osadzacz i późne wiązanie — `app-modules`).

pub mod http;
pub mod proxy;

pub use app_modules::{catalog, probe};
