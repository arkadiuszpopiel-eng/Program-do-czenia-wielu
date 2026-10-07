//! Testy własności (ACC-F2-model-residency-01): dowolna sekwencja operacji → suma dzierżaw
//! ≤ budżet w każdym momencie; żądanie o wyższym priorytecie nigdy nie czeka na niższy;
//! `TooLarge` tylko, gdy model nie mieści się w pustej maszynie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use model_residency_contract::{
    Budget, Device, LeaseId, LeaseRequest, LeaseTable, Mode, ModelRole, Placement, Priority,
    ResidencyError, RevokeReason, fits,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Acquire(LeaseRequest),
    Release(usize),
    InUse(usize, bool),
    Touch(usize),
    Advance(u64),
    Reap,
    Mode(bool, bool, Option<u32>),
    Budget(u32, u32),
}

fn priority() -> impl Strategy<Value = Priority> {
    prop_oneof![
        Just(Priority::Background),
        Just(Priority::Conversation),
        Just(Priority::VoiceRt)
    ]
}

fn placement() -> impl Strategy<Value = Placement> {
    prop_oneof![
        Just(Placement::GpuOnly),
        Just(Placement::GpuPreferred),
        Just(Placement::GpuIfFree),
        Just(Placement::CpuOnly)
    ]
}

fn role() -> impl Strategy<Value = ModelRole> {
    prop_oneof![
        Just(ModelRole::Stt),
        Just(ModelRole::Tts),
        Just(ModelRole::Llm),
        Just(ModelRole::Embedder),
        Just(ModelRole::Vad)
    ]
}

fn request() -> impl Strategy<Value = LeaseRequest> {
    (
        role(),
        priority(),
        placement(),
        0u32..9_000,
        0u32..800,
        0u32..9_000,
        0u64..5_000,
    )
        .prop_map(
            |(role, priority, placement, vram, ram, cpu_ram, idle)| LeaseRequest {
                owner: "p".into(),
                model: format!("{role:?}"),
                role,
                priority,
                placement,
                vram_mb: vram,
                ram_mb: ram,
                cpu_ram_mb: cpu_ram,
                idle_unload_ms: idle,
            },
        )
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => request().prop_map(Op::Acquire),
        2 => any::<usize>().prop_map(Op::Release),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(i, b)| Op::InUse(i, b)),
        1 => any::<usize>().prop_map(Op::Touch),
        1 => (0u64..3_000).prop_map(Op::Advance),
        1 => Just(Op::Reap),
        1 => (any::<bool>(), any::<bool>(), proptest::option::of(2_000u32..8_000))
            .prop_map(|(g, b, e)| Op::Mode(g, b, e)),
        1 => (1_000u32..16_000, 1_000u32..16_000).prop_map(|(v, r)| Op::Budget(v, r)),
    ]
}

fn budget(vram_mb: u32, ram_mb: u32, exclusive: bool) -> Budget {
    Budget {
        vram_mb,
        ram_mb,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: exclusive,
    }
}

fn pick(ids: &[LeaseId], i: usize) -> Option<LeaseId> {
    (!ids.is_empty()).then(|| ids[i % ids.len()])
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1_000, ..ProptestConfig::default() })]

    #[test]
    fn invariants_hold_for_any_sequence(
        exclusive in any::<bool>(),
        ops in proptest::collection::vec(op(), 1..60),
    ) {
        let mut t = LeaseTable::new(budget(7_408, 8_192, exclusive));
        let mut now = 0u64;
        for op in ops {
            let ids: Vec<LeaseId> = t.snapshot().leases.iter().map(|l| l.id).collect();
            match op {
                Op::Acquire(req) => {
                    let before = t.clone();
                    match t.acquire(&req, now) {
                        Ok(g) => {
                            prop_assert_eq!(t.lease(g.lease.id), Some(&g.lease));
                            if g.lease.device == Device::Gpu {
                                prop_assert!(req.placement != Placement::CpuOnly);
                                prop_assert!(!t.mode().gaming);
                            }
                            for r in &g.evicted {
                                let p = r.lease.request.priority;
                                prop_assert!(p < req.priority
                                    || (p == req.priority && !r.lease.in_use),
                                    "wyparto dzierżawę, która nie ustępuje: {:?}", r);
                            }
                            let preempted = g.evicted.iter()
                                .any(|r| matches!(r.reason, RevokeReason::Preempted { .. }));
                            let gpu = g.lease.device == Device::Gpu;
                            if req.placement == Placement::GpuIfFree && gpu && preempted {
                                // Wypieranie z GPU tylko wtedy, gdy CPU się nie da.
                                let mut cpu = req.clone();
                                cpu.placement = Placement::CpuOnly;
                                prop_assert!(before.clone().acquire(&cpu, now).is_err());
                            }
                        }
                        Err(ResidencyError::Wait { blockers }) => {
                            prop_assert_eq!(&t, &before, "odmowa nie zmienia stanu");
                            for b in blockers {
                                let l = t.lease(b).expect("bloker istnieje");
                                prop_assert!(l.request.priority > req.priority
                                    || (l.request.priority == req.priority && l.in_use),
                                    "żądanie czeka na niższy priorytet: {:?}", l);
                            }
                        }
                        Err(ResidencyError::TooLarge { .. }) => {
                            let b = t.budget();
                            let gpu = req.vram_mb <= b.vram_mb && req.ram_mb <= b.ram_mb;
                            let cpu = req.cpu_ram_mb <= b.ram_mb;
                            let gpu_allowed = req.placement != Placement::CpuOnly && !t.mode().gaming;
                            let cpu_allowed = req.placement != Placement::GpuOnly;
                            prop_assert!(!((gpu && gpu_allowed) || (cpu && cpu_allowed)));
                        }
                        Err(ResidencyError::Gaming) => prop_assert!(t.mode().gaming),
                        Err(ResidencyError::Battery) => prop_assert!(t.mode().battery),
                        Err(e) => prop_assert!(false, "nieoczekiwany błąd {e}"),
                    }
                }
                Op::Release(i) => if let Some(id) = pick(&ids, i) { t.release(id).unwrap(); },
                Op::InUse(i, b) => if let Some(id) = pick(&ids, i) { t.set_in_use(id, b, now).unwrap(); },
                Op::Touch(i) => if let Some(id) = pick(&ids, i) { t.touch(id, now).unwrap(); },
                Op::Advance(ms) => now += ms,
                Op::Reap => {
                    for r in t.reap_idle(now) {
                        prop_assert!(!r.lease.in_use);
                    }
                }
                Op::Mode(gaming, battery, emu) => {
                    t.set_mode(Mode {
                        gaming,
                        battery,
                        emulated: emu.map(|v| budget(v, 6_000, true)),
                    });
                    if gaming {
                        prop_assert_eq!(t.used().vram_mb, 0, "gra zwalnia GPU");
                    }
                    if battery {
                        prop_assert!(t.snapshot().leases.iter()
                            .all(|l| l.request.priority != Priority::Background));
                    }
                }
                Op::Budget(v, r) => { t.set_budget(budget(v, r, exclusive)); }
            }
            prop_assert!(fits(t.used(), t.budget()), "{:?} > {:?}", t.used(), t.budget());
            let b = t.budget();
            if b.stt_tts_exclusive {
                let gpu_roles: Vec<ModelRole> = t.snapshot().leases.iter()
                    .filter(|l| l.device == Device::Gpu).map(|l| l.request.role).collect();
                prop_assert!(!(gpu_roles.contains(&ModelRole::Stt) && gpu_roles.contains(&ModelRole::Tts)),
                    "STT i TTS naraz na GPU przy wykluczeniu");
            }
        }
    }
}
