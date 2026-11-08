//! Współdzielone testy kontraktowe `ReadAloud` (feature `contract-tests`) na wirtualnym pulpicie
//! i wirtualnym zegarze odtwarzania sterowanych przez [`ReadDriver`].

use personas_contract::PersonaId;

use crate::{
    ReadAloud, ReadAloudError, ReadControl, ReadPhase, ReadScope, RefuseReason, ShareConsent,
};

/// Sterowanie światem testu.
pub trait ReadDriver: Send + Sync {
    /// Okno aplikacji z dokumentem (fokus w polu z tekstem); zwraca id okna.
    fn open_document(&self, image: &str, text: &str) -> u64;
    /// Okno z fokusem w polu hasła.
    fn open_password(&self) -> u64;
    /// Zaznaczenie w oknie (UIA albo przez Ctrl+C — zależnie od świata).
    fn select(&self, window: u64, text: &str);
    /// Upływ czasu odtwarzania (ms).
    fn advance(&self, ms: u64);
}

fn ok<T>(r: Result<T, ReadAloudError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Kroki do spełnienia warunku (≤ 60 s czasu wirtualnego).
pub async fn run_until<R: ReadAloud>(
    r: &mut R,
    drv: &dyn ReadDriver,
    cond: impl Fn(&crate::ReadStatus) -> bool,
) {
    for _ in 0..3_000 {
        let s = r.step().await;
        if cond(&s) {
            return;
        }
        drv.advance(20);
    }
    panic!("warunek niespełniony: {:?}", r.status());
}

const DOC: &str = "Pierwsze zdanie. Drugie zdanie, np. z przykładem! Czy trzecie? Czwarte.";

/// Dokument: zdania po kolei, pauza/wznów, tempo, dalej/wstecz, stop czyści treść.
pub async fn reads_document_with_controls<R: ReadAloud>(r: &mut R, drv: &dyn ReadDriver) {
    drv.open_document("notepad.exe", DOC);
    let s = ok(r.start(ReadScope::Document, PersonaId::beta()).await);
    assert_eq!((s.phase, s.segments, s.index), (ReadPhase::Speaking, 4, 0));
    assert_eq!(s.app.as_deref(), Some("notepad.exe"));
    run_until(r, drv, |s| s.index == 1).await;
    assert_eq!(r.status().highlight, Some((17, 49)));
    let s = r.control(ReadControl::Pause).await;
    assert_eq!(s.phase, ReadPhase::Paused);
    drv.advance(2_000);
    assert_eq!(r.step().await.index, 1, "pauza trzyma zdanie");
    let s = r.control(ReadControl::Resume).await;
    assert_eq!((s.phase, s.index), (ReadPhase::Speaking, 1));
    let s = r.control(ReadControl::Faster).await;
    assert!((s.rate - 1.1).abs() < 1e-6);
    let s = r.control(ReadControl::Next).await;
    assert_eq!(s.index, 2);
    let s = r.control(ReadControl::Previous).await;
    assert_eq!(s.index, 1);
    run_until(r, drv, |s| s.phase == ReadPhase::Finished).await;
    assert_eq!(r.status().index, 3);
    assert_eq!(
        r.share_with_model(None).err(),
        Some(ReadAloudError::ConsentRequired)
    );
    let shared = ok(r.share_with_model(Some(&ShareConsent {
        confirmed_by_user: true,
    })));
    assert!(shared.contains("NIEZAUFANE") && shared.contains("Czwarte."));
    let s = r.control(ReadControl::Stop).await;
    assert_eq!(s.phase, ReadPhase::Idle);
    assert_eq!(
        r.share_with_model(Some(&ShareConsent {
            confirmed_by_user: true
        }))
        .err(),
        Some(ReadAloudError::NotReading),
        "stop usuwa treść z pamięci"
    );
    for e in r.take_events() {
        let payload = e.to_bus_event().payload.to_string();
        assert!(
            !payload.contains("zdanie"),
            "zdarzenia bez treści: {payload}"
        );
    }
}

/// Zaznaczenie zamiast całego dokumentu.
pub async fn reads_selection<R: ReadAloud>(r: &mut R, drv: &dyn ReadDriver) {
    let w = drv.open_document("winword.exe", DOC);
    drv.select(w, "Tylko to. I to.");
    let s = ok(r.start(ReadScope::Selection, PersonaId::alfa()).await);
    assert_eq!(s.segments, 2);
    run_until(r, drv, |s| s.phase == ReadPhase::Finished).await;
}

/// Okna chronione i pola haseł — odmowa.
pub async fn refuses_protected_and_password<R: ReadAloud>(r: &mut R, drv: &dyn ReadDriver) {
    drv.open_document("alfa-desktop.exe", "Wewnętrzne. Okno Alfy.");
    assert_eq!(
        r.start(ReadScope::Document, PersonaId::alfa()).await.err(),
        Some(ReadAloudError::Refused(RefuseReason::ProtectedTarget))
    );
    drv.open_password();
    for scope in [ReadScope::Document, ReadScope::Selection] {
        assert_eq!(
            r.start(scope, PersonaId::alfa()).await.err(),
            Some(ReadAloudError::Refused(RefuseReason::PasswordField))
        );
    }
    assert_eq!(r.status().phase, ReadPhase::Idle);
}
