//! Powiadomienia natywne (toast Windows) gdy okno główne jest ukryte (PLAN §14.8, SPEC notify):
//! zakończenie odpowiedzi, błąd, prośba o zatwierdzenie (bez przycisku zatwierdzania — tylko
//! przekierowanie do okna Brokera), raport dzienny Marszałka. Treść bez sekretów i bez treści odpowiedzi.

use crate::dto::{AlfaEvent, StopReason, ToastKind, TurnErrorCode};

/// Powiadomienie do pokazania przez powłokę.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNotice {
    /// Tytuł.
    pub title: String,
    /// Treść.
    pub body: String,
    /// Sesja (kliknięcie otwiera ją w oknie głównym).
    pub session_id: Option<String>,
}

fn notice(title: &str, body: String, session: Option<&str>) -> NativeNotice {
    NativeNotice {
        title: title.to_owned(),
        body,
        session_id: session.map(str::to_owned),
    }
}

/// Zdarzenie → powiadomienie natywne (tylko gdy okno ukryte; decyduje powłoka).
pub fn native_notice(event: &AlfaEvent) -> Option<NativeNotice> {
    match event {
        AlfaEvent::Stop {
            session_id,
            reason: StopReason::End | StopReason::MaxTokens,
            ..
        } => Some(notice(
            "Alfa",
            "Odpowiedź jest gotowa.".to_owned(),
            Some(session_id),
        )),
        AlfaEvent::Error {
            session_id, error, ..
        } if error.code != TurnErrorCode::Offline => Some(notice(
            "Alfa — błąd",
            error.message.chars().take(160).collect(),
            Some(session_id),
        )),
        AlfaEvent::ApprovalPending { session_id, .. } => Some(notice(
            "Alfa — czeka na zatwierdzenie",
            "Otwórz okno Brokera, aby zdecydować.".to_owned(),
            Some(session_id),
        )),
        AlfaEvent::Toast {
            kind: ToastKind::Error | ToastKind::Warning,
            message,
        } => Some(notice("Alfa", message.pl.clone(), None)),
        AlfaEvent::MarshalReportReady { report } => Some(notice(
            "Alfa — raport dnia",
            report.text.chars().take(200).collect(),
            None,
        )),
        _ => None,
    }
}
