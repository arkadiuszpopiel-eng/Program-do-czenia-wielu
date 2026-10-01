//! Testy tabelaryczne maszyny stanów (scenariusze z PLAN §3.4/§3.5).

use super::*;

pub(crate) fn req(
    model: &str,
    role: ModelRole,
    priority: Priority,
    placement: Placement,
    vram_mb: u32,
    cpu_ram_mb: u32,
) -> LeaseRequest {
    LeaseRequest {
        owner: "test".into(),
        model: model.into(),
        role,
        priority,
        placement,
        vram_mb,
        ram_mb: 200,
        cpu_ram_mb,
        idle_unload_ms: 600_000,
    }
}

/// Baseline: 8 GB VRAM − 768 MB rezerwy pulpitu, połowa z 16 GB RAM.
fn baseline() -> Budget {
    Budget {
        vram_mb: 8_176 - 768,
        ram_mb: 8_192,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    }
}

fn stt() -> LeaseRequest {
    req(
        "whisper-turbo-q5",
        ModelRole::Stt,
        Priority::VoiceRt,
        Placement::GpuPreferred,
        1_500,
        1_200,
    )
}

fn llm(vram: u32) -> LeaseRequest {
    req(
        "bielik-4.5b-q4_k_m",
        ModelRole::Llm,
        Priority::Conversation,
        Placement::GpuPreferred,
        vram,
        3_000,
    )
}

#[test]
fn baseline_fits_stt_and_small_llm_on_gpu() {
    let mut t = LeaseTable::new(baseline());
    let a = t.acquire(&stt(), 0).unwrap();
    let b = t.acquire(&llm(3_500), 1).unwrap();
    assert_eq!(a.lease.device, Device::Gpu);
    assert_eq!(b.lease.device, Device::Gpu);
    assert!(b.evicted.is_empty());
    assert_eq!(t.used().vram_mb, 5_000);
    assert!(t.within_budget());
}

#[test]
fn eight_b_llm_needs_stt_on_cpu() {
    // 8B Q4 ≈ 5,5 GB: nie mieści się obok STT na GPU, rozmowa nie wypiera głosu → CPU.
    let mut t = LeaseTable::new(baseline());
    t.acquire(&stt(), 0).unwrap();
    let big = t.acquire(&llm(6_000), 1).unwrap();
    assert_eq!(
        big.lease.device,
        Device::Cpu,
        "LLM nie wypiera STT (głos > rozmowa)"
    );
    assert!(big.evicted.is_empty());
}

#[test]
fn voice_preempts_conversation_lru() {
    let mut t = LeaseTable::new(baseline());
    let l1 = t.acquire(&llm(4_000), 0).unwrap().lease.id;
    let l2 = t.acquire(&llm(3_000), 5).unwrap().lease.id;
    t.touch(l1, 10).unwrap(); // l2 jest teraz najdawniej używana
    let g = t.acquire(&stt(), 20).unwrap();
    assert_eq!(g.lease.device, Device::Gpu);
    assert_eq!(g.evicted.len(), 1);
    assert_eq!(g.evicted[0].lease.id, l2);
    assert_eq!(
        g.evicted[0].reason,
        RevokeReason::Preempted { by: g.lease.id }
    );
    assert!(t.lease(l1).is_some() && t.lease(l2).is_none());
}

#[test]
fn background_waits_only_for_higher_priority() {
    let budget = Budget {
        vram_mb: 4_000,
        ram_mb: 1_000,
        ..baseline()
    };
    let mut t = LeaseTable::new(budget);
    let voice = t.acquire(&stt(), 0).unwrap().lease.id;
    let conv = t.acquire(&llm(2_000), 0).unwrap().lease.id;
    let mut bg = req(
        "embedder",
        ModelRole::Embedder,
        Priority::Background,
        Placement::GpuOnly,
        1_000,
        900,
    );
    match t.acquire(&bg, 1) {
        Err(ResidencyError::Wait { blockers }) => {
            assert!(blockers.contains(&voice) && blockers.contains(&conv));
        }
        other => panic!("{other:?}"),
    }
    bg.priority = Priority::VoiceRt;
    let g = t.acquire(&bg, 2).unwrap();
    assert_eq!(g.evicted.len(), 1, "wyparta rozmowa, głos w spokoju");
    assert_eq!(g.evicted[0].lease.id, conv);
}

#[test]
fn equal_priority_in_use_blocks_idle_does_not() {
    let budget = Budget {
        vram_mb: 2_000,
        ram_mb: 1_000,
        ..baseline()
    };
    let mut t = LeaseTable::new(budget);
    let first = t.acquire(&stt(), 0).unwrap().lease.id;
    t.set_in_use(first, true, 1).unwrap();
    let mut second = stt();
    second.model = "stt-2".into();
    second.placement = Placement::GpuOnly;
    assert_eq!(
        t.acquire(&second, 2),
        Err(ResidencyError::Wait {
            blockers: vec![first]
        })
    );
    t.set_in_use(first, false, 3).unwrap();
    let g = t.acquire(&second, 4).unwrap();
    assert_eq!(g.evicted[0].lease.id, first);
}

#[test]
fn stt_tts_exclusive_on_small_gpu() {
    let budget = Budget {
        vram_mb: 6_000 - 768,
        ram_mb: 8_000,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: true,
    };
    let mut t = LeaseTable::new(budget);
    let s = t.acquire(&stt(), 0).unwrap().lease.id;
    let tts = req(
        "chatterbox",
        ModelRole::Tts,
        Priority::VoiceRt,
        Placement::GpuPreferred,
        2_000,
        2_500,
    );
    let g = t.acquire(&tts, 1).unwrap();
    assert_eq!(g.lease.device, Device::Gpu);
    assert_eq!(
        g.evicted[0].reason,
        RevokeReason::Exclusive { by: g.lease.id }
    );
    assert!(t.lease(s).is_none());
    // STT w trakcie tury blokuje ciężki TTS na GPU → TTS idzie na CPU.
    let mut t = LeaseTable::new(budget);
    let s = t.acquire(&stt(), 0).unwrap().lease.id;
    t.set_in_use(s, true, 0).unwrap();
    let g = t.acquire(&tts, 1).unwrap();
    assert_eq!(g.lease.device, Device::Cpu);
    assert!(g.evicted.is_empty());
}

#[test]
fn gaming_moves_to_cpu_or_evicts_and_denies_gpu_only() {
    let mut t = LeaseTable::new(baseline());
    let s = t.acquire(&stt(), 0).unwrap().lease.id;
    let mut gpu_only = llm(2_000);
    gpu_only.placement = Placement::GpuOnly;
    let g = t.acquire(&gpu_only, 0).unwrap().lease.id;
    let changes = t.set_mode(Mode {
        gaming: true,
        ..Mode::normal()
    });
    assert_eq!(changes.len(), 2);
    assert!(matches!(&changes[0], Change::Moved(l) if l.id == s && l.device == Device::Cpu));
    assert!(matches!(&changes[1],
        Change::Revoked(r) if r.lease.id == g && r.reason == RevokeReason::Gaming));
    assert_eq!(t.used().vram_mb, 0);
    assert_eq!(t.acquire(&gpu_only, 1), Err(ResidencyError::Gaming));
    let cpu = t.acquire(&llm(2_000), 1).unwrap();
    assert_eq!(cpu.lease.device, Device::Cpu, "w trybie gry tylko CPU");
}

#[test]
fn battery_evicts_and_denies_background() {
    let mut t = LeaseTable::new(baseline());
    let bg = req(
        "embedder",
        ModelRole::Embedder,
        Priority::Background,
        Placement::CpuOnly,
        0,
        300,
    );
    let id = t.acquire(&bg, 0).unwrap().lease.id;
    let changes = t.set_mode(Mode {
        battery: true,
        ..Mode::normal()
    });
    assert!(matches!(&changes[..],
        [Change::Revoked(r)] if r.lease.id == id && r.reason == RevokeReason::Battery));
    assert_eq!(t.acquire(&bg, 1), Err(ResidencyError::Battery));
    assert!(t.acquire(&stt(), 1).is_ok(), "głos działa na baterii");
}

#[test]
fn emulation_and_budget_shrink_evict_lowest_first() {
    let desktop = Budget {
        vram_mb: 16_000 - 768,
        ram_mb: 16_000,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    };
    let mut t = LeaseTable::new(desktop);
    let voice = t.acquire(&stt(), 0).unwrap().lease.id;
    let big = t.acquire(&llm(9_000), 0).unwrap().lease.id;
    let changes = t.set_mode(Mode {
        emulated: Some(baseline()),
        ..Mode::normal()
    });
    assert!(matches!(&changes[..],
        [Change::Revoked(r)] if r.lease.id == big && r.reason == RevokeReason::BudgetShrunk));
    assert!(t.lease(voice).is_some());
    assert_eq!(t.budget(), desktop.min(baseline()));
    let changes = t.set_budget(Budget {
        vram_mb: 100,
        ..desktop
    });
    assert_eq!(changes.len(), 1);
    assert!(t.within_budget());
}

#[test]
fn too_large_invalid_unknown_and_idle() {
    let mut t = LeaseTable::new(baseline());
    let mut huge = llm(50_000);
    huge.cpu_ram_mb = 50_000;
    assert!(matches!(
        t.acquire(&huge, 0),
        Err(ResidencyError::TooLarge { .. })
    ));
    let mut bad = stt();
    bad.owner = " ".into();
    assert!(matches!(
        t.acquire(&bad, 0),
        Err(ResidencyError::Invalid(_))
    ));
    bad.owner = "x".into();
    bad.model = String::new();
    assert!(matches!(
        t.acquire(&bad, 0),
        Err(ResidencyError::Invalid(_))
    ));
    assert_eq!(
        t.release(LeaseId(99)),
        Err(ResidencyError::UnknownLease(LeaseId(99)))
    );
    assert!(t.touch(LeaseId(99), 0).is_err());
    let id = t.acquire(&stt(), 0).unwrap().lease.id;
    let mut forever = llm(1_000);
    forever.idle_unload_ms = 0;
    let keep = t.acquire(&forever, 0).unwrap().lease.id;
    assert!(t.reap_idle(599_999).is_empty());
    let reaped = t.reap_idle(600_000);
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].lease.id, id);
    assert_eq!(reaped[0].reason, RevokeReason::Idle);
    assert!(t.lease(keep).is_some());
    assert_eq!(t.release(keep).unwrap().id, keep);
    assert_eq!(t.snapshot().leases.len(), 0);
}

#[test]
fn events_describe_grants_and_changes() {
    let mut t = LeaseTable::new(Budget {
        vram_mb: 2_000,
        ram_mb: 10_000,
        ..baseline()
    });
    t.acquire(&llm(1_800), 0).unwrap();
    let g = t.acquire(&stt(), 1).unwrap();
    let evs = ResidencyEvent::from_grant(&g, true);
    let names: Vec<&str> = evs.iter().map(ResidencyEvent::name).collect();
    assert_eq!(names, [EVENT_EVICTED, EVENT_OOM_AVOIDED, EVENT_GRANTED]);
    let changes = t.set_mode(Mode {
        gaming: true,
        ..Mode::normal()
    });
    let evs = ResidencyEvent::from_changes(&changes);
    assert_eq!(evs[0].name(), EVENT_MOVED);
    assert_eq!(event_kind(EVENT_MOVED).as_str(), "residency.moved");
    let json = serde_json::to_value(&evs[0]).unwrap();
    assert_eq!(json["event"], "moved");
    assert_eq!(json["to"], "cpu");
    assert!(state_schema().get("title").is_some());
    assert!(event_schema().to_string().contains("budget_exceeded"));
    assert_eq!(LeaseId(3).to_string(), "lease-3");
}
