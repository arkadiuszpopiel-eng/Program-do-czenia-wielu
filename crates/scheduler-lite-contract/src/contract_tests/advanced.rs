//! Przypadki zaawansowane: voice-first, przekazanie, zakleszczenia, kill-switch.

use crate::contract_tests::{Harness, ok, persona, poll_once, req};
use crate::{Holder, LeaseSignal, PreemptReason, Priority, Resource, SchedError, SchedulerLite};

/// Voice-first: mowa użytkownika natychmiast (0 ms) prosi narrację o zwolnienie, bez zabijania.
pub async fn user_speech_preempts_narration<H: Harness>(h: &H) {
    let s = h.scheduler();
    let narration = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Narration,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut user = Box::pin(s.acquire(req(
        Resource::Speaker,
        Holder::User,
        Priority::UserSpeech,
        1000,
    )));
    assert!(poll_once(&mut user).await.is_none());
    assert_eq!(
        narration.signal(),
        LeaseSignal::PreemptRequested {
            by: Holder::User,
            reason: PreemptReason::UserSpeaks
        }
    );
    assert_eq!(
        s.holder(&Resource::Speaker).map(|l| l.holder),
        Some(persona("alfa")),
        "nie zabita"
    );
    drop(narration); // punkt atomowy: posiadaczka sama zwalnia
    let user = ok(poll_once(&mut user).await);
    assert_eq!(user.holder(), &Holder::User);
    let mic = s
        .acquire(req(
            Resource::Mic,
            Holder::System("voice-audio".into()),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        s.preempt(&Resource::Mic, Holder::User, PreemptReason::UserSpeaks),
        Err(SchedError::NotPreemptible(Resource::Mic))
    );
    drop(mic);
}

/// Przekazanie bez luki: adresatka dostaje głos przed innymi czekającymi.
pub async fn handoff_without_gap<H: Harness>(h: &H) {
    let s = h.scheduler();
    let mut alfa = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut beta = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("beta"),
        Priority::Normal,
        5000,
    )));
    let mut delta = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("delta"),
        Priority::Normal,
        5000,
    )));
    assert!(poll_once(&mut beta).await.is_none() && poll_once(&mut delta).await.is_none());
    alfa.handoff(persona("delta"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut delta = ok(poll_once(&mut delta).await);
    assert!(poll_once(&mut beta).await.is_none());
    drop(alfa); // nieaktywna po przekazaniu — nic nie zwalnia
    assert_eq!(
        s.holder(&Resource::Speaker).map(|l| l.holder),
        Some(persona("delta"))
    );
    // Przekazanie do agentki, która jeszcze nie prosi: rezerwacja blokuje czekającą Betę.
    delta
        .handoff(persona("gama"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(poll_once(&mut beta).await.is_none());
    let gama = s
        .acquire(req(Resource::Speaker, persona("gama"), Priority::Normal, 0))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    drop(gama);
    drop(ok(poll_once(&mut beta).await));
    assert_eq!(
        s.handoff(&Resource::Speaker, &persona("alfa"), persona("beta")),
        Err(SchedError::NotHeld(Resource::Speaker))
    );
}

/// Zakleszczenie: najmłodsze żądanie w cyklu dostaje błąd, reszta działa dalej.
pub async fn deadlock_fails_youngest<H: Harness>(h: &H) {
    let s = h.scheduler();
    let alfa_speaker = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let delta_screen = s
        .acquire(req(
            Resource::ScreenInput,
            persona("delta"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut alfa_screen = Box::pin(s.acquire(req(
        Resource::ScreenInput,
        persona("alfa"),
        Priority::Normal,
        5000,
    )));
    assert!(poll_once(&mut alfa_screen).await.is_none());
    let delta_speaker = s
        .acquire(req(
            Resource::Speaker,
            persona("delta"),
            Priority::Normal,
            5000,
        ))
        .await;
    assert!(matches!(delta_speaker, Err(SchedError::Deadlock { .. })));
    drop(delta_screen);
    drop(ok(poll_once(&mut alfa_screen).await));
    drop(alfa_speaker);
}

/// Kill-switch: odebranie i anulowanie; porzucone `acquire` samo znika z kolejki.
pub async fn kill_switch_and_cancel<H: Harness>(h: &H) {
    let s = h.scheduler();
    let held = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    {
        let mut dropped = Box::pin(s.acquire(req(
            Resource::Speaker,
            persona("gama"),
            Priority::Normal,
            5000,
        )));
        assert!(poll_once(&mut dropped).await.is_none());
        assert_eq!(s.queue(&Resource::Speaker).len(), 1);
    }
    assert!(
        s.queue(&Resource::Speaker).is_empty(),
        "porzucone żądanie anulowane"
    );
    let mut waiting = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("beta"),
        Priority::Normal,
        5000,
    )));
    assert!(poll_once(&mut waiting).await.is_none());
    assert_eq!(s.kill_all(), 2);
    assert!(held.is_revoked());
    assert!(matches!(
        poll_once(&mut waiting).await,
        Some(Err(SchedError::Cancelled))
    ));
    assert_eq!(s.holder(&Resource::Speaker), None);
    let system = s
        .acquire(req(
            Resource::Speaker,
            Holder::System("router".into()),
            Priority::Normal,
            10,
        ))
        .await;
    assert!(matches!(system, Err(SchedError::SystemCannotSpeak)));
    let file = s
        .acquire(req(
            Resource::file("C:\\A.txt"),
            persona("delta"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let again = s
        .acquire(req(
            Resource::file("c:/a.txt"),
            persona("delta"),
            Priority::Normal,
            10,
        ))
        .await;
    assert!(matches!(again, Err(SchedError::AlreadyHeld { .. })));
    drop(file);
}
