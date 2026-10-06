//! Ochrona ruchu sieciowego agentek i wtyczek (biblioteka `lib-*` bez logiki modułu; PLAN §8.1
//! egress-allowlista, THREAT_MODEL: SSRF, DNS rebinding).
//!
//! - [`is_public_ip`] — adres jest publiczny (nie: pętla, prywatne, link-local, CGNAT, multicast,
//!   dokumentacja, zarezerwowane, IPv4 mapowane/zgodne/NAT64/6to4 z takim adresem, ULA, Teredo);
//!   IPv6 publiczne tylko z `2000::/3`;
//! - [`check_url`] / [`redirect_target`] — adres od modelu albo z nagłówka `Location` parsowany tym
//!   samym parserem WHATWG co klient: tylko `https://`, bez danych logowania, bez hostów lokalnych
//!   (`localhost`, `*.local`, `*.internal`, jednoczłonowe — LLMNR/NetBIOS), adres IP tylko publiczny
//!   (także w postaciach `0x7f.1`, `2130706433`);
//! - [`check_resolved`] — wszystkie adresy z DNS muszą być publiczne (jeden niepubliczny = odmowa);
//! - `client` (cecha `client`): resolver dla `reqwest` z tym sprawdzeniem — połączenie idzie
//!   wyłącznie na adresy z tego samego rozwiązania (bez drugiego zapytania DNS) — oraz klient bez
//!   proxy, bez przekierowań i tylko HTTPS.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod addr;
#[cfg(feature = "client")]
pub mod client;
mod target;

pub use addr::{GuardError, check_resolved, is_public_ip};
pub use target::{MAX_URL_LEN, Target, UrlError, check_url, redirect_target};
pub use url::Url;
