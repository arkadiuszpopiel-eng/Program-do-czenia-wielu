//! Karta dla rozpoznawania mowy Qwen3-ASR na komputerze z kilkoma kartami. Kompilacja Vulkan
//! `llama-server` widzi np. grafikę zintegrowaną i osobną kartę — bez `--device` potrafi wybrać
//! słabszą albo rozłożyć model na obie. Wybieramy główną kartę tą samą regułą co profil urządzenia
//! (karta z własną pulą pamięci, osobna przed zintegrowaną, największa) i dopasowujemy ją po nazwie
//! do listy `--list-devices` (ta sama logika co model rozmowy: `providers_local_impl::device_for`).
//! Przy jednej karcie serwer nie jest w ogóle odpytywany — zero kosztu na typowym komputerze.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use platform_contract::{GpuAdapter, HardwarePort};
use providers_local_impl::{BackendKey, EngineDevice, PROBE_TIMEOUT, device_for, probe};

use crate::gpu_mem::{discrete, hardware};

/// Główna karta z listy adapterów: z własną pulą pamięci, osobna przed zintegrowaną (z dużą
/// rezerwą z BIOS-u), potem największa — jak `Profile::primary_gpu`.
pub fn main_card(adapters: &[GpuAdapter]) -> Option<&GpuAdapter> {
    adapters.iter().filter(|a| discrete(a)).max_by_key(|a| {
        (
            u8::from(a.integrated != Some(true)),
            a.dedicated_vram_mb,
            std::cmp::Reverse(a.index),
        )
    })
}

/// Urządzenie serwera dla głównej karty; `None` — jedna karta (nie ma czego wybierać), serwer widzi
/// jedno urządzenie albo żadne nie pasuje nazwą (model startuje jak dotąd, bez wskazania).
pub fn device_choice(
    adapters: &[GpuAdapter],
    devices: &[EngineDevice],
    vulkan: bool,
) -> Option<String> {
    if adapters.iter().filter(|a| hardware(a)).count() < 2 || devices.len() < 2 {
        return None;
    }
    let card = main_card(adapters)?;
    let backend = if vulkan {
        BackendKey::Vulkan
    } else {
        BackendKey::Cuda
    };
    device_for(devices, backend, &card.name).map(|d| d.id.clone())
}

/// Lista urządzeń serwera z pamięci (sonda trwa do kilku sekund); pusty wynik nie jest zapamiętywany.
fn devices_of(server: &Path) -> Vec<EngineDevice> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<EngineDevice>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(found) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(server)
    {
        return found.clone();
    }
    let run = || probe(server, PROBE_TIMEOUT);
    // W wątku roboczym tokio sonda nie może blokować innych zadań.
    let found = match tokio::runtime::Handle::try_current() {
        Ok(h) if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(run)
        }
        _ => run(),
    };
    if !found.is_empty() {
        cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(server.to_path_buf(), found.clone());
    }
    found
}

/// `--device` dla Qwen3-ASR na tym komputerze (DXGI + `--list-devices`); `None` — bez wskazania.
pub fn qwen_device(server: &Path, vulkan: bool) -> Option<String> {
    let adapters = HardwarePort::gpus(&platform_windows_impl::WinHardware).ok()?;
    if adapters.iter().filter(|a| hardware(a)).count() < 2 {
        return None;
    }
    let devices = devices_of(server);
    let found = device_choice(&adapters, &devices, vulkan);
    match &found {
        Some(id) => {
            tracing::info!(urządzenie = %id, "rozpoznawanie mowy: Qwen3-ASR przypięty do karty")
        }
        None if devices.len() >= 2 => {
            tracing::warn!(
                "rozpoznawanie mowy: serwer nie widzi głównej karty po nazwie — start bez przypięcia"
            );
        }
        None => {}
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(
        index: u32,
        name: &str,
        vendor: u32,
        mb: u64,
        integrated: Option<bool>,
    ) -> GpuAdapter {
        GpuAdapter {
            name: name.into(),
            vendor_id: vendor,
            device_id: 1,
            dedicated_vram_mb: mb,
            shared_memory_mb: 16_000,
            software: false,
            index,
            luid: u64::from(index) + 1,
            integrated,
            driver_version: None,
        }
    }

    /// Laptop: Iris Xe (adapter 0) + RTX 4050 (adapter 1); Vulkan widzi obie w tej samej kolejności.
    /// Desktop z RX 9070 XT: jedna karta — wyjście `--list-devices` z 2026-10-17, bez przypięcia.
    #[test]
    fn main_card_is_pinned_only_with_two_cards() {
        let iris = adapter(0, "Intel(R) Iris(R) Xe Graphics", 0x8086, 128, Some(true));
        let rtx = adapter(
            1,
            "NVIDIA GeForce RTX 4050 Laptop GPU",
            0x10DE,
            5_921,
            Some(false),
        );
        let laptop = providers_local_impl::parse_devices(
            "Available devices:\n  Vulkan0: Intel(R) Iris(R) Xe Graphics (8012 MiB, 7200 MiB free)\n  Vulkan1: NVIDIA GeForce RTX 4050 Laptop GPU (6140 MiB, 5900 MiB free)\n",
        );
        let pair = [iris.clone(), rtx.clone()];
        assert_eq!(main_card(&pair).map(|a| a.index), Some(1));
        assert_eq!(
            device_choice(&pair, &laptop, true).as_deref(),
            Some("Vulkan1")
        );
        // Kompilacja CUDA widzi tylko kartę NVIDIA — jedno urządzenie, bez przypięcia.
        let cuda = providers_local_impl::parse_devices(
            "Available devices:\n  CUDA0: NVIDIA GeForce RTX 4050 Laptop GPU (6140 MiB, 5080 MiB free)\n",
        );
        assert_eq!(device_choice(&pair, &cuda, false), None);
        let rx = adapter(0, "AMD Radeon RX 9070 XT", 0x1002, 16_304, Some(false));
        let desktop = providers_local_impl::parse_devices(
            "Available devices:\n  Vulkan0: AMD Radeon RX 9070 XT (16304 MiB, 15416 MiB free)\n",
        );
        assert_eq!(
            device_choice(std::slice::from_ref(&rx), &desktop, true),
            None
        );
        // Karta, której serwer nie zna z nazwy — bez przypięcia (model startuje jak dotąd).
        let other = providers_local_impl::parse_devices(
            "Available devices:\n  Vulkan0: Intel(R) Iris(R) Xe Graphics (8012 MiB, 7200 MiB free)\n  Vulkan1: Inna karta (4096 MiB, 4000 MiB free)\n",
        );
        assert_eq!(device_choice(&pair, &other, true), None);
        // APU z dużą rezerwą obok osobnej karty: główna — osobna.
        let apu = adapter(0, "AMD Radeon 780M Graphics", 0x1002, 8_192, Some(true));
        let rx7600 = adapter(1, "AMD Radeon RX 7600S", 0x1002, 8_176, Some(false));
        assert_eq!(main_card(&[apu, rx7600]).map(|a| a.index), Some(1));
    }
}
