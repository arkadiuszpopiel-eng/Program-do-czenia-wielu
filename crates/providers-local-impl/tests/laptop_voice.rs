//! Rozmowa głosowa na laptopie właściciela (fala 6, na atrapach — `support::laptop`): kolejne tury
//! (STT whisper CUDA → odpowiedź Bielika) przez pół godziny czasu zarządcy rezydencji. Model
//! rozmowy ładuje się **raz**: nie jest wypierany przez STT (dzierżawa `GpuIfFree` — STT bierze
//! kartę tylko z wolnego miejsca, inaczej CPU) ani zwalniany jako „bezczynny”, choć odpowiada co
//! kilka minut (dzierżawa odświeżana i oznaczana jako używana w trakcie żądania).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use device_profile_contract::PowerState;
use model_residency_contract::{Device, Residency};
use support::arg;
use support::laptop::{Laptop, VRAM_BUDGET, laptop, laptop_with};

/// Minuta czasu zarządcy.
const MINUTE: u64 = 60_000;

/// Sześć tur co 6 minut (dłużej niż `idle_unload` = 10 min od pierwszej): po każdej turze zadanie
/// tła zarządcy; ani LLM, ani przypięty STT nie mogą zostać zwolnione.
async fn voice_turns(l: &Laptop) {
    for turn in 0..6 {
        let text = format!("tura {turn}");
        assert_eq!(l.ask(&text).await, format!("Echo: {text}"));
        let reaped = l.tick(6 * MINUTE);
        assert!(reaped.is_empty(), "tura {turn}: zwolniono {reaped:?}");
    }
    assert_eq!(l.env.launches().len(), 1, "bez przeładowań modelu rozmowy");
    let llm = l.llm_lease().unwrap();
    assert!(!llm.in_use, "po odpowiedzi dzierżawa nie jest „w użyciu”");
    assert_eq!(
        llm.last_used_ms,
        36 * MINUTE - 6 * MINUTE,
        "odświeżona ostatnią turą"
    );
    // Po rozmowie (bez żądań) model zwalnia się zwykłym trybem bezczynności.
    let reaped = l.tick(10 * MINUTE);
    assert_eq!(reaped, [llm.id]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_turns_on_ac_keep_llm_and_stt_on_gpu_without_reloads() {
    let l = laptop(&["llama-cuda", "llama-cpu"]);
    // Głos włączony przed pierwszą wiadomością: STT pierwszy na karcie, LLM (34 z 60 warstw
    // Bieliku 4.5B Q8_0) obok.
    let stt = l.start_stt();
    assert_eq!(stt.lease.device, Device::Gpu);
    assert_eq!(l.ask("dzień dobry").await, "Echo: dzień dobry");
    let llm = l.llm_lease().unwrap();
    assert_eq!((llm.device, llm.request.vram_mb), (Device::Gpu, 3_570));
    assert_eq!(arg(&l.env.launches()[0], "-ngl").as_deref(), Some("34"));
    assert_eq!(l.residency.snapshot().used.vram_mb, 3_570 + 1_500);
    assert!(l.residency.snapshot().used.vram_mb <= VRAM_BUDGET);
    voice_turns(&l).await;
    let stt_now = l.residency.lease(stt.lease.id).unwrap();
    assert_eq!(
        stt_now.device,
        Device::Gpu,
        "STT na karcie przez całą rozmowę"
    );
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_stt_reserve_stt_goes_to_cpu_and_llm_is_not_evicted() {
    // Bez rezerwy na STT LLM bierze 51 z 60 warstw (5080 MB); whisper CUDA się nie mieści.
    let l = laptop_with(&["llama-cuda", "llama-cpu"], "bielik-4.5b", |c| {
        c.stt_reserve_mb = 0;
    });
    assert_eq!(l.ask("najpierw tekst").await, "Echo: najpierw tekst");
    let llm = l.llm_lease().unwrap();
    assert_eq!((llm.device, llm.request.vram_mb), (Device::Gpu, 5_080));
    assert_eq!(arg(&l.env.launches()[0], "-ngl").as_deref(), Some("51"));
    let stt = l.start_stt();
    assert_eq!(
        stt.lease.device,
        Device::Cpu,
        "brak miejsca obok LLM → STT na CPU"
    );
    assert!(
        stt.evicted.is_empty(),
        "LLM nie jest wypierany: {:?}",
        stt.evicted
    );
    voice_turns(&l).await;
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_turns_with_the_light_model_keep_everything_on_gpu() {
    let l = laptop_with(&["llama-cuda", "llama-cpu"], "bielik-1.5b", |_| {});
    let stt = l.start_stt();
    assert_eq!(l.ask("krótko").await, "Echo: krótko");
    assert_eq!(l.llm_lease().unwrap().device, Device::Gpu);
    assert_eq!(stt.lease.device, Device::Gpu);
    voice_turns(&l).await;
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_turns_on_battery_llm_on_cpu_stt_on_gpu() {
    let l = laptop(&["llama-cuda", "llama-cpu"]);
    l.device
        .set_power(PowerState::Battery { percent: Some(55) });
    let stt = l.start_stt();
    assert_eq!(stt.lease.device, Device::Gpu, "STT CUDA także na baterii");
    assert_eq!(l.ask("na baterii").await, "Echo: na baterii");
    let llm = l.llm_lease().unwrap();
    assert_eq!(llm.device, Device::Cpu);
    assert_eq!(arg(&l.env.launches()[0], "-ngl").as_deref(), Some("0"));
    let state = l.residency.snapshot();
    assert!(state.used.ram_mb <= state.budget.ram_mb && state.used.vram_mb == 1_500);
    voice_turns(&l).await;
    l.provider.sidecar().stop("test").await;
}
