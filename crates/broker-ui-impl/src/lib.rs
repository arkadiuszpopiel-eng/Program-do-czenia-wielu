//! Broker-UI (docs/modules/broker-ui/SPEC.md, PLAN §8.2, ADR 3): okno zatwierdzeń jako osobny
//! natywny proces w sesji użytkownika, uruchamiany przez usługę Brokera z wyższym poziomem
//! integralności (UIPI blokuje SendInput z procesów agentek). Bez WebView i bez HTML/markdown.
//!
//! Cykl: prośba z Brokera ([`BrokerLink`]: IPC przez named pipe z ACL albo kanał w procesie) →
//! karta w oknie ([`NativeBrokerUi`] na porcie `ApprovalSurfacePort` z `platform-windows-impl`) →
//! decyzja z `PhysicalInputProof` budowanym wyłącznie w [`session`] po regułach z
//! `broker-ui-contract` (wejście niewstrzyknięte, okno ≥ 500 ms na pierwszym planie, bez nakładki,
//! jednorazowy nonce) → odpowiedź do Brokera ([`driver::cycle`]).
//!
//! Klawiatura: `Tab` + `Spacja`; `Enter` niczego nie zatwierdza; `Esc` = odmowa; fokus startowy
//! na „Odmów”. Okno nie kradnie fokusu (miga), chyba że ryzyko jest wysokie (alertdialog).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod driver;
mod link;
mod native;
pub mod session;
mod view;

pub use broker_ui_contract::{BrokerLink, BrokerUi};
pub use link::{ChannelLink, PipeLink};
pub use native::NativeBrokerUi;
pub use view::{BTN_DENY, BTN_ONCE, BTN_SCOPED, decision_for, view_of};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
