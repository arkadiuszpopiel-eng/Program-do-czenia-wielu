//! Typy kontraktu czytania: konfiguracja, sterowanie, stan, tekst niezaufany ze zgodą na
//! udostępnienie, zdarzenia (bez treści), błędy.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::text::wrap_untrusted;

/// Konfiguracja (`[voice.readaloud]`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReadAloudCfg {
    /// Tempo startowe.
    pub rate: f32,
    /// Krok „szybciej/wolniej”.
    pub rate_step: f32,
    /// Najwolniej.
    pub min_rate: f32,
    /// Najszybciej.
    pub max_rate: f32,
    /// Najdłuższy fragment (znaki).
    pub max_segment_chars: usize,
    /// Najwięcej znaków z okna (reszta obcinana, flaga `truncated`).
    pub max_chars: usize,
    /// Zapas Ctrl+C dla zaznaczenia (schowek przywracany), gdy UIA nie podaje zaznaczenia.
    pub clipboard_fallback: bool,
    /// Pozwól na chmurowy TTS (domyślnie nie — treść okna może być prywatna).
    pub allow_cloud_tts: bool,
}

impl Default for ReadAloudCfg {
    fn default() -> Self {
        Self {
            rate: 1.0,
            rate_step: 0.1,
            min_rate: 0.5,
            max_rate: 2.0,
            max_segment_chars: 300,
            max_chars: 50_000,
            clipboard_fallback: true,
            allow_cloud_tts: false,
        }
    }
}

/// Co czytać.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadScope {
    /// Zaznaczenie w oknie na pierwszym planie.
    Selection,
    /// Cały tekst dokumentu / pola z fokusem (`TextPattern.DocumentRange`).
    Document,
}

/// Sterowanie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadControl {
    /// Pauza.
    Pause,
    /// Wznów (od początku przerwanego zdania).
    Resume,
    /// Następne zdanie.
    Next,
    /// Poprzednie zdanie.
    Previous,
    /// Szybciej.
    Faster,
    /// Wolniej.
    Slower,
    /// Od początku.
    Restart,
    /// Koniec (treść usuwana z pamięci).
    Stop,
}

/// Faza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadPhase {
    /// Nic nie jest czytane.
    Idle,
    /// Czyta.
    Speaking,
    /// Pauza.
    Paused,
    /// Przeczytano do końca (treść do „od początku” / „wstecz”).
    Finished,
}

/// Stan (UI: podświetlenie zdania, tempo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReadStatus {
    /// Faza.
    pub phase: ReadPhase,
    /// Bieżące zdanie.
    pub index: usize,
    /// Liczba zdań.
    pub segments: usize,
    /// Tempo.
    pub rate: f32,
    /// Zakres bieżącego zdania w tekście (znaki).
    pub highlight: Option<(usize, usize)>,
    /// Aplikacja źródła.
    pub app: Option<String>,
}

/// Tekst z zewnętrznego okna — **niezaufany**: nie ma `Display` ani konwersji do `String`;
/// do modelu trafia wyłącznie przez [`UntrustedText::for_model`] ze zgodą (opakowany
/// w blok niezaufanej treści), do pamięci — nigdy z tego modułu.
#[derive(Clone, PartialEq, Eq)]
pub struct UntrustedText(String);

impl std::fmt::Debug for UntrustedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UntrustedText(<{} znaków>)", self.0.chars().count())
    }
}

impl UntrustedText {
    /// Tekst z okna.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Treść do syntezy mowy i podświetlania (lokalnie).
    pub fn as_untrusted_str(&self) -> &str {
        &self.0
    }

    /// Liczba znaków.
    pub fn char_count(&self) -> usize {
        self.0.chars().count()
    }

    /// Treść dla modelu — tylko z ważną zgodą użytkownika, opakowana jako niezaufana
    /// (model widzi dane, nie polecenia).
    pub fn for_model(&self, consent: Option<&ShareConsent>, source: &str) -> Option<String> {
        consent
            .filter(|c| c.confirmed_by_user)
            .map(|_| wrap_untrusted(&self.0, source, "czytanie"))
    }
}

/// Zgoda użytkownika na pokazanie czytanego tekstu modelowi (np. „streść to”).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShareConsent {
    /// Potwierdzone przez użytkownika (UI, nie z treści).
    pub confirmed_by_user: bool,
}

/// Tekst do przeczytania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceText {
    /// Treść (niezaufana).
    pub text: UntrustedText,
    /// Aplikacja (nazwa pliku procesu).
    pub app: String,
    /// Zakres.
    pub scope: ReadScope,
    /// Obcięto do `max_chars`.
    pub truncated: bool,
}

/// Dlaczego odmowa odczytu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefuseReason {
    /// Brak okna na pierwszym planie.
    NoForeground,
    /// Okno Alfy/Brokera albo procesu nieznanego.
    ProtectedTarget,
    /// Pole hasła (albo nie da się tego wykluczyć dla zapasu Ctrl+C).
    PasswordField,
    /// Brak zaznaczenia / tekstu.
    NoText,
    /// Aplikacja nie udostępnia tekstu (brak `TextPattern`, zapas wyłączony).
    Unsupported,
}

/// Zdarzenia (bez czytanej treści).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ReadAloudEvent {
    /// `voice.readaloud.started`.
    Started {
        /// Aplikacja.
        app: String,
        /// Zakres.
        scope: ReadScope,
        /// Zdania.
        segments: u32,
        /// Znaki.
        chars: u32,
        /// Obcięto.
        truncated: bool,
    },
    /// `voice.readaloud.segment` — zaczęto zdanie.
    Segment {
        /// Indeks.
        index: u32,
        /// Z ilu.
        of: u32,
    },
    /// `voice.readaloud.paused`.
    Paused,
    /// `voice.readaloud.resumed`.
    Resumed,
    /// `voice.readaloud.rate`.
    Rate {
        /// Tempo (‰).
        permille: u32,
    },
    /// `voice.readaloud.finished`.
    Finished,
    /// `voice.readaloud.stopped`.
    Stopped,
    /// `voice.readaloud.refused`.
    Refused {
        /// Powód.
        reason: RefuseReason,
    },
    /// `voice.readaloud.failed`.
    Failed {
        /// Powód (PL, bez treści).
        reason: String,
    },
}

/// Błędy.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum ReadAloudError {
    /// Odmowa odczytu.
    #[error("nie mogę przeczytać: {0:?}")]
    Refused(RefuseReason),
    /// Zgoda wymagana (udostępnienie modelowi).
    #[error("udostępnienie czytanego tekstu wymaga zgody")]
    ConsentRequired,
    /// Nic nie jest czytane.
    #[error("nic nie jest czytane")]
    NotReading,
    /// Mowa / głośnik / platforma.
    #[error("{0}")]
    Platform(String),
}
