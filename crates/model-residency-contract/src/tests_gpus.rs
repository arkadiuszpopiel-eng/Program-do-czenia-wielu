//! Osobny budżet pamięci na kartę (C6.6): dwie osobne karty, karta zintegrowana obok osobnej
//! (laptop: Intel UHD + RTX 4050 6 GB), wykluczenie STT/TTS na każdej karcie, gra, emulacja,
//! zmiana kart, karta nieznana → główna.

use crate::tests::req;
use crate::*;

const A: GpuId = GpuId(1);
const B: GpuId = GpuId(0);
const IGPU: GpuId = GpuId(0);

fn pool(gpu: GpuId, vram_mb: u32, exclusive: bool, shared: bool) -> GpuPool {
    GpuPool {
        gpu,
        vram_mb,
        desktop_reserve_mb: if shared { 0 } else { 768 },
        stt_tts_exclusive: exclusive,
        shared,
    }
}

/// Karta A 8 GB (główna) i karta B 8 GB, 16 GB RAM dla modeli.
fn two_cards() -> LeaseTable {
    let budget = Budget {
        vram_mb: 7_408,
        ram_mb: 16_384,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    };
    LeaseTable::with_gpus(
        budget,
        GpuLayout {
            primary: A,
            others: vec![pool(B, 7_408, false, false)],
        },
    )
}

/// Laptop właściciela: RTX 4050 6 GB (główna, adapter 1) + Intel UHD (adapter 0, pamięć z RAM).
fn laptop() -> LeaseTable {
    let budget = Budget {
        vram_mb: 6_141 - 768,
        ram_mb: 8_192,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: true,
    };
    LeaseTable::with_gpus(
        budget,
        GpuLayout {
            primary: A,
            others: vec![pool(IGPU, 4_096, true, true)],
        },
    )
}

fn on(mut r: LeaseRequest, gpu: GpuId) -> LeaseRequest {
    r.gpu = Some(gpu);
    r
}

fn chat(vram: u32) -> LeaseRequest {
    req(
        "bielik-11b",
        ModelRole::Llm,
        Priority::Conversation,
        Placement::GpuPreferred,
        vram,
        8_000,
    )
}

fn speech() -> LeaseRequest {
    req(
        "qwen3-asr",
        ModelRole::Stt,
        Priority::VoiceRt,
        Placement::GpuIfFree,
        2_900,
        2_900,
    )
}

/// Przypadek z raportu nocnego: model rozmowy zajmuje kartę A, rozpoznawanie mowy na karcie B
/// dostaje kartę B — bez wypierania i bez spadku na procesor.
#[test]
fn speech_on_card_b_is_not_blocked_by_chat_on_card_a() {
    let mut t = two_cards();
    let llm = t.acquire(&chat(7_300), 0).unwrap().lease;
    assert_eq!((llm.device, llm.gpu), (Device::Gpu, Some(A)));
    let g = t.acquire(&on(speech(), B), 1).unwrap();
    assert_eq!((g.lease.device, g.lease.gpu), (Device::Gpu, Some(B)));
    assert!(g.evicted.is_empty(), "{:?}", g.evicted);
    assert!(t.lease(llm.id).is_some());
    assert_eq!((t.used_on(A), t.used_on(B)), (7_300, 2_900));
    assert_eq!(t.used().vram_mb, 7_300, "`used` — karta główna jak dotąd");
    let s = t.snapshot();
    assert_eq!(s.gpu(B).map(GpuState::free_mb), Some(7_408 - 2_900));
    assert!(s.gpu(A).is_some_and(|g| g.primary) && s.within_budget());
    // Ta sama mowa bez wskazania karty (albo z kartą nieznaną) — karta główna: pełna, więc
    // `GpuIfFree` idzie na procesor zamiast wypierać rozmowę.
    t.release(g.lease.id).unwrap();
    for gpu in [None, Some(GpuId(9))] {
        let mut r = speech();
        r.gpu = gpu;
        let g = t.acquire(&r, 2).unwrap();
        assert_eq!((g.lease.device, g.lease.gpu), (Device::Cpu, None));
        t.release(g.lease.id).unwrap();
    }
}

/// Eksmisja na karcie B wypiera tylko dzierżawy z karty B, gdy RAM wystarcza.
#[test]
fn eviction_stays_on_the_target_card() {
    let mut t = two_cards();
    let a = t.acquire(&chat(7_000), 0).unwrap().lease.id;
    let b = t.acquire(&on(chat(6_000), B), 1).unwrap().lease.id;
    let mut voice = on(speech(), B);
    voice.placement = Placement::GpuOnly;
    let g = t.acquire(&voice, 2).unwrap();
    assert_eq!(g.lease.gpu, Some(B));
    let victims: Vec<LeaseId> = g.evicted.iter().map(|r| r.lease.id).collect();
    assert_eq!(victims, [b], "rozmowa na karcie A zostaje");
    assert!(t.lease(a).is_some() && t.within_budget());
    // Karta B pełna dzierżaw głosu w użyciu: żądanie czeka, karta A nie jest ofiarą.
    t.set_in_use(g.lease.id, true, 3).unwrap();
    let mut second = voice.clone();
    second.vram_mb = 5_000;
    match t.acquire(&second, 4) {
        Err(ResidencyError::Wait { blockers }) => assert!(blockers.contains(&g.lease.id)),
        other => panic!("{other:?}"),
    }
    assert!(t.lease(a).is_some());
}

/// Laptop: lekki model na karcie zintegrowanej zajmuje jej pulę i RAM; model rozmowy na RTX
/// zostaje nietknięty; pula karty zintegrowanej jest osobna.
#[test]
fn laptop_integrated_card_uses_its_pool_and_ram() {
    let mut t = laptop();
    let llm = t.acquire(&chat(5_080), 0).unwrap().lease.id;
    let mut light = on(chat(3_000), IGPU);
    light.model = "bielik-1.5b".into();
    light.placement = Placement::GpuOnly;
    let g = t.acquire(&light, 1).unwrap();
    assert_eq!((g.lease.device, g.lease.gpu), (Device::Gpu, Some(IGPU)));
    assert!(g.evicted.is_empty());
    assert_eq!(t.used_on(IGPU), 3_000);
    assert_eq!(
        t.used().ram_mb,
        200 + 200 + 3_000,
        "pamięć karty zintegrowanej to RAM"
    );
    // Drugi lekki model nie mieści się w puli karty zintegrowanej (4 GB): wypiera pierwszy (równy
    // priorytet, nieużywany), model rozmowy na RTX zostaje.
    let g2 = t.acquire(&light, 2).unwrap();
    assert_eq!(g2.evicted.len(), 1);
    assert_eq!(g2.evicted[0].lease.id, g.lease.id);
    assert!(t.lease(llm).is_some());
    // Pamięć karty zintegrowanej to RAM: model na procesorze, który nie zmieści się obok, wypiera
    // najdawniej używany model z karty zintegrowanej (zwalnia RAM) — model rozmowy na RTX był
    // używany przed chwilą.
    t.touch(llm, 3).unwrap();
    let mut cpu = chat(0);
    cpu.placement = Placement::CpuOnly;
    cpu.cpu_ram_mb = 8_192 - 400 - 1_000;
    let g3 = t.acquire(&cpu, 4).unwrap();
    assert_eq!(g3.evicted.len(), 1);
    assert_eq!(g3.evicted[0].lease.id, g2.lease.id);
    assert!(t.lease(llm).is_some() && t.within_budget());
}

/// Wykluczenie STT/ciężki TTS dotyczy jednej karty: STT na karcie zintegrowanej i TTS na RTX
/// mogą być naraz, na tej samej karcie — wymiana.
#[test]
fn stt_tts_exclusion_is_per_card() {
    let mut t = laptop();
    let tts = req(
        "chatterbox",
        ModelRole::Tts,
        Priority::VoiceRt,
        Placement::GpuPreferred,
        2_000,
        2_500,
    );
    let tts = t.acquire(&tts, 0).unwrap().lease;
    let mut stt = on(speech(), IGPU);
    stt.vram_mb = 1_500;
    let g = t.acquire(&stt, 1).unwrap();
    assert_eq!((g.lease.gpu, g.evicted.len()), (Some(IGPU), 0));
    t.release(g.lease.id).unwrap();
    stt.gpu = Some(A);
    stt.placement = Placement::GpuPreferred;
    let g = t.acquire(&stt, 2).unwrap();
    assert_eq!(g.lease.gpu, Some(A));
    assert_eq!(g.evicted[0].lease.id, tts.id);
    assert_eq!(
        g.evicted[0].reason,
        RevokeReason::Exclusive { by: g.lease.id }
    );
}

#[test]
fn gaming_frees_every_card_and_emulation_limits_other_cards() {
    let mut t = two_cards();
    t.acquire(&chat(3_000), 0).unwrap();
    let b = t.acquire(&on(chat(6_000), B), 0).unwrap().lease.id;
    let changes = t.set_mode(Mode {
        emulated: Some(Budget {
            vram_mb: 5_000,
            ram_mb: 16_384,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        }),
        ..Mode::normal()
    });
    assert!(matches!(&changes[..],
        [Change::Revoked(r)] if r.lease.id == b && r.reason == RevokeReason::BudgetShrunk));
    assert_eq!(t.pools().card(B).map(|p| p.vram_mb), Some(5_000));
    t.set_mode(Mode::normal());
    t.acquire(&on(chat(6_000), B), 1).unwrap();
    let changes = t.set_mode(Mode {
        gaming: true,
        ..Mode::normal()
    });
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, Change::Moved(l) if l.gpu.is_none()))
    );
    assert_eq!((t.used_on(A), t.used_on(B)), (0, 0));
}

#[test]
fn removed_card_revokes_its_leases_and_unknown_card_falls_back() {
    let mut t = two_cards();
    let a = t.acquire(&chat(3_000), 0).unwrap().lease.id;
    let b = t.acquire(&on(chat(3_000), B), 0).unwrap().lease.id;
    let changes = t.set_gpus(GpuLayout::single(A));
    assert!(matches!(&changes[..],
        [Change::Revoked(r)] if r.lease.id == b && r.reason == RevokeReason::BudgetShrunk));
    assert!(t.lease(a).is_some());
    assert!(t.snapshot().gpus.len() == 1 && t.used_on(B) == 0);
    let g = t.acquire(&on(chat(3_000), B), 1).unwrap();
    assert_eq!(g.lease.gpu, Some(A), "karta bez puli → główna");
    // Za duży na kartę docelową i na procesor: błąd podaje pulę tej karty.
    let mut t = two_cards();
    t.set_gpus(GpuLayout {
        primary: A,
        others: vec![pool(B, 2_000, false, false)],
    });
    let mut huge = on(chat(3_000), B);
    huge.placement = Placement::GpuOnly;
    assert!(matches!(
        t.acquire(&huge, 0),
        Err(ResidencyError::TooLarge { vram_mb: 2_000, .. })
    ));
}

/// Tablica bez kart z profilu (`new`: karta główna to umowna `GpuId(0)`), potem prawdziwe karty
/// laptopa: dzierżawa „na karcie głównej” idzie za kartą główną (RTX, adapter 1), nie zostaje
/// na Intel UHD (adapter 0); wskazana wprost karta 0 zostaje na niej.
#[test]
fn leases_on_primary_follow_it_when_real_cards_arrive() {
    let laptop_layout = || laptop().gpus().clone();
    let laptop_budget = laptop().budget();
    let mut t = LeaseTable::new(laptop_budget);
    let g = t.acquire(&chat(5_000), 0).unwrap().lease;
    assert_eq!((g.device, g.gpu), (Device::Gpu, Some(GpuId(0))));
    let changes = t.set_gpus(laptop_layout());
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(t.lease(g.id).and_then(|l| l.gpu), Some(A));
    assert_eq!((t.used_on(A), t.used_on(IGPU)), (5_000, 0));
    assert!(t.within_budget());
    let mut t = LeaseTable::new(laptop_budget);
    let pinned = t.acquire(&on(speech(), GpuId(0)), 0).unwrap().lease;
    t.set_gpus(laptop_layout());
    assert_eq!(t.lease(pinned.id).and_then(|l| l.gpu), Some(IGPU));
    // Zmiana sprzętu naraz: mniejszy budżet tymczasowy nie odbiera dzierżawy, która mieści się
    // w nowym budżecie i na nowych kartach.
    let small = Budget {
        vram_mb: 5_000,
        ..laptop_budget
    };
    let mut t = LeaseTable::new(small);
    let g = t.acquire(&chat(5_000), 0).unwrap().lease;
    assert!(t.set_hardware(laptop_budget, laptop_layout()).is_empty());
    assert_eq!(t.lease(g.id).and_then(|l| l.gpu), Some(A));
    assert_eq!(t.budget(), laptop_budget);
}
