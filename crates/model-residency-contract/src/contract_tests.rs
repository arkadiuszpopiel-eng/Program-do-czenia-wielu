//! Współdzielone testy kontraktowe (feature `contract-tests`), uruchamiane na `-impl` i `-fake`.

use std::sync::{Arc, Mutex};

use crate::{
    Budget, Change, Device, Lease, LeaseListener, LeaseRequest, ModeSource, ModelRole, Placement,
    Priority, Residency, ResidencyError, Revocation, RevokeReason, fits,
};

/// Uchwyt testowy: świeży zarządca z budżetem i sterowany zegar.
pub trait Harness {
    /// Typ zarządcy.
    type R: Residency;
    /// Nowy zarządca z budżetem (zegar od 0 ms).
    fn residency(&self, budget: Budget) -> Self::R;
    /// Przesuwa zegar ostatnio utworzonego zarządcy.
    fn advance_ms(&self, ms: u64);
}

/// Słuchacz zapisujący powiadomienia.
#[derive(Default)]
pub struct Recorder {
    /// Odebrane dzierżawy.
    pub revoked: Mutex<Vec<Revocation>>,
    /// Przeniesione dzierżawy.
    pub moved: Mutex<Vec<Lease>>,
}

impl LeaseListener for Recorder {
    fn revoked(&self, revocation: &Revocation) {
        self.revoked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(revocation.clone());
    }
    fn moved(&self, lease: &Lease) {
        self.moved
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(lease.clone());
    }
}

struct Signals(bool, bool);
impl ModeSource for Signals {
    fn fullscreen_active(&self) -> bool {
        self.0
    }
    fn on_battery(&self) -> bool {
        self.1
    }
}

fn budget() -> Budget {
    Budget {
        vram_mb: 7_408,
        ram_mb: 8_192,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    }
}

/// Żądanie testowe.
pub fn request(owner: &str, priority: Priority, vram_mb: u32) -> LeaseRequest {
    LeaseRequest {
        owner: owner.into(),
        model: format!("{owner}-model"),
        role: ModelRole::Llm,
        priority,
        placement: Placement::GpuPreferred,
        vram_mb,
        ram_mb: 100,
        cpu_ram_mb: 1_000,
        idle_unload_ms: 1_000,
    }
}

fn ok<T>(r: Result<T, ResidencyError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Przyznanie, migawka, zwolnienie; podwójne zwolnienie = `UnknownLease`.
pub fn grant_release_snapshot<H: Harness>(h: &H) {
    let r = h.residency(budget());
    let g = ok(r.acquire(request("a", Priority::Conversation, 3_000)));
    assert_eq!(g.lease.device, Device::Gpu);
    assert_eq!(r.lease(g.lease.id), Some(g.lease.clone()));
    assert_eq!(r.snapshot().used.vram_mb, 3_000);
    ok(r.touch(g.lease.id));
    ok(r.set_in_use(g.lease.id, true));
    ok(r.release(g.lease.id));
    assert!(r.lease(g.lease.id).is_none());
    assert_eq!(
        r.release(g.lease.id),
        Err(ResidencyError::UnknownLease(g.lease.id))
    );
    assert_eq!(r.snapshot().used.vram_mb, 0);
}

/// Głos wypiera rozmowę; właściciel wypartej dzierżawy dostaje powiadomienie.
pub fn preemption_notifies_owner<H: Harness>(h: &H) {
    let r = h.residency(budget());
    let rec = Arc::new(Recorder::default());
    r.listen("llm", rec.clone());
    let low = ok(r.acquire(request("llm", Priority::Conversation, 6_000)));
    let high = ok(r.acquire(request("stt", Priority::VoiceRt, 3_000)));
    assert_eq!(high.lease.device, Device::Gpu);
    assert_eq!(high.evicted.len(), 1);
    let revoked = rec
        .revoked
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].lease.id, low.lease.id);
    assert_eq!(
        revoked[0].reason,
        RevokeReason::Preempted { by: high.lease.id }
    );
    // Tło nigdy nie wypiera głosu — dostaje CPU.
    let bg = ok(r.acquire(request("bg", Priority::Background, 6_000)));
    assert_eq!(bg.lease.device, Device::Cpu);
    assert!(bg.evicted.is_empty());
}

/// Sygnał pełnego ekranu przenosi modele z GPU na CPU i powiadamia właściciela.
pub fn fullscreen_moves_models_to_cpu<H: Harness>(h: &H) {
    let r = h.residency(budget());
    let rec = Arc::new(Recorder::default());
    r.listen("stt", rec.clone());
    let g = ok(r.acquire(request("stt", Priority::VoiceRt, 1_500)));
    let changes = r.refresh_mode(&Signals(true, false));
    assert!(matches!(&changes[..], [Change::Moved(l)] if l.id == g.lease.id));
    assert!(r.snapshot().mode.gaming);
    assert_eq!(r.snapshot().used.vram_mb, 0);
    assert_eq!(rec.moved.lock().unwrap_or_else(|p| p.into_inner()).len(), 1);
    assert!(
        r.refresh_mode(&Signals(true, false)).is_empty(),
        "bez zmiany"
    );
    let mut gpu_only = request("x", Priority::VoiceRt, 100);
    gpu_only.placement = Placement::GpuOnly;
    assert_eq!(r.acquire(gpu_only), Err(ResidencyError::Gaming));
    r.refresh_mode(&Signals(false, false));
    assert!(!r.snapshot().mode.gaming);
}

/// Bezczynność liczona zegarem zarządcy.
pub fn idle_reaping_follows_clock<H: Harness>(h: &H) {
    let r = h.residency(budget());
    let g = ok(r.acquire(request("a", Priority::Conversation, 1_000)));
    h.advance_ms(999);
    assert!(r.reap_idle().is_empty());
    ok(r.touch(g.lease.id));
    h.advance_ms(999);
    assert!(r.reap_idle().is_empty(), "touch odświeża licznik");
    h.advance_ms(1);
    let reaped = r.reap_idle();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].reason, RevokeReason::Idle);
}

/// Deterministyczna seria 300 operacji: budżet nigdy nieprzekroczony, `Wait` tylko na wyższy/równy.
pub fn never_exceeds_budget<H: Harness>(h: &H) {
    let r = h.residency(budget());
    let mut seed: u64 = 0x5eed_1234;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        seed >> 33
    };
    let prios = [
        Priority::Background,
        Priority::Conversation,
        Priority::VoiceRt,
    ];
    let mut held = Vec::new();
    for _ in 0..300 {
        match next() % 4 {
            0 | 1 => {
                let p = prios[(next() % 3) as usize];
                let vram = u32::try_from(next() % 5_000).unwrap_or(0);
                match r.acquire(request("p", p, vram)) {
                    Ok(g) => held.push(g.lease.id),
                    Err(ResidencyError::Wait { blockers }) => {
                        for b in blockers {
                            if let Some(l) = r.lease(b) {
                                assert!(l.request.priority >= p, "czeka na niższy priorytet");
                            }
                        }
                    }
                    Err(e) => panic!("nieoczekiwany błąd {e}"),
                }
            }
            2 if !held.is_empty() => {
                let id = held.swap_remove((next() as usize) % held.len());
                let _ = r.release(id);
            }
            _ => h.advance_ms(next() % 700),
        }
        let s = r.snapshot();
        assert!(fits(s.used, s.budget), "{:?} > {:?}", s.used, s.budget);
    }
}

/// Uruchamia cały zestaw.
pub fn run_all<H: Harness>(h: &H) {
    grant_release_snapshot(h);
    preemption_notifies_owner(h);
    fullscreen_moves_models_to_cpu(h);
    idle_reaping_follows_clock(h);
    never_exceeds_budget(h);
}
