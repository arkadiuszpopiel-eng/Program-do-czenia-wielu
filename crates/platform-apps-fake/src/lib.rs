//! Atrapa portów aplikacji F6 (docs/modules/platform-apps/SPEC.md, sekcja „Fake”): Office
//! z dokumentem w pamięci, przeglądarka z wirtualną siecią, rejestr z deny-listą sekretów, system
//! (procesy z drzewem, usługi, zdarzenia, zmienne) i kwarantanna pobrań w pamięci —
//! deterministycznie, bez COM, bez sieci i bez rejestru systemu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod browser;
mod doc;
mod downloads;
mod office;
mod registry;
mod sys;

pub use browser::{FakeBrowser, FakeNode, FakePage};
pub use doc::{Block, Cell, FakeDocument, Sheet};
pub use downloads::{FakeDownloads, StoredFile};
pub use office::FakeOffice;
pub use registry::FakeRegistry;
pub use sys::{FakeSys, SysCall, fake_process};
