//! Polecenia wyjściowe automatu i powiadomienia.

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_cmd_contract::VoiceCommand;

use crate::{DialogPhase, ProactiveLabel, TurnId, UtteranceId};

/// Źródło prefiksu (hierarchia PLAN §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrefixSource {
    /// Znaczniki słów z TTS.
    WordMarks,
    /// Forced alignment.
    Alignment,
    /// Zliczanie odtworzonych próbek skorygowane o opóźnienie urządzenia.
    SampleCount,
    /// Nic jeszcze nie zostało odtworzone.
    NothingPlayed,
}

/// Co użytkownik usłyszał przed przerwaniem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeardPrefix {
    /// Wypowiedź.
    pub utterance: UtteranceId,
    /// Liczba usłyszanych znaków (prefiks pełnego tekstu).
    pub chars: usize,
    /// Liczba usłyszanych słów.
    pub words: usize,
    /// Tekst prefiksu.
    pub text: String,
    /// Przybliżony (liczenie próbek bez znaczników).
    pub approximate: bool,
    /// Źródło.
    pub source: PrefixSource,
}

/// Klasa intencji przerwania (PLAN §6.5, VOICE.md §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterruptIntent {
    /// „nie, chodziło mi o…”.
    Correction,
    /// „a jeszcze…”.
    Addition,
    /// „czyli…?”.
    Clarify,
    /// „zostaw, powiedz mi o…”.
    TopicChange,
    /// „stop”, „zostaw to”.
    StopCancel,
    /// „dalej”, „kontynuuj”.
    Continue,
    /// „mhm”, „tak” — nie przerwanie.
    Backchannel,
}

/// Źródło tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnSource {
    /// Głos.
    Voice,
    /// Composer.
    Text,
}

/// Dlaczego odrzucono mowę proaktywną.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProactiveRejection {
    /// „Nie przeszkadzać”.
    DoNotDisturb,
    /// Nie w `Idle` (np. użytkownik mówi).
    NotIdle,
    /// Wyłączona w ustawieniach.
    Disabled,
    /// Głośnik zajęty.
    SpeakerBusy,
}

/// Powiadomienia (→ zdarzenia `voice.dialog.*` na magistrali).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "notice", rename_all = "snake_case")]
pub enum DialogNotice {
    /// Zmiana fazy.
    StateChanged {
        /// Z.
        from: DialogPhase,
        /// Do.
        to: DialogPhase,
    },
    /// Wyciszono TTS (krok 1 zatrzymania).
    Ducked,
    /// Przywrócono głośność TTS.
    Restored,
    /// Backchannel użytkownika (nie przerwał).
    Backchannel {
        /// Tekst.
        text: String,
    },
    /// Przerwanie (twardy stop).
    Interrupted {
        /// Usłyszany prefiks.
        heard: HeardPrefix,
    },
    /// Sklasyfikowano intencję przerwania.
    IntentClassified {
        /// Intencja.
        intent: InterruptIntent,
        /// Pewność.
        confidence: f32,
    },
    /// Mowa proaktywna odrzucona.
    ProactiveRejected {
        /// Powód.
        reason: ProactiveRejection,
    },
    /// Głośnik zajęty — wypowiedź czeka.
    SpeakerBusy {
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Zdarzenie zignorowane (diagnostyka).
    Ignored {
        /// Opis.
        reason: String,
    },
}

/// Polecenie wyjściowe (wykonuje runtime potoku głosu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Otwórz mikrofon / słuchaj.
    StartListening,
    /// Zamknij mikrofon.
    StopListening,
    /// Ducking TTS (krok 1, < 50 ms).
    DuckOutput {
        /// Tłumienie w dB.
        db: f32,
    },
    /// Przywróć głośność TTS.
    RestoreOutput,
    /// Stop TTS tej wypowiedzi (krok 2 — twardy stop).
    StopTts {
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Anuluj generowanie odpowiedzi LLM (bieżącej odpowiedzi mówionej).
    CancelGeneration,
    /// Wyczyść kolejkę mowy.
    ClearSpeechQueue,
    /// Anuluj bieżące zadanie agentki („anuluj”).
    CancelTask,
    /// Poproś o zasób „głośnik”.
    AcquireSpeaker {
        /// Persona.
        persona: PersonaId,
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Zwolnij zasób „głośnik”.
    ReleaseSpeaker {
        /// Persona.
        persona: PersonaId,
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Zacznij odtwarzać odpowiedź (głośnik przydzielony).
    StartTts {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Persona.
        persona: PersonaId,
    },
    /// Wypowiedz tekst proaktywny z etykietą.
    SpeakProactive {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Persona.
        persona: PersonaId,
        /// Tekst.
        text: String,
        /// Etykieta (kto i dlaczego) — pokazywana w UI.
        label: ProactiveLabel,
    },
    /// Wznów wypowiedź od punktu cięcia jako nową wypowiedź.
    ResumeFrom {
        /// Przerwana wypowiedź.
        from: UtteranceId,
        /// Punkt cięcia (znaki pełnego tekstu).
        offset: usize,
        /// Znany tekst od punktu cięcia.
        text: String,
        /// Nowa wypowiedź.
        utterance: UtteranceId,
    },
    /// Przekaż turę do LLM.
    SubmitTurn {
        /// Tura.
        turn: TurnId,
        /// Tekst użytkownika.
        text: String,
        /// Co użytkownik usłyszał (gdy tura przerwała mowę).
        heard_prefix: Option<HeardPrefix>,
        /// Intencja przerwania.
        interrupted_intent: Option<InterruptIntent>,
        /// Źródło.
        source: TurnSource,
    },
    /// Zagraj filler (poza prefiksem, przerywalny).
    PlayFiller,
    /// Przerwij filler.
    StopFiller,
    /// Komenda do wykonania poza automatem (głośność, mikrofon, zmiana persony).
    ForwardCommand {
        /// Komenda.
        command: VoiceCommand,
    },
    /// Kill-switch („stop wszystko”) — obsługuje watchdog/broker.
    KillSwitch,
    /// Powiadomienie (zdarzenie na magistrali).
    Notify {
        /// Treść.
        notice: DialogNotice,
    },
}
