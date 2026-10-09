//! Drobne funkcje pomocnicze Routera: które błędy obciążają obwód, kody odrzuceń i ostrzeżeń
//! dla interfejsu i testów, składanie Routera z listy dostawców.

use std::sync::Arc;

use providers_contract::{ModelProvider, ProviderErrorKind};
use router_contract::{RejectReason, RouteKind, RouteWarning};

use crate::routing::RouterCore;

/// Czy błąd obciąża dostawcę (obwód): nie — odrzucenia żądania, prywatność, brak funkcji,
/// limit 429 (osobne okno).
pub fn counts_as_failure(kind: &ProviderErrorKind) -> bool {
    !matches!(
        kind,
        ProviderErrorKind::InvalidRequest
            | ProviderErrorKind::PrivacyBlocked
            | ProviderErrorKind::Unsupported
            | ProviderErrorKind::RateLimited { .. }
    )
}

/// Ułatwienie dla kompozycji: `Arc<RouterCore>` z dostawcami.
pub fn router_with(providers: Vec<(Arc<dyn ModelProvider>, RouteKind)>) -> Arc<RouterCore> {
    let core = RouterCore::default();
    for (p, kind) in providers {
        core.register(p, kind);
    }
    Arc::new(core)
}

/// Pojedyncze odrzucenie — dla testów i UI.
pub fn reason_code(reason: &RejectReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Ostrzeżenie — kod.
pub fn warning_code(warning: &RouteWarning) -> String {
    serde_json::to_value(warning)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}
