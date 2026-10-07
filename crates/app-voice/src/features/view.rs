//! Widok głosu rozszerzonego dla UI (panel Głos, Ustawienia → Głos) i komunikaty PL/EN.
//! Widok nie zawiera audio, embeddingu ani czytanego tekstu.

use app_api::dto::{
    DictationStateView, DictationView, LocalizedText, ReadAloudView, ReadStateView, S2sView,
    SpeakerState, SpeakerView, VoiceFeatures, WakeCalibrationView, WakeWordsState, WakeWordsView,
};
use personas_contract::builtin_personas;
use voice_dictation_contract::{DictationPhase, PauseReason, RefuseReason as DictRefuse};
use voice_readaloud_contract::{ReadPhase, RefuseReason as ReadRefuse};
use voice_speaker_contract::{EnrollmentStatus, MIN_ENROLL_UTTERANCES};

use super::{F5, F5State, dictation, read, speaker, wake};

/// Brak modelu słów wywoławczych.
pub const NO_KWS_MODEL: &str =
    "Brak modelu słów wywoławczych — umieść model `*.kws.json` w katalogu modeli `kws`.";
/// Brak modelu rozpoznawania głosu.
pub const NO_SPEAKER_MODEL: &str =
    "Brak modelu rozpoznawania głosu — umieść model `*.speaker.json` w katalogu modeli `speaker`.";

fn text(pl: impl Into<String>, en: impl Into<String>) -> LocalizedText {
    LocalizedText::new(pl, en)
}

/// Odmowa dyktowania (PL/EN).
pub fn dictation_refusal(reason: DictRefuse) -> LocalizedText {
    match reason {
        DictRefuse::NoForeground => text(
            "Brak okna na pierwszym planie — przejdź do okna, w którym chcesz dyktować.",
            "No window in the foreground — switch to the window you want to dictate into.",
        ),
        DictRefuse::ProtectedTarget => text(
            "To okno jest chronione (Alfa, Broker albo nieznany program) — dyktowanie wyłączone.",
            "This window is protected (Alfa, Broker or an unknown app) — dictation is off.",
        ),
        DictRefuse::ElevatedTarget => text(
            "Na pierwszym planie jest okno administratora — dyktowanie do niego nie działa.",
            "An administrator window is in the foreground — dictation cannot type into it.",
        ),
        DictRefuse::PasswordField => text(
            "Pole hasła — dyktowanie wyłączone.",
            "Password field — dictation is off.",
        ),
    }
}

/// Odmowa czytania (PL/EN).
pub fn read_refusal(reason: ReadRefuse) -> LocalizedText {
    match reason {
        ReadRefuse::NoForeground => text(
            "Brak okna na pierwszym planie.",
            "No window in the foreground.",
        ),
        ReadRefuse::ProtectedTarget => text(
            "To okno jest chronione (Alfa, Broker albo nieznany program) — nie czytam go.",
            "This window is protected (Alfa, Broker or an unknown app) — not reading it.",
        ),
        ReadRefuse::PasswordField => {
            text("Pole hasła — nie czytam.", "Password field — not reading.")
        }
        ReadRefuse::NoText => text(
            "Nic do przeczytania — zaznacz tekst albo skopiuj go do schowka.",
            "Nothing to read — select some text or copy it to the clipboard.",
        ),
        ReadRefuse::Unsupported => text(
            "Ta aplikacja nie udostępnia tekstu do czytania.",
            "This app does not expose text for reading.",
        ),
    }
}

fn pause_text(reason: PauseReason) -> LocalizedText {
    match reason {
        PauseReason::FocusChanged => text(
            "Wstrzymane: na pierwszym planie jest inne okno (tekst czeka).",
            "Paused: another window is in the foreground (text is waiting).",
        ),
        PauseReason::UserTyping => text(
            "Wstrzymane: piszesz albo używasz myszy.",
            "Paused: you are typing or using the mouse.",
        ),
    }
}

fn unavailable(pl: &str, en: &str) -> Option<LocalizedText> {
    Some(text(pl, en))
}

fn wake_view(st: &F5State) -> WakeWordsView {
    let cal = st.calibration;
    let calibration = WakeCalibrationView {
        measured: cal.is_some(),
        sufficient: cal.is_some_and(|c| c.sufficient),
        passes: cal.is_some_and(|c| c.passes),
        far_per_day: cal.map(|c| c.far_per_day).filter(|v| v.is_finite()),
        frr: cal.map(|c| c.frr),
        threshold: f64::from(cal.map_or(wake::DEFAULT_THRESHOLD, |c| c.threshold)),
    };
    let w = &st.wake;
    let model = st.kws_model;
    let (state, reason) = if !model {
        (
            WakeWordsState::Unavailable,
            unavailable(NO_KWS_MODEL, "No wake word model (`*.kws.json`)."),
        )
    } else if !w.armed && !w.suspended {
        let reason = w
            .error
            .as_ref()
            .filter(|_| st.settings.wake_enabled)
            .map(|e| text(e.clone(), "Wake words could not start."));
        (WakeWordsState::Off, reason)
    } else if w.listening {
        (WakeWordsState::Listening, None)
    } else if w.suspended || w.dnd || w.muted {
        let reason = if w.dnd {
            text(
                "„Nie przeszkadzać” — nasłuch wstrzymany.",
                "Do not disturb — paused.",
            )
        } else if w.muted {
            text(
                "Mikrofon wyciszony — nasłuch wstrzymany.",
                "Microphone muted — paused.",
            )
        } else {
            text(
                "Mikrofon zajęty dyktowaniem albo nagraniem — nasłuch wstrzymany.",
                "Microphone busy with dictation or recording — paused.",
            )
        };
        (WakeWordsState::Suspended, Some(reason))
    } else {
        (WakeWordsState::Armed, None)
    };
    WakeWordsView {
        state,
        reason,
        enabled: st.settings.wake_enabled,
        risk_accepted: st.settings.wake_accept_risk,
        owner_gate: st.settings.wake_owner_gate,
        dnd: w.dnd,
        phrases: builtin_personas()
            .iter()
            .flat_map(|p| p.wake_phrases.clone())
            .collect(),
        calibration,
        test: w.test.clone(),
    }
}

fn speaker_view(status: Result<EnrollmentStatus, String>, st: &F5State) -> SpeakerView {
    let needed = MIN_ENROLL_UTTERANCES as u32;
    let mut v = SpeakerView {
        state: SpeakerState::Unavailable,
        reason: None,
        done: 0,
        needed,
        recording: st.enroll.recording.is_some(),
        prompts: speaker::prompts(),
        last_sample: st.enroll.last_sample.clone(),
        required_for_risky: st.settings.speaker_required,
        last_check: st.last_check,
    };
    match status {
        Err(e) => v.reason = Some(text(e, "Voice recognition is unavailable.")),
        Ok(status) => match status {
            EnrollmentStatus::NotEnrolled => v.state = SpeakerState::NotEnrolled,
            EnrollmentStatus::Enrolling { done, needed, .. } => {
                v.state = SpeakerState::Enrolling;
                v.done = done;
                v.needed = needed;
            }
            EnrollmentStatus::Enrolled { utterances, .. } => {
                v.state = SpeakerState::Enrolled;
                v.done = utterances;
            }
        },
    }
    v
}

fn dictation_view(f5: &F5, st: &F5State) -> DictationView {
    let d = &st.dictation;
    let available = f5.deps.desktop.is_some() && f5.parts().is_some();
    let status = d.status.as_ref().filter(|_| d.active());
    let state = match (available, status.map(|s| s.phase)) {
        (false, _) => DictationStateView::Unavailable,
        (true, Some(DictationPhase::Active)) => DictationStateView::Active,
        (true, Some(DictationPhase::Paused(_))) => DictationStateView::Paused,
        (true, _) => DictationStateView::Idle,
    };
    let reason = match status.map(|s| s.phase) {
        _ if !available => unavailable(
            "Dyktowanie wymaga modułów pulpitu (Windows) i modelu STT.",
            "Dictation needs the desktop modules (Windows) and an STT model.",
        ),
        Some(DictationPhase::Paused(r)) => Some(pause_text(r)),
        _ => d.refused.clone(),
    };
    DictationView {
        state,
        reason,
        app: status.and_then(|s| s.app.clone()),
        typed_chars: status.map_or(0, |s| s.typed_chars as u64),
        pending_chars: status.map_or(0, |s| s.pending_chars as u64),
        can_undo: status.is_some_and(|s| s.can_undo),
        preview: d.preview.clone().filter(|_| d.active()),
        shortcut: dictation::SHORTCUT.into(),
        profiles: st.settings.profiles.clone(),
    }
}

fn read_view(f5: &F5, st: &F5State) -> ReadAloudView {
    let r = &st.read;
    let available = f5.parts().is_some();
    let status = r.status.as_ref().filter(|_| r.active());
    let state = match (available, status.map(|s| s.phase)) {
        (false, _) => ReadStateView::Unavailable,
        (true, Some(ReadPhase::Speaking)) => ReadStateView::Speaking,
        (true, Some(ReadPhase::Paused)) => ReadStateView::Paused,
        (true, _) => ReadStateView::Idle,
    };
    ReadAloudView {
        state,
        reason: if available {
            r.refused.clone()
        } else {
            unavailable(
                "Czytanie wymaga syntezy mowy (Pocket TTS / Piper).",
                "Reading needs speech synthesis (Pocket TTS / Piper).",
            )
        },
        app: status.and_then(|s| s.app.clone()),
        index: status.map_or(0, |s| u32::try_from(s.index).unwrap_or(u32::MAX)),
        segments: status.map_or(0, |s| u32::try_from(s.segments).unwrap_or(u32::MAX)),
        rate: f64::from(status.map_or(st.settings.read_rate, |s| s.rate)),
        queued: u32::try_from(r.queued).unwrap_or(u32::MAX),
        agent: st.agent.clone(),
        shortcut: read::SHORTCUT.into(),
    }
}

/// Widok bieżący.
pub fn build(f5: &F5) -> VoiceFeatures {
    // Model mówcy może się ładować — poza blokadą stanu.
    let speaker_status = f5.verifier().map(|v| v.status());
    let st = f5.lock();
    let mut out = VoiceFeatures::unavailable(text(
        "Głos niedostępny: pobierz modele w Ustawieniach → Głos.",
        "Voice unavailable: download the models in Settings → Voice.",
    ));
    out.wake = wake_view(&st);
    out.dictation = dictation_view(f5, &st);
    out.read = read_view(f5, &st);
    out.speaker = speaker_view(speaker_status, &st);
    out.s2s = S2sView {
        available: false,
        reason: out.s2s.reason.clone(),
    };
    out
}
