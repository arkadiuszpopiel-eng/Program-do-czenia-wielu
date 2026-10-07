//! Model biblioteki wtyczek: źródło, stan wersji, zatwierdzenie właściciela (z hashem
//! przejrzanej wersji — podmiana manifestu albo modułu po przeglądzie jest odrzucana).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest::PluginManifest;

/// Źródło wtyczki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PluginSource {
    /// Właściciel (plik wskazany w UI).
    User,
    /// Ulepszacz — pierścień R2 (`improver`, propozycja `P-…`).
    Improver {
        /// Identyfikator propozycji Ulepszacza.
        proposal: String,
    },
    /// Import paczki (własna `.alfa` albo z zewnątrz).
    Import {
        /// Paczka spoza maszyn właściciela.
        external: bool,
    },
}

/// Stan wersji wtyczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Czeka na zatwierdzenie właściciela (nieuruchamialna, narzędzia niewidoczne).
    Proposed,
    /// Zainstalowana (aktywna wersja — narzędzia w rejestrze agentek).
    Installed,
    /// Wyłączona przez właściciela (ponowne włączenie = ponowne zatwierdzenie).
    Disabled,
    /// Odrzucona.
    Rejected,
    /// Zastąpiona nowszą zainstalowaną wersją.
    Superseded,
}

/// Kanał zatwierdzenia (wyłącznie właściciel; agentka nie ma tu wariantu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalOrigin {
    /// Kliknięcie w oknie (karta wtyczki / „Zdrowie systemu”) — jedyny kanał wystarczający.
    Ui,
    /// Polecenie tekstowe (niewystarczające dla kodu R2).
    Text,
    /// Polecenie głosowe (niewystarczające dla kodu R2).
    Voice,
}

/// Zatwierdzenie właściciela: kanał + hash przejrzanego manifestu (obejmuje hash modułu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PluginApproval {
    /// Kanał.
    pub origin: ApprovalOrigin,
    /// Hash przejrzanej wersji ([`crate::review_hash`]).
    pub reviewed_hash: String,
    /// Podpis zatwierdzenia R2 kluczem TPM (PLAN §12.1; weryfikuje kompozycja — otwarte).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl PluginApproval {
    /// Zatwierdzenie kliknięciem w oknie.
    pub fn ui(reviewed_hash: impl Into<String>) -> Self {
        Self {
            origin: ApprovalOrigin::Ui,
            reviewed_hash: reviewed_hash.into(),
            signature: None,
        }
    }
}

/// Wersja wtyczki w bibliotece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginRecord {
    /// Manifest.
    pub manifest: PluginManifest,
    /// SHA-256 kanonicznego manifestu (hex) — to zatwierdza właściciel.
    pub review_hash: String,
    /// Źródło.
    pub source: PluginSource,
    /// Stan.
    pub state: PluginState,
    /// Zatwierdzenie (instalacja / ponowne włączenie).
    #[serde(default)]
    pub approval: Option<PluginApproval>,
    /// Kiedy zgłoszono (ms).
    pub proposed_at_ms: u64,
    /// Kiedy rozstrzygnięto (ms).
    #[serde(default)]
    pub decided_at_ms: Option<u64>,
}

impl PluginRecord {
    /// Czy narzędzia wtyczki są w rejestrze agentek.
    pub fn is_active(&self) -> bool {
        self.state == PluginState::Installed
    }
}
