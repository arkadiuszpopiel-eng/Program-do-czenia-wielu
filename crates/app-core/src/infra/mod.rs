//! Kleje kompozycji: sekrety, katalog dostawców, sonda kont, osadzacz, pośrednicy.

pub mod embedder;
pub mod http;
pub mod late;
pub mod proxy;
pub mod secrets;

pub use app_modules::{catalog, probe};
