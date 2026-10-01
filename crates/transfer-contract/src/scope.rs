//! Zakres eksportu, tryby importu, rozstrzyganie kolizji i żądania operacji.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use accounts_hub_contract::SecretString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::SessionId;

use crate::error::TransferError;
use crate::manifest::PackageKind;

/// Kategoria elementów paczki (klucz zakresu i katalog w archiwum).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Konfiguracja wspólna (`config/common/*.toml`).
    ConfigCommon,
    /// Nakładka maszyny (`config/machine/<id>.toml`) — domyślnie nie eksportowana.
    ConfigMachine,
    /// Agentki, biblie głosów, słownik wymowy.
    Personas,
    /// Obsady ról.
    Casts,
    /// Reguły Marszałka.
    Rules,
    /// Umiejętności / agenci z Kreatora.
    Skills,
    /// Sesje (NDJSON drzewa tur + metadane).
    Sessions,
    /// Zakresy pamięci (`memory/<zakres>.ndjson`).
    Memory,
    /// Artefakty wybranych sesji (`artifacts/<sesja>/…`).
    Artifacts,
    /// Logi — domyślnie nie eksportowane.
    Logs,
    /// Sekrety (wyłącznie paczka `secrets`).
    Secrets,
}

impl Category {
    /// Kategorie obsługiwane przez [`crate::DocumentStore`] (wszystkie poza sesjami i sekretami).
    pub const DOCUMENTS: [Category; 9] = [
        Category::ConfigCommon,
        Category::ConfigMachine,
        Category::Personas,
        Category::Casts,
        Category::Rules,
        Category::Skills,
        Category::Memory,
        Category::Artifacts,
        Category::Logs,
    ];

    /// Klucz zakresu w manifeście (`config.common`, `sessions`…).
    pub fn key(self) -> &'static str {
        match self {
            Category::ConfigCommon => "config.common",
            Category::ConfigMachine => "config.machine",
            Category::Personas => "personas",
            Category::Casts => "casts",
            Category::Rules => "rules",
            Category::Skills => "skills",
            Category::Sessions => "sessions",
            Category::Memory => "memory",
            Category::Artifacts => "artifacts",
            Category::Logs => "logs",
            Category::Secrets => "secrets",
        }
    }

    /// Katalog w archiwum.
    pub fn dir(self) -> &'static str {
        match self {
            Category::ConfigCommon => "config/common",
            Category::ConfigMachine => "config/machine",
            Category::Secrets => "secrets",
            other => other.key(),
        }
    }
}

/// Wybór elementów (sesji, zakresów pamięci).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "select", content = "items", rename_all = "snake_case")]
pub enum Selection<T> {
    /// Nic.
    #[default]
    None,
    /// Wszystkie (kopie zapasowe).
    All,
    /// Wybrane.
    Only(Vec<T>),
}

impl<T: PartialEq> Selection<T> {
    /// Czy element jest wybrany.
    pub fn includes(&self, item: &T) -> bool {
        match self {
            Selection::None => false,
            Selection::All => true,
            Selection::Only(items) => items.contains(item),
        }
    }

    /// Czy wybór jest pusty.
    pub fn is_none(&self) -> bool {
        matches!(self, Selection::None) || matches!(self, Selection::Only(v) if v.is_empty())
    }
}

/// Zakres eksportu (PLAN §15.1). Domyślnie: konfiguracja wspólna, agentki, obsady, reguły,
/// umiejętności; sesje i pamięć — wybór; artefakty, logi, nakładka maszyny — nie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportScope {
    /// Konfiguracja wspólna.
    pub config_common: bool,
    /// Agentki i biblie głosów.
    pub personas: bool,
    /// Obsady ról.
    pub casts: bool,
    /// Reguły.
    pub rules: bool,
    /// Umiejętności.
    pub skills: bool,
    /// Sesje.
    pub sessions: Selection<SessionId>,
    /// Zakresy pamięci (nazwy dokumentów magazynu pamięci).
    pub memory: Selection<String>,
    /// Artefakty wybranych sesji.
    pub artifacts: bool,
    /// Logi.
    pub logs: bool,
    /// Nakładka tej maszyny.
    pub config_machine: bool,
    /// Sesje z tagiem prywatności (`private`, `local_only`) — tylko jawnie i tylko w paczce szyfrowanej.
    pub include_private: bool,
}

impl Default for ExportScope {
    fn default() -> Self {
        Self {
            config_common: true,
            personas: true,
            casts: true,
            rules: true,
            skills: true,
            sessions: Selection::None,
            memory: Selection::None,
            artifacts: false,
            logs: false,
            config_machine: false,
            include_private: false,
        }
    }
}

impl ExportScope {
    /// Czy kategoria dokumentów jest w zakresie (pamięć i artefakty filtrowane dodatkowo po nazwie).
    pub fn includes(&self, category: Category) -> bool {
        match category {
            Category::ConfigCommon => self.config_common,
            Category::ConfigMachine => self.config_machine,
            Category::Personas => self.personas,
            Category::Casts => self.casts,
            Category::Rules => self.rules,
            Category::Skills => self.skills,
            Category::Sessions => !self.sessions.is_none(),
            Category::Memory => !self.memory.is_none(),
            Category::Artifacts => self.artifacts && !self.sessions.is_none(),
            Category::Logs => self.logs,
            Category::Secrets => false,
        }
    }
}

/// Tryb importu elementu (docs/formats/alfa-package.md §6.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Dodaj tylko brakujące elementy; kolizje pomijane.
    #[default]
    Add,
    /// Scal: konfiguracja klucz po kluczu (paczka nadpisuje), sesje — suma drzew tur.
    Merge,
    /// Zastąp element w całości.
    Replace,
}

/// Tryb globalny z nadpisaniami per kategoria.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModeMap {
    /// Tryb domyślny.
    pub default: ImportMode,
    /// Nadpisania per kategoria.
    pub per_category: BTreeMap<Category, ImportMode>,
}

impl ModeMap {
    /// Jeden tryb dla wszystkiego.
    pub fn all(mode: ImportMode) -> Self {
        Self {
            default: mode,
            per_category: BTreeMap::new(),
        }
    }

    /// Tryb dla kategorii.
    pub fn mode_for(&self, category: Category) -> ImportMode {
        self.per_category
            .get(&category)
            .copied()
            .unwrap_or(self.default)
    }
}

/// Rozstrzygnięcie kolizji sesji (ten sam `id`, inna historia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CollisionResolution {
    /// Scal drzewa tur (suma; nic nie znika).
    Merge,
    /// Zastąp wersję lokalną.
    Replace,
    /// Importuj jako kopię z nowym `id` (tytuł z sufiksem „(import z …)”).
    Copy,
    /// Pomiń.
    Skip,
}

/// Token anulowania (sprawdzany między elementami).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Nowy token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Anuluje operację.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Czy anulowano.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// `Err(Cancelled)`, gdy anulowano.
    pub fn check(token: Option<&CancelToken>) -> Result<(), TransferError> {
        match token {
            Some(t) if t.is_cancelled() => Err(TransferError::Cancelled),
            _ => Ok(()),
        }
    }
}

/// Opcje importu (i dry-run).
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Hasło paczki zaszyfrowanej.
    pub password: Option<SecretString>,
    /// Tryby.
    pub modes: ModeMap,
    /// Rozstrzygnięcia kolizji sesji (klucz: `id` z paczki).
    pub resolutions: BTreeMap<SessionId, CollisionResolution>,
    /// Import nakładki maszyny (domyślnie nie; ostrzeżenie przy innej klasie sprzętu).
    pub include_machine_overlay: bool,
    /// Jawna zgoda na import paczki sekretów.
    pub allow_secrets: bool,
    /// Anulowanie.
    pub cancel: Option<CancelToken>,
}

/// Żądanie eksportu (ręczny eksport albo kopia zapasowa).
#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// Zakres.
    pub scope: ExportScope,
    /// Plik docelowy `.alfa`.
    pub dest: PathBuf,
    /// Rodzaj (`Export` albo `Backup`; `Secrets` — [`crate::Transfer::export_secrets`]).
    pub kind: PackageKind,
    /// Hasło — szyfruje całą paczkę.
    pub password: Option<SecretString>,
    /// Opis od użytkownika.
    pub notes: Option<String>,
    /// Anulowanie.
    pub cancel: Option<CancelToken>,
}

impl ExportRequest {
    /// Eksport zakresu do pliku, bez hasła.
    pub fn new(scope: ExportScope, dest: impl Into<PathBuf>) -> Self {
        Self {
            scope,
            dest: dest.into(),
            kind: PackageKind::Export,
            password: None,
            notes: None,
            cancel: None,
        }
    }
}

/// Minimalna długość hasła paczki.
pub const MIN_PASSWORD_CHARS: usize = 8;

/// Sprawdza hasło paczki (długość w znakach, bez białych znaków na brzegach).
pub fn validate_password(password: &SecretString) -> Result<(), TransferError> {
    if password.expose_secret().trim().chars().count() < MIN_PASSWORD_CHARS {
        return Err(TransferError::WeakPassword {
            min: MIN_PASSWORD_CHARS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scope_matches_plan() {
        let s = ExportScope::default();
        assert!(s.includes(Category::ConfigCommon) && s.includes(Category::Personas));
        assert!(!s.includes(Category::Logs) && !s.includes(Category::ConfigMachine));
        assert!(!s.includes(Category::Sessions) && !s.includes(Category::Artifacts));
        let with = ExportScope {
            sessions: Selection::Only(vec![SessionId::new("s1")]),
            artifacts: true,
            ..ExportScope::default()
        };
        assert!(with.includes(Category::Sessions) && with.includes(Category::Artifacts));
        assert!(Selection::<u8>::Only(vec![]).is_none());
    }

    #[test]
    fn modes_and_password() {
        let mut m = ModeMap::all(ImportMode::Merge);
        m.per_category
            .insert(Category::Sessions, ImportMode::Replace);
        assert_eq!(m.mode_for(Category::Sessions), ImportMode::Replace);
        assert_eq!(m.mode_for(Category::Personas), ImportMode::Merge);
        assert!(validate_password(&SecretString::from("krótkie")).is_err());
        assert!(validate_password(&SecretString::from("długie hasło")).is_ok());
        let t = CancelToken::new();
        assert!(CancelToken::check(Some(&t)).is_ok());
        t.cancel();
        assert_eq!(CancelToken::check(Some(&t)), Err(TransferError::Cancelled));
    }
}
