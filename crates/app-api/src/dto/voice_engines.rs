//! Silniki rozmowy (`voice_engines`, część `VoiceFeatures`): który silnik rozpoznaje mowę, czy
//! pracuje strażnik i bezpiecznik, co ocenia koniec wypowiedzi i które głosy są gotowe. Widok nie
//! zawiera nagrań ani rozpoznanego tekstu — tylko liczniki i czasy.

use serde::{Deserialize, Serialize};

/// Silnik rozpoznawania mowy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SttEngineKind {
    /// Qwen3-ASR na `llama-server`.
    QwenAsr,
    /// Whisper na `whisper-server`.
    Whisper,
    /// Brak silnika.
    #[default]
    None,
}

/// Stan bezpiecznika silnika głównego.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BreakerStateView {
    /// Silnik główny pracuje.
    #[default]
    Closed,
    /// Silnik główny odstawiony — rozpoznaje zapasowy.
    Open,
    /// Po przerwie: jedna próba silnika głównego.
    HalfOpen,
}

/// Strażnik rozpoznawania: bezpiecznik i skrót telemetrii z ostatnich wypowiedzi.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SttGuardView {
    pub breaker: BreakerStateView,
    /// Awarie z rzędu.
    pub failures: u32,
    /// Próg odstawienia silnika.
    pub failures_to_open: u32,
    /// Ile razy bezpiecznik zadziałał od startu rozmowy.
    pub opened_count: u32,
    /// Sekundy do następnej próby (gdy otwarty).
    pub retry_in_s: Option<u32>,
    /// Mediana czasu rozpoznania (ms).
    pub median_ms: Option<u32>,
    /// 95. centyl czasu rozpoznania (ms).
    pub p95_ms: Option<u32>,
    /// Liczba wypowiedzi w telemetrii.
    pub samples: u32,
    /// Ile z nich rozpoznał silnik zapasowy.
    pub fallbacks: u32,
    /// Ile wyników miało zastrzeżenie strażnika.
    pub flagged: u32,
}

/// Rozpoznawanie mowy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SttEngineView {
    /// Silnik główny.
    pub engine: SttEngineKind,
    /// Silnik główny liczy na karcie graficznej.
    pub gpu: bool,
    /// Silnik zapasowy (gdy główny ma strażnika).
    pub fallback: Option<SttEngineKind>,
    /// Pozycje katalogu, których pobranie włączy lepszy silnik (pusto — lepszego nie ma).
    pub missing: Vec<String>,
    /// Stan strażnika (`None` — rozmowa ani dyktowanie jeszcze nie ruszyły albo strażnika nie ma).
    pub guard: Option<SttGuardView>,
}

/// Co ocenia koniec wypowiedzi.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TurnEngineKind {
    /// Model Smart Turn słuchający dźwięku.
    SmartTurn,
    /// Ocena z tekstu i długości ciszy.
    #[default]
    Text,
}

/// Praca modelu końca wypowiedzi od startu programu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnStatsView {
    /// Udane oceny modelu.
    pub scored: u32,
    /// Oceny nieudane (odpowiedziała ocena z tekstu).
    pub failed: u32,
    /// Ostatnia pewność końca wypowiedzi (%).
    pub last_pct: Option<u8>,
    /// Czas ostatniej oceny (ms).
    pub last_ms: Option<u32>,
}

/// Koniec wypowiedzi.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnEngineView {
    pub engine: TurnEngineKind,
    /// Pozycje katalogu, których pobranie włączy Smart Turn (pusto — już działa).
    pub missing: Vec<String>,
    /// Liczniki modelu (`None` — model jeszcze niczego nie oceniał).
    pub stats: Option<TurnStatsView>,
}

/// Silnik mowy z łańcucha (kolejność = kolejność prób).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TtsEngineView {
    /// Nazwa silnika (`supertonic`, `pocket`, `piper`).
    pub engine: String,
    /// Pliki silnika i głosu są na miejscu.
    pub ready: bool,
}

/// Silniki rozmowy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VoiceEnginesView {
    pub stt: SttEngineView,
    pub turn: TurnEngineView,
    pub tts: Vec<TtsEngineView>,
}

/// Akcja na silnikach rozmowy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnginesAction {
    /// Odczyt bieżącego stanu (liczniki zmieniają się w trakcie rozmowy).
    Refresh,
    /// Zamknięcie bezpiecznika: silnik główny dostaje szansę od razu, bez czekania na koniec przerwy.
    ResetBreaker,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_are_stable() {
        let view = VoiceEnginesView {
            stt: SttEngineView {
                engine: SttEngineKind::QwenAsr,
                gpu: true,
                fallback: Some(SttEngineKind::Whisper),
                missing: Vec::new(),
                guard: Some(SttGuardView {
                    breaker: BreakerStateView::HalfOpen,
                    failures_to_open: 3,
                    ..SttGuardView::default()
                }),
            },
            turn: TurnEngineView {
                engine: TurnEngineKind::SmartTurn,
                missing: Vec::new(),
                stats: None,
            },
            tts: vec![TtsEngineView {
                engine: "supertonic".into(),
                ready: true,
            }],
        };
        let json = serde_json::to_value(&view).unwrap_or_default();
        assert_eq!(json["stt"]["engine"], "qwen_asr");
        assert_eq!(json["stt"]["guard"]["breaker"], "half_open");
        assert_eq!(json["turn"]["engine"], "smart_turn");
        let action = serde_json::to_value(EnginesAction::ResetBreaker).unwrap_or_default();
        assert_eq!(action["kind"], "reset_breaker");
        assert_eq!(VoiceEnginesView::default().stt.engine, SttEngineKind::None);
    }
}
