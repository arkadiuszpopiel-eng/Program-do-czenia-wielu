//! Zasoby wyłączne, równoległe agentki, klasy priorytetu, wywłaszczanie w punktach atomowych
//! i voice-first (mowa użytkownika przez `scheduler-lite` na tej samej tablicy blokad).

use std::time::Duration;

use super::{Harness, Script, advance_checked, done, poll_once, state_of, succeeded, task};
use crate::{
    Holder, LeaseRequest, Priority, Resource, Scheduler, SchedulerLite, TaskClass, TaskId,
    TaskState,
};

/// Agentki pracują równolegle; zadania o wspólnym zasobie — po kolei, nigdy naraz (F5-01).
/// Zadanie wyższej rangi czekające na komplet zasobów rezerwuje go (ochrona przed zagłodzeniem).
pub async fn exclusive_resources_in_parallel<H: Harness>(h: &H) {
    let s = h.scheduler();
    let file = Resource::file("C:\\Users\\Ja\\raport.docx");
    let mut specs = Vec::new();
    for (id, res) in [
        ("ekran-1", vec![Resource::ScreenInput]),
        ("glos", vec![Resource::Speaker]),
        ("plik", vec![file.clone()]),
        ("ekran-2", vec![Resource::ScreenInput, file.clone()]),
    ] {
        h.script(&id.into(), Script::ok(3, 100));
        specs.push(task(id, TaskClass::Agent).with_resources(res));
    }
    s.submit(specs).unwrap();
    h.advance(50).await;
    // ekran-1, glos i plik startują od razu (rozłączne zasoby), ekran-2 czeka na ekran i plik.
    let running = |id: &str| matches!(state_of(h, id), TaskState::Running { .. });
    assert!(running("ekran-1") && running("glos") && running("plik"));
    assert!(!running("ekran-2"));
    advance_checked(h, 2_000, 25).await;
    for id in ["ekran-1", "ekran-2", "glos", "plik"] {
        assert!(succeeded(h, id), "{id}");
    }
    let view = s.task(&"ekran-2".into()).unwrap();
    assert!(view.finished_at_ms.unwrap() >= view.submitted_at_ms + 600);

    // Rezerwacja: „duze” (ekran + głośnik) czeka na głośnik; „male” (sam ekran, niższa ranga)
    // nie zajmie wolnego ekranu przed nim.
    h.script(&"mowi".into(), Script::ok(3, 100));
    h.script(&"duze".into(), Script::ok(1, 50));
    h.script(&"male".into(), Script::ok(1, 50));
    s.submit(vec![
        task("mowi", TaskClass::User).with_resources([Resource::Speaker]),
    ])
    .unwrap();
    h.advance(10).await;
    s.submit(vec![
        task("duze", TaskClass::User).with_resources([Resource::ScreenInput, Resource::Speaker]),
        task("male", TaskClass::Agent).with_resources([Resource::ScreenInput]),
    ])
    .unwrap();
    h.advance(10).await;
    assert_eq!(
        s.task(&"male".into()).unwrap().blocked,
        Some(crate::BlockReason::Reserved {
            resources: vec![Resource::ScreenInput]
        })
    );
    advance_checked(h, 1_000, 25).await;
    let fin = |id: &str| s.task(&id.into()).unwrap().finished_at_ms.unwrap();
    assert!(fin("mowi") <= fin("duze") && fin("duze") <= fin("male"));
    assert!(h.held().is_empty());
}

/// Klasy: przy rywalizacji o zasób użytkownik > agentki > tło; zadanie tła na zasobie
/// wywłaszczalnym oddaje go zadaniu użytkownika w najbliższym punkcie atomowym.
pub async fn priorities_and_preemption<H: Harness>(h: &H) {
    let s = h.scheduler();
    h.script(&"tlo".into(), Script::ok(5, 100));
    h.script(&"uzytkownik".into(), Script::ok(1, 10));
    s.submit(vec![
        task("tlo", TaskClass::Background).with_resources([Resource::Speaker]),
    ])
    .unwrap();
    h.advance(150).await;
    s.submit(vec![
        task("uzytkownik", TaskClass::User).with_resources([Resource::Speaker]),
    ])
    .unwrap();
    h.advance(60).await; // punkt atomowy tła w t = 200 → oddaje
    assert!(matches!(
        state_of(h, "uzytkownik"),
        TaskState::Running { .. } | TaskState::Done { .. }
    ));
    h.advance(1_000).await;
    assert!(succeeded(h, "uzytkownik") && succeeded(h, "tlo"));
    let tlo = s.task(&"tlo".into()).unwrap();
    assert_eq!(tlo.preemptions, 1);
    assert_eq!(
        tlo.steps, 5,
        "wznowienie od ukończonych kroków, bez powtórzeń"
    );
    let u = s.task(&"uzytkownik".into()).unwrap();
    assert!(u.finished_at_ms < tlo.finished_at_ms);

    // Plik nie jest wywłaszczalny: kolejka wg klasy po zwolnieniu przez posiadaczkę.
    let file = Resource::file("D:/dane.csv");
    h.script(&"trzyma".into(), Script::ok(2, 100));
    s.submit(vec![
        task("trzyma", TaskClass::Background).with_resources([file.clone()]),
    ])
    .unwrap();
    h.advance(10).await;
    let mut specs = Vec::new();
    for (id, class) in [
        ("k-tlo", TaskClass::Background),
        ("k-agentka", TaskClass::Agent),
        ("k-uzytkownik", TaskClass::User),
    ] {
        h.script(&id.into(), Script::ok(1, 50));
        specs.push(task(id, class).with_resources([file.clone()]));
    }
    s.submit(specs).unwrap();
    h.advance(1_000).await;
    let order: Vec<String> = h
        .dispatches()
        .iter()
        .map(|d| d.task.to_string())
        .filter(|t| t.starts_with("k-"))
        .collect();
    assert_eq!(order, ["k-uzytkownik", "k-agentka", "k-tlo"]);
    assert_eq!(s.task(&"trzyma".into()).unwrap().preemptions, 0);
}

/// Voice-first: mowa użytkownika (`acquire` z `UserSpeech`) dostaje głośnik w ≤ 1 kroku
/// atomowym zadania, które go trzyma; zadanie wraca po zwolnieniu i kończy od miejsca przerwania.
pub async fn voice_first<H: Harness>(h: &H) {
    let s = h.scheduler();
    h.script(&"narracja".into(), Script::ok(5, 100));
    s.submit(vec![
        task("narracja", TaskClass::Agent).with_resources([Resource::Speaker]),
    ])
    .unwrap();
    h.advance(150).await;
    let req = LeaseRequest::new(
        Resource::Speaker,
        Holder::User,
        Priority::UserSpeech,
        Duration::from_secs(5),
    );
    let mut fut = Box::pin(s.acquire(req));
    assert!(
        poll_once(&mut fut).await.is_none(),
        "zadanie trzyma głośnik"
    );
    h.advance(60).await; // punkt atomowy w t = 200
    let lease = poll_once(&mut fut)
        .await
        .expect("mowa powinna dostać głośnik po jednym kroku")
        .unwrap();
    assert_eq!(lease.holder(), &Holder::User);
    assert!(matches!(state_of(h, "narracja"), TaskState::Ready));
    h.advance(300).await;
    assert!(matches!(state_of(h, "narracja"), TaskState::Ready));
    drop(lease);
    h.advance(1_000).await;
    assert!(succeeded(h, "narracja"));
    let view = s.task(&TaskId::new("narracja")).unwrap();
    assert_eq!((view.preemptions, view.steps), (1, 5));
    assert!(matches!(
        done(h, "narracja"),
        crate::Termination::Succeeded { .. }
    ));
    assert!(s.holder(&Resource::Speaker).is_none());
}
