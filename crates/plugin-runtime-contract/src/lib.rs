//! Kontrakt modułu `plugin-runtime` — wtyczki Wasm (docs/modules/plugin-runtime/SPEC.md,
//! ADR 0012, PLAN §3.2, §8.7, §12.1 pierścień R2, THREAT_MODEL S09, ACCEPTANCE F8-05).
//!
//! Wtyczka = komponent Wasm świata WIT [`WIT`] (`alfa:plugin/plugin`) + [`PluginManifest`]:
//! id, wersja semver, autor, opis, SHA-256 modułu, **zadeklarowane zdolności** ⊆
//! [`ALLOWED_FAMILIES`] (nigdy Jądro/`system.admin`, `secrets.read`, `shell.exec`,
//! `gui.control`), limity piaskownicy ([`PluginLimits`]: pamięć, paliwo, czas, I/O, operacje
//! hosta) i narzędzia z JSON Schema, które trafiają do rejestru narzędzi agentek
//! (`tools-common`, prefiks [`TOOL_PREFIX`], grupy ról `plugin`/`plugin.<id>`).
//!
//! Cykl życia ([`PluginLibrary`], wspólny dla `-impl` i `-fake`): propozycja → zatwierdzenie
//! właściciela **w oknie** z hashem przejrzanej wersji ([`review_hash`], obejmuje hash
//! modułu) → instalacja; aktualizacja = wyższa wersja i ponowne zatwierdzenie; wyłączenie;
//! włączenie = ponowne zatwierdzenie; usunięcie. Ulepszacz proponuje zmianę R2
//! ([`r2_change`]) — klucz `plugins.<id>.version` z hashem jako wartością.
//!
//! Jedyny dostęp wtyczki do świata: operacje hosta ([`HostOp`]) — zdolność musi mieścić się
//! w manifeście, a token wydaje Broker dla agentki wywołującej (wtyczka ≤ rola). Wynik
//! wtyczki jest treścią niezaufaną ([`UNTRUSTED_SOURCE`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod error;
mod host;
mod improver;
mod invoke;
mod library;
mod manifest;
mod model;
pub mod samples;
mod store;
mod validate;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventKind;
use semver::Version;
use tools_common_contract::{Tool, ToolManifest};

pub use error::{ExecError, LoadError, MAX_ERROR_CHARS, PluginError, sanitize};
pub use host::{
    HOST_OPS, HostCall, HostError, HostOp, MAX_LOG_CHARS, MemHost, PluginHost, declared,
};
pub use improver::{KEY_PREFIX, KEY_SUFFIX, R2Change, improver_key, parse_r2, r2_change};
pub use invoke::{
    InvocationStats, MAX_LOGS, MAX_TEXT_CHARS, check_input, ok_outcome, parse_output,
};
pub use library::{PluginLibrary, record_event};
pub use manifest::{
    ALLOWED_FAMILIES, EXPORT_INVOKE, FORBIDDEN_FAMILIES, HOST_CALL, HOST_INTERFACE, MAX_WASM_BYTES,
    PluginId, PluginLimits, PluginManifest, PluginToolDecl, RUN_CAPABILITY, TOOL_GROUP,
    TOOL_PREFIX, UNTRUSTED_SOURCE, WIT, WIT_PACKAGE, ceilings,
};
pub use model::{ApprovalOrigin, PluginApproval, PluginRecord, PluginSource, PluginState};
pub use store::{MemPluginStore, PluginStore, wasm_key};
pub use validate::{
    COMPONENT_HEADER, MAX_CAPABILITIES, MAX_SCHEMA_BYTES, MAX_TOOLS, canonical_json,
    check_approved, check_capabilities, check_wasm, fold, is_component, is_sha256_hex, review_hash,
    sha256_hex, suspicious, validate_manifest,
};

/// Nazwy zdarzeń (ładunki bez treści wejścia/wyjścia: id, wersja, hashe, stan, liczniki).
pub mod events {
    /// Nowa wersja czeka na zatwierdzenie.
    pub const PROPOSED: &str = "plugin.proposed";
    /// Zainstalowano wersję.
    pub const INSTALLED: &str = "plugin.installed";
    /// Starsza wersja zastąpiona.
    pub const SUPERSEDED: &str = "plugin.superseded";
    /// Odrzucono propozycję.
    pub const REJECTED: &str = "plugin.rejected";
    /// Wyłączono.
    pub const DISABLED: &str = "plugin.disabled";
    /// Włączono ponownie (po ponownym zatwierdzeniu).
    pub const ENABLED: &str = "plugin.enabled";
    /// Usunięto (wszystkie wersje).
    pub const REMOVED: &str = "plugin.removed";
    /// Moduł nie przeszedł ładowania (hash, importy, eksporty) — poziom `Warn`.
    pub const LOAD_FAILED: &str = "plugin.load_failed";
    /// Wywołanie zakończone (statystyki) — poziom `Debug`.
    pub const INVOKED: &str = "plugin.invoked";
    /// Wtyczka przerwana przez piaskownicę (paliwo, czas, pamięć, pułapka) — dla Diagnosty.
    pub const TRAPPED: &str = "plugin.trapped";
    /// Operacja hosta odrzucona (manifest albo Broker) — poziom `Warn`.
    pub const HOST_DENIED: &str = "plugin.host_denied";
}

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Biblioteka i środowisko uruchomieniowe wtyczek. Metody zmieniające stan wołają wyłącznie
/// komendy UI właściciela (strona „Wtyczki”, „Zdrowie systemu”) — nie są narzędziami agentek.
#[async_trait]
pub trait Plugins: Send + Sync {
    /// Wszystkie wersje.
    fn list(&self) -> Vec<PluginRecord>;

    /// Zainstalowana (aktywna) wersja.
    fn installed(&self, id: &PluginId) -> Option<PluginRecord>;

    /// Manifesty narzędzi aktywnych wtyczek (rejestr narzędzi agentek, walidacja umiejętności).
    fn tool_catalog(&self) -> Vec<ToolManifest>;

    /// Narzędzia aktywnych wtyczek do rejestru `agent-runtime`.
    fn tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Propozycja: walidacja manifestu, hash i nagłówek modułu, kontrola importów/eksportów.
    async fn propose(
        &self,
        manifest: PluginManifest,
        wasm: Vec<u8>,
        source: PluginSource,
    ) -> Result<PluginRecord, PluginError>;

    /// Zatwierdzenie (kanał UI, hash przejrzanej wersji) → instalacja.
    async fn approve(
        &self,
        id: &PluginId,
        version: &Version,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError>;

    /// Odrzucenie propozycji.
    async fn reject(&self, id: &PluginId, version: &Version) -> Result<PluginRecord, PluginError>;

    /// Wyłączenie (narzędzia znikają z rejestru, moduł zwolniony).
    async fn disable(&self, id: &PluginId) -> Result<PluginRecord, PluginError>;

    /// Ponowne włączenie po ponownym zatwierdzeniu.
    async fn enable(
        &self,
        id: &PluginId,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError>;

    /// Usunięcie wszystkich wersji i modułów.
    async fn remove(&self, id: &PluginId) -> Result<Vec<PluginRecord>, PluginError>;

    /// Zmiana R2 dla propozycji (karta Ulepszacza).
    fn r2_proposal(&self, id: &PluginId, version: &Version) -> Result<R2Change, PluginError> {
        let record = self
            .list()
            .into_iter()
            .find(|r| &r.manifest.id == id && &r.manifest.version == version)
            .ok_or_else(|| PluginError::NotFound(format!("{id}@{version}")))?;
        Ok(r2_change(&record, self.installed(id).as_ref()))
    }

    /// Wdrożenie zmiany R2 zatwierdzonej w Ulepszaczu: wartość klucza = hash przejrzanej
    /// wersji, zatwierdzenie właściciela musi dotyczyć dokładnie tego hasha.
    async fn deploy_r2(
        &self,
        key: &str,
        value: &serde_json::Value,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError> {
        let (id, hash) = parse_r2(key, value)?;
        if approval.reviewed_hash != hash {
            return Err(PluginError::HashMismatch);
        }
        let record = self
            .list()
            .into_iter()
            .find(|r| r.manifest.id == id && r.review_hash == hash)
            .ok_or_else(|| PluginError::NotFound(format!("{id}#{hash}")))?;
        self.approve(&id, &record.manifest.version, approval).await
    }
}
