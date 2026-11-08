//! Błędy: cyklu życia ([`PluginError`]), ładowania modułu ([`LoadError`]) i wykonania
//! w piaskownicy ([`ExecError`]) — komunikaty po polsku, trafiają do UI i do modelu.
//! Żaden błąd wtyczki nie jest paniką hosta: pułapka Wasm = czytelny wynik narzędzia.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::text::{redact_secrets, truncate_chars};
use tools_common_contract::{ToolErrorKind, ToolOutcome};

use crate::model::PluginState;

/// Najdłuższy tekst z wtyczki w komunikacie błędu (znaki).
pub const MAX_ERROR_CHARS: usize = 400;

/// Błąd ładowania modułu Wasm (przy propozycji i przed każdym uruchomieniem po starcie).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "load_error", content = "detail", rename_all = "snake_case")]
pub enum LoadError {
    /// Moduł większy niż limit.
    #[error("moduł Wasm za duży ({0} B)")]
    TooLarge(usize),
    /// To nie jest komponent Wasm (np. moduł rdzeniowy z importami WASI).
    #[error("to nie jest komponent Wasm (wymagany komponent świata `alfa:plugin/plugin`)")]
    NotComponent,
    /// Kompilacja/walidacja odrzuciła moduł.
    #[error("moduł Wasm niepoprawny: {0}")]
    Invalid(String),
    /// Import spoza interfejsu hosta Alfy (WASI, inne interfejsy).
    #[error("niedozwolony import `{0}` — wtyczka ma dostęp do świata wyłącznie przez host Alfy")]
    ForbiddenImport(String),
    /// Import hosta o złym kształcie (inne funkcje, inne typy).
    #[error("import hosta niezgodny z WIT: {0}")]
    ImportType(String),
    /// Brak wymaganego eksportu.
    #[error("brak eksportu `{0}`")]
    MissingExport(String),
    /// Eksport o złym typie.
    #[error("eksport niezgodny z WIT: {0}")]
    ExportType(String),
    /// Bajty modułu nie zgadzają się z hashem zatwierdzonej wersji.
    #[error("hash modułu ({actual}) różni się od zatwierdzonego ({expected}) — odmowa ładowania")]
    HashMismatch {
        /// Oczekiwany (z zatwierdzonego manifestu).
        expected: String,
        /// Rzeczywisty.
        actual: String,
    },
    /// Brak bajtów modułu w magazynie.
    #[error("brak modułu Wasm w magazynie")]
    MissingModule,
    /// Rekord nie zgadza się z zatwierdzeniem (manifest zmieniony poza biblioteką).
    #[error("wersja niezatwierdzona albo manifest zmieniony po zatwierdzeniu — odmowa ładowania")]
    NotApproved,
}

/// Błąd operacji na wtyczkach.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// Niepoprawny manifest.
    #[error("niepoprawna wtyczka: {0}")]
    Invalid(String),
    /// Zdolność zabroniona wtyczkom.
    #[error("wtyczka nie może deklarować zdolności `{0}`")]
    ForbiddenCapability(String),
    /// Nie ma takiej wtyczki (albo wersji).
    #[error("nie ma wtyczki `{0}`")]
    NotFound(String),
    /// Operacja niedozwolona w tym stanie.
    #[error("operacja niedozwolona w stanie {0:?}")]
    WrongState(PluginState),
    /// Zatwierdzenie dotyczy innej treści niż przejrzana.
    #[error("zatwierdzenie dotyczy innej wersji (hash przejrzanego manifestu niezgodny)")]
    HashMismatch,
    /// Kanał zatwierdzenia niewystarczający (instalacja kodu tylko kliknięciem w oknie).
    #[error("instalacja wtyczki wymaga zatwierdzenia w oknie (nie głosem ani tekstem)")]
    ApprovalChannel,
    /// Ta sama wersja z inną treścią już istnieje.
    #[error("wersja {0} już istnieje z inną treścią — podnieś wersję")]
    VersionExists(String),
    /// Nowa wersja musi być wyższa niż zainstalowana.
    #[error("wersja {0} nie jest wyższa od zainstalowanej")]
    NotNewer(String),
    /// Nazwa narzędzia zajęta przez inną zainstalowaną wtyczkę.
    #[error("narzędzie `{0}` dostarcza już inna wtyczka")]
    ToolConflict(String),
    /// Moduł Wasm odrzucony.
    #[error("{0}")]
    Load(LoadError),
    /// Magazyn.
    #[error("magazyn wtyczek: {0}")]
    Store(String),
}

/// Dlaczego wywołanie wtyczki się nie powiodło.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "exec_error", content = "detail", rename_all = "snake_case")]
pub enum ExecError {
    /// Wyczerpane paliwo (np. nieskończona pętla).
    #[error("wtyczka przekroczyła limit paliwa (instrukcji) i została przerwana")]
    OutOfFuel,
    /// Przekroczony czas wykonania (epoch).
    #[error("wtyczka przekroczyła limit czasu i została przerwana")]
    Timeout,
    /// Anulowano (kill-switch, „stop”).
    #[error("wywołanie wtyczki anulowano")]
    Cancelled,
    /// Przekroczony limit pamięci.
    #[error("wtyczka przekroczyła limit pamięci i została przerwana")]
    MemoryLimit,
    /// Przepełnienie stosu.
    #[error("wtyczka przepełniła stos i została przerwana")]
    StackOverflow,
    /// Inna pułapka Wasm (np. `unreachable`, dostęp poza pamięcią).
    #[error("pułapka Wasm: {0}")]
    Trap(String),
    /// Za duże wejście.
    #[error("argumenty za duże ({bytes} B > {limit} B)")]
    InputTooLarge {
        /// Rozmiar.
        bytes: usize,
        /// Limit.
        limit: usize,
    },
    /// Za duże wyjście.
    #[error("wynik wtyczki za duży ({bytes} B > {limit} B)")]
    OutputTooLarge {
        /// Rozmiar.
        bytes: usize,
        /// Limit.
        limit: usize,
    },
    /// Wynik nie jest poprawnym JSON-em (albo przekracza głębokość).
    #[error("wynik wtyczki nie jest poprawnym JSON-em: {0}")]
    InvalidOutput(String),
    /// Wtyczka zwróciła błąd (tekst niezaufany, obcięty).
    #[error("wtyczka zgłosiła błąd: {0}")]
    PluginFailed(String),
    /// Przekroczony limit operacji hosta.
    #[error("wtyczka przekroczyła limit operacji hosta i została przerwana")]
    HostCallLimit,
    /// Wtyczka niezainstalowana, wyłączona albo nieznane narzędzie.
    #[error("wtyczka niedostępna: {0}")]
    Unavailable(String),
    /// Moduł nie przeszedł ładowania.
    #[error("{0}")]
    Load(LoadError),
    /// Błąd hosta (wątek, kompilacja).
    #[error("błąd wewnętrzny piaskownicy: {0}")]
    Internal(String),
}

/// Tekst z wtyczki do komunikatu: redakcja sekretów, obcięcie, bez znaków sterujących.
pub fn sanitize(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    truncate_chars(&redact_secrets(&clean), MAX_ERROR_CHARS).0
}

impl ExecError {
    /// Czy to przerwanie przez limit piaskownicy (zdarzenie `plugin.trapped` dla Diagnosty).
    pub fn is_sandbox_stop(&self) -> bool {
        matches!(
            self,
            Self::OutOfFuel
                | Self::Timeout
                | Self::MemoryLimit
                | Self::StackOverflow
                | Self::Trap(_)
                | Self::HostCallLimit
                | Self::OutputTooLarge { .. }
                | Self::InvalidOutput(_)
        )
    }

    /// Krótki rodzaj (ładunki zdarzeń, metryki).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OutOfFuel => "out_of_fuel",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::MemoryLimit => "memory_limit",
            Self::StackOverflow => "stack_overflow",
            Self::Trap(_) => "trap",
            Self::InputTooLarge { .. } => "input_too_large",
            Self::OutputTooLarge { .. } => "output_too_large",
            Self::InvalidOutput(_) => "invalid_output",
            Self::PluginFailed(_) => "plugin_failed",
            Self::HostCallLimit => "host_call_limit",
            Self::Unavailable(_) => "unavailable",
            Self::Load(_) => "load",
            Self::Internal(_) => "internal",
        }
    }

    /// Wynik narzędzia dla modelu (bez paniki, z czytelnym powodem).
    pub fn to_outcome(&self, action: &str) -> ToolOutcome {
        let kind = match self {
            Self::Cancelled => return ToolOutcome::cancelled(action),
            Self::OutOfFuel | Self::Timeout => ToolErrorKind::Timeout,
            Self::InputTooLarge { .. } => ToolErrorKind::InvalidArgs,
            Self::Unavailable(_) => ToolErrorKind::NotFound,
            Self::PluginFailed(_) => ToolErrorKind::Io,
            _ => ToolErrorKind::Internal,
        };
        let mut out = ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {self}."));
        out.data = serde_json::json!({ "error": self.kind() });
        out
    }
}
