//! „O programie”: wersja, kanał, data kompilacji i commit (zmienne `ALFA_BUILD_DATE`,
//! `ALFA_BUILD_COMMIT` ustawiane przez `.github/workflows/release.yml`; w buildzie deweloperskim
//! brak), licencje zależności z `data/licenses.json` (generuje
//! `apps/desktop/scripts/gen-licenses.mjs` z `cargo metadata` powłoki i `pnpm licenses`).

use app_api::dto::{AboutInfo, LicenseEntry};
use serde::Deserialize;
use updater_impl::UpdateService;

use crate::view::channel_dto;

/// Wygenerowana lista licencji (wbudowana w binarium).
pub const LICENSES_JSON: &str = include_str!("../data/licenses.json");

#[derive(Deserialize)]
struct LicensesFile {
    generated_at: Option<String>,
    entries: Vec<LicenseEntry>,
}

/// Lista licencji i data wygenerowania; uszkodzony plik = pusta lista.
pub fn licenses() -> (Option<String>, Vec<LicenseEntry>) {
    match serde_json::from_str::<LicensesFile>(LICENSES_JSON) {
        Ok(f) => (f.generated_at, f.entries),
        Err(e) => {
            tracing::warn!(error = %e, "data/licenses.json nieczytelny");
            (None, Vec::new())
        }
    }
}

pub(crate) fn about(service: &UpdateService) -> AboutInfo {
    let status = service.status();
    let (generated_at, entries) = licenses();
    AboutInfo {
        version: status.current.to_string(),
        channel: channel_dto(status.channel),
        build_date: option_env!("ALFA_BUILD_DATE").map(str::to_owned),
        commit: option_env!("ALFA_BUILD_COMMIT").map(str::to_owned),
        target: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        updates_configured: service.updater().config().disabled_reason().is_none(),
        licenses_generated_at: generated_at,
        licenses: entries,
    }
}
