//! Testy własności osobnego budżetu pamięci na kartę (C6.6): dowolna sekwencja operacji na
//! maszynie z trzema kartami (główna, zintegrowana ze wspólną pamięcią, druga osobna) → każda pula
//! mieści swoje dzierżawy, wykluczenie STT/TTS na każdej karcie, dzierżawa trafia na kartę
//! z żądania (nieznana → główna), żądanie na karcie nie wypiera dzierżaw z innych kart, gdy RAM
//! wystarcza; żądania bez karty dają **dokładnie** to samo co tablica z jedną kartą.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use model_residency_contract::{
    Budget, Device, GpuId, GpuLayout, GpuPool, LeaseId, LeaseRequest, LeaseTable, Mode, ModelRole,
    Placement, Priority, ResidencyError, RevokeReason,
};
use proptest::prelude::*;

const PRIMARY: GpuId = GpuId(1);

#[derive(Debug, Clone)]
enum Op {
    Acquire(LeaseRequest),
    Release(usize),
    InUse(usize, bool),
    Advance(u64),
    Reap,
    Mode(bool, bool, Option<u32>),
    Budget(u32, u32),
    DropSecond(bool),
}

fn request(gpus: Vec<Option<GpuId>>) -> impl Strategy<Value = LeaseRequest> {
    let role = prop_oneof![
        Just(ModelRole::Stt),
        Just(ModelRole::Tts),
        Just(ModelRole::Llm),
        Just(ModelRole::Embedder)
    ];
    let priority = prop_oneof![
        Just(Priority::Background),
        Just(Priority::Conversation),
        Just(Priority::VoiceRt)
    ];
    let placement = prop_oneof![
        Just(Placement::GpuOnly),
        Just(Placement::GpuPreferred),
        Just(Placement::GpuIfFree),
        Just(Placement::CpuOnly)
    ];
    (
        (role, priority, placement),
        (0u32..8_000, 0u32..800, 0u32..9_000, 0u64..5_000),
        proptest::sample::select(gpus),
    )
        .prop_map(
            |((role, priority, placement), (vram, ram, cpu_ram, idle), gpu)| LeaseRequest {
                owner: "p".into(),
                model: format!("{role:?}"),
                role,
                priority,
                placement,
                vram_mb: vram,
                ram_mb: ram,
                cpu_ram_mb: cpu_ram,
                idle_unload_ms: idle,
                gpu,
            },
        )
}

/// Operacje; `DropSecond` (zniknięcie/powrót drugiej osobnej karty) test równoważności pomija.
fn op(gpus: Vec<Option<GpuId>>) -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => request(gpus).prop_map(Op::Acquire),
        2 => any::<usize>().prop_map(Op::Release),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(i, b)| Op::InUse(i, b)),
        1 => (0u64..3_000).prop_map(Op::Advance),
        1 => Just(Op::Reap),
        1 => (any::<bool>(), any::<bool>(), proptest::option::of(2_000u32..8_000))
            .prop_map(|(g, b, e)| Op::Mode(g, b, e)),
        1 => (1_000u32..16_000, 1_000u32..16_000).prop_map(|(v, r)| Op::Budget(v, r)),
        1 => any::<bool>().prop_map(Op::DropSecond),
    ]
}

fn budget(vram_mb: u32, ram_mb: u32) -> Budget {
    Budget {
        vram_mb,
        ram_mb,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: true,
    }
}

/// Główna RTX 4050 (adapter 1), zintegrowana Intel UHD (adapter 0, pamięć z RAM), druga osobna 8 GB.
fn layout(second: bool, exclusive: bool) -> GpuLayout {
    let mut others = vec![GpuPool {
        gpu: GpuId(0),
        vram_mb: 4_096,
        desktop_reserve_mb: 0,
        stt_tts_exclusive: true,
        shared: true,
    }];
    if second {
        others.push(GpuPool {
            gpu: GpuId(2),
            vram_mb: 7_408,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: exclusive,
            shared: false,
        });
    }
    GpuLayout {
        primary: PRIMARY,
        others,
    }
}

fn pick(ids: &[LeaseId], i: usize) -> Option<LeaseId> {
    (!ids.is_empty()).then(|| ids[i % ids.len()])
}

fn check_pools(t: &LeaseTable) -> Result<(), TestCaseError> {
    let pools = t.pools();
    prop_assert!(t.within_budget(), "{:?}", t.snapshot().gpus);
    for l in t.snapshot().leases {
        prop_assert_eq!(l.gpu.is_some(), l.device == Device::Gpu, "{:?}", l);
        if let Some(g) = l.gpu {
            prop_assert!(
                pools.card(g).is_some(),
                "dzierżawa na karcie bez puli: {:?}",
                l
            );
        }
    }
    for card in pools.cards().filter(|p| p.stt_tts_exclusive) {
        let roles: Vec<ModelRole> = t
            .snapshot()
            .leases
            .iter()
            .filter(|l| l.gpu == Some(card.gpu))
            .map(|l| l.request.role)
            .collect();
        prop_assert!(
            !(roles.contains(&ModelRole::Stt) && roles.contains(&ModelRole::Tts)),
            "STT i TTS naraz na karcie {}",
            card.gpu
        );
    }
    if t.mode().gaming {
        prop_assert!(t.snapshot().leases.iter().all(|l| l.device == Device::Cpu));
    }
    Ok(())
}

fn check_acquire(
    before: &LeaseTable,
    t: &LeaseTable,
    req: &LeaseRequest,
    result: &Result<model_residency_contract::Grant, ResidencyError>,
) -> Result<(), TestCaseError> {
    let pools = before.pools();
    let card = pools.resolve(req.gpu);
    let leases = before.snapshot().leases;
    match result {
        Ok(g) => {
            if g.lease.device == Device::Gpu {
                prop_assert_eq!(g.lease.gpu, Some(card), "karta z żądania albo główna");
            }
            let on = (g.lease.device == Device::Gpu).then_some(card);
            let ram_used = pools.used(leases.iter(), None).ram_mb;
            let ram_ok = ram_used.saturating_add(pools.need(req, on).ram_mb) <= pools.ram_mb;
            for r in &g.evicted {
                let p = r.lease.request.priority;
                prop_assert!(p < req.priority || (p == req.priority && !r.lease.in_use));
                let victim_card = pools.card_of(&r.lease);
                if ram_ok {
                    prop_assert_eq!(victim_card, on, "wyparto dzierżawę z innej karty: {:?}", r);
                } else if victim_card != on {
                    prop_assert!(pools.load(&r.lease, None).ram_mb > 0, "{:?}", r);
                }
                if matches!(r.reason, RevokeReason::Exclusive { .. }) {
                    prop_assert_eq!(victim_card, on);
                }
            }
        }
        Err(ResidencyError::Wait { blockers }) => {
            prop_assert_eq!(t, before, "odmowa nie zmienia stanu");
            for b in blockers {
                let l = t.lease(*b).expect("bloker istnieje");
                prop_assert!(
                    l.request.priority > req.priority
                        || (l.request.priority == req.priority && l.in_use)
                );
            }
        }
        Err(ResidencyError::TooLarge {
            vram_mb, ram_mb, ..
        }) => {
            prop_assert_eq!(
                (*vram_mb, *ram_mb),
                (pools.limit(Some(card)).vram_mb, pools.ram_mb)
            );
            let gpu = pools.need(req, Some(card)).within(pools.limit(Some(card)));
            let cpu = pools.need(req, None).within(pools.limit(None));
            let gpu_allowed = req.placement != Placement::CpuOnly && !before.mode().gaming;
            let cpu_allowed = req.placement != Placement::GpuOnly;
            prop_assert!(!((gpu && gpu_allowed) || (cpu && cpu_allowed)));
        }
        Err(ResidencyError::Gaming) => prop_assert!(before.mode().gaming),
        Err(ResidencyError::Battery) => prop_assert!(before.mode().battery),
        Err(e) => prop_assert!(false, "nieoczekiwany błąd {e}"),
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 500, ..ProptestConfig::default() })]

    #[test]
    fn every_card_keeps_its_own_budget(
        exclusive in any::<bool>(),
        ops in proptest::collection::vec(
            op(vec![None, Some(GpuId(0)), Some(PRIMARY), Some(GpuId(2)), Some(GpuId(7))]),
            1..60,
        ),
    ) {
        let mut t = LeaseTable::with_gpus(budget(5_373, 8_192), layout(true, exclusive));
        let mut now = 0u64;
        for op in ops {
            let ids: Vec<LeaseId> = t.snapshot().leases.iter().map(|l| l.id).collect();
            match op {
                Op::Acquire(req) => {
                    let before = t.clone();
                    let result = t.acquire(&req, now);
                    check_acquire(&before, &t, &req, &result)?;
                }
                Op::Release(i) => if let Some(id) = pick(&ids, i) { t.release(id).unwrap(); },
                Op::InUse(i, b) => if let Some(id) = pick(&ids, i) { t.set_in_use(id, b, now).unwrap(); },
                Op::Advance(ms) => now += ms,
                Op::Reap => { t.reap_idle(now); }
                Op::Mode(gaming, battery, emu) => {
                    t.set_mode(Mode { gaming, battery, emulated: emu.map(|v| budget(v, 6_000)) });
                }
                Op::Budget(v, r) => { t.set_budget(budget(v, r)); }
                Op::DropSecond(drop) => {
                    t.set_gpus(layout(!drop, exclusive));
                    if drop {
                        prop_assert_eq!(t.used_on(GpuId(2)), 0);
                    }
                }
            }
            check_pools(&t)?;
        }
    }

    /// Maszyna z jedną kartą działa jak dotąd: żądania bez karty (albo z kartą nieznaną) na
    /// tablicy z dodatkowymi kartami dają te same wyniki, zmiany i migawki co na tablicy z jedną.
    #[test]
    fn requests_without_card_behave_like_single_card(
        exclusive in any::<bool>(),
        ops in proptest::collection::vec(op(vec![None, Some(GpuId(7))]), 1..60),
    ) {
        let b = budget(5_373, 8_192);
        let mut single = LeaseTable::with_gpus(b, GpuLayout::single(PRIMARY));
        let mut multi = LeaseTable::with_gpus(b, layout(true, exclusive));
        let mut now = 0u64;
        for op in ops {
            let ids: Vec<LeaseId> = single.snapshot().leases.iter().map(|l| l.id).collect();
            match op {
                Op::Acquire(req) => prop_assert_eq!(single.acquire(&req, now), multi.acquire(&req, now)),
                Op::Release(i) => if let Some(id) = pick(&ids, i) {
                    prop_assert_eq!(single.release(id), multi.release(id));
                },
                Op::InUse(i, v) => if let Some(id) = pick(&ids, i) {
                    prop_assert_eq!(single.set_in_use(id, v, now), multi.set_in_use(id, v, now));
                },
                Op::Advance(ms) => now += ms,
                Op::Reap => prop_assert_eq!(single.reap_idle(now), multi.reap_idle(now)),
                Op::Mode(gaming, battery, emu) => {
                    let mode = Mode { gaming, battery, emulated: emu.map(|v| budget(v, 6_000)) };
                    prop_assert_eq!(single.set_mode(mode), multi.set_mode(mode));
                }
                Op::Budget(v, r) => prop_assert_eq!(single.set_budget(budget(v, r)), multi.set_budget(budget(v, r))),
                Op::DropSecond(_) => {}
            }
            let (s, m) = (single.snapshot(), multi.snapshot());
            prop_assert_eq!((&s.budget, &s.used, &s.mode, &s.leases), (&m.budget, &m.used, &m.mode, &m.leases));
        }
    }
}
