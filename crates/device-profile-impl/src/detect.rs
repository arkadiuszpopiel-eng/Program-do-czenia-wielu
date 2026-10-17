//! Mapowanie surowych danych `HardwarePort` na `Profile` (przenośne, testowane na atrapach).

use std::collections::BTreeSet;

use device_profile_contract::{
    AudioDevice, AudioDirection as Direction, BatteryInfo, CpuInfo, GpuInfo, GpuVendor, MachineId,
    NpuInfo, OsInfo, PowerState, Profile,
};
use platform_contract::{AudioDirection, AudioEndpoint, GpuAdapter, HardwarePort, PowerStatus};

/// Identyfikator PCI Microsoftu (Basic Render Driver, zdalny pulpit) — nie jest prawdziwym GPU.
const MICROSOFT_PCI: u32 = 0x1414;

/// Pełna detekcja; brakujące informacje dają wartości bezpieczne (nie błąd).
pub(crate) fn build_profile(hw: &dyn HardwarePort, machine_id: MachineId) -> Profile {
    let os = hw.os().map_or_else(
        |_| OsInfo {
            name: std::env::consts::OS.into(),
            version: String::new(),
            build: None,
        },
        |o| OsInfo {
            name: o.name,
            version: o.version,
            build: o.build,
        },
    );
    let cpu = hw.cpu().map_or_else(
        |_| {
            let logical = std::thread::available_parallelism()
                .map_or(1, |n| u32::try_from(n.get()).unwrap_or(u32::MAX));
            CpuInfo {
                model: String::new(),
                physical_cores: logical,
                logical_cores: logical,
                l3_cache_mb: None,
            }
        },
        |c| {
            let physical = c.physical_cores.max(1);
            CpuInfo {
                model: c.model.trim().to_owned(),
                physical_cores: physical,
                logical_cores: c.logical_cores.max(physical),
                l3_cache_mb: c.l3_cache_kb.map(|kb| kb.div_ceil(1024)),
            }
        },
    );
    let (battery, power) = power_of(hw.power_status().ok());
    Profile {
        machine_id,
        os,
        cpu,
        ram_mb: hw
            .memory_total_mb()
            .map_or(0, |mb| u32::try_from(mb).unwrap_or(u32::MAX)),
        gpus: hw.gpus().map(map_gpus).unwrap_or_default(),
        npu: hw.npu().ok().flatten().map(|name| NpuInfo { name }),
        battery,
        power,
        audio: hw.audio_endpoints().ok().map(map_audio),
        emulation: None,
    }
}

/// Tylko sprzętowe GPU z backendami producenta. System potrafi wymienić ten sam adapter dwa
/// razy (ten sam numer) — to odrzucamy; dwie takie same karty mają różne numery i zostają obie.
pub(crate) fn map_gpus(adapters: Vec<GpuAdapter>) -> Vec<GpuInfo> {
    let mut seen = BTreeSet::new();
    let mb = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
    adapters
        .into_iter()
        .filter(|a| !a.software && a.vendor_id != MICROSOFT_PCI)
        .filter(|a| seen.insert((a.index, a.vendor_id, a.device_id)))
        .map(|a| {
            let mut gpu = GpuInfo::describe(
                a.index,
                &a.name,
                GpuVendor::from_pci(a.vendor_id),
                mb(a.dedicated_vram_mb),
                mb(a.shared_memory_mb),
            )
            .with_hardware_flag(a.integrated);
            gpu.luid = a.luid;
            gpu.driver_version = a.driver_version;
            gpu
        })
        .collect()
}

fn map_audio(endpoints: Vec<AudioEndpoint>) -> Vec<AudioDevice> {
    endpoints
        .into_iter()
        .map(|e| AudioDevice {
            name: e.name,
            direction: match e.direction {
                AudioDirection::Capture => Direction::Input,
                AudioDirection::Render => Direction::Output,
            },
        })
        .collect()
}

/// Bateria i zasilanie ze stanu systemu.
pub(crate) fn power_of(status: Option<PowerStatus>) -> (Option<BatteryInfo>, PowerState) {
    let Some(s) = status else {
        return (None, PowerState::Unknown);
    };
    let battery = s.battery_present.then_some(BatteryInfo {
        percent: s.battery_percent,
    });
    let power = match (s.ac_online, s.battery_present) {
        (Some(true), _) | (None, false) => PowerState::Ac,
        (Some(false), true) => PowerState::Battery {
            percent: s.battery_percent,
        },
        _ => PowerState::Unknown,
    };
    (battery, power)
}

/// Czy zmienił się sprzęt (zasilanie ma osobne zdarzenie i nie jest porównywane).
pub(crate) fn hardware_changed(old: &Profile, new: &Profile) -> bool {
    let strip = |p: &Profile| Profile {
        power: PowerState::Unknown,
        ..p.clone()
    };
    strip(old) != strip(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, vendor_id: u32, vram: u64, software: bool) -> GpuAdapter {
        GpuAdapter {
            name: name.into(),
            vendor_id,
            device_id: 1,
            dedicated_vram_mb: vram,
            shared_memory_mb: 8_000,
            software,
            index: 0,
            luid: 0,
            integrated: None,
            driver_version: None,
        }
    }

    /// Procesor AMD z grafiką (APU): BIOS rezerwuje 4 GB „dedykowanej” pamięci, ale sterownik mówi
    /// „zintegrowana” — ta flaga wygrywa ze zgadywaniem po pamięci. LUID i sterownik przechodzą
    /// do profilu.
    #[test]
    fn driver_flag_wins_over_memory_guess() {
        let mut apu = adapter("AMD Radeon 780M Graphics", 0x1002, 4_096, false);
        apu.integrated = Some(true);
        apu.luid = 0xD1F2;
        apu.driver_version = Some("32.0.12033.1030".into());
        let mut arc = adapter("Intel(R) Arc(TM) A770 Graphics", 0x8086, 16_224, false);
        arc.index = 1;
        arc.integrated = Some(false);
        let mut old = adapter("Intel(R) UHD Graphics 620", 0x8086, 128, false);
        old.index = 2;
        let gpus = map_gpus(vec![apu, arc, old]);
        let kinds: Vec<(bool, u64)> = gpus.iter().map(|g| (g.integrated, g.luid)).collect();
        assert_eq!(kinds, [(true, 0xD1F2), (false, 0), (true, 0)]);
        assert_eq!(gpus[0].driver_version.as_deref(), Some("32.0.12033.1030"));
        // Bez flagi (stary system): 4 GB AMD wyglądałoby na osobną kartę.
        let guessed = map_gpus(vec![adapter("AMD Radeon 780M", 0x1002, 4_096, false)]);
        assert!(!guessed[0].integrated);
    }

    /// Laptop z dwiema kartami: obie zostają, każda z numerem i typem; dwie identyczne karty
    /// (stacja robocza) nie zlewają się w jedną.
    #[test]
    fn every_card_is_kept_with_its_number_and_kind() {
        let at = |index: u32, mut a: GpuAdapter| {
            a.index = index;
            a
        };
        let gpus = map_gpus(vec![
            at(
                0,
                adapter("Intel(R) Iris(R) Xe Graphics", 0x8086, 128, false),
            ),
            at(
                1,
                adapter("NVIDIA GeForce RTX 4050 Laptop GPU", 0x10DE, 5_921, false),
            ),
            at(
                2,
                adapter("NVIDIA GeForce RTX 4050 Laptop GPU", 0x10DE, 5_921, false),
            ),
            at(
                2,
                adapter("NVIDIA GeForce RTX 4050 Laptop GPU", 0x10DE, 5_921, false),
            ),
            at(3, adapter("Microsoft Basic Render Driver", 0x1414, 0, true)),
        ]);
        let seen: Vec<(u32, bool, u32)> = gpus
            .iter()
            .map(|g| (g.id, g.integrated, g.shared_mb))
            .collect();
        assert_eq!(
            seen,
            [(0, true, 8_000), (1, false, 8_000), (2, false, 8_000)]
        );
    }

    #[test]
    fn gpus_are_filtered_deduplicated_and_mapped() {
        let gpus = map_gpus(vec![
            adapter("NVIDIA GeForce RTX 4050 Laptop GPU ", 0x10DE, 5_921, false),
            adapter("NVIDIA GeForce RTX 4050 Laptop GPU ", 0x10DE, 5_921, false),
            adapter("Microsoft Basic Render Driver", MICROSOFT_PCI, 0, true),
            adapter("Intel(R) Iris(R) Xe Graphics", 0x8086, 128, false),
        ]);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vendor, GpuVendor::Nvidia);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4050 Laptop GPU");
        assert_eq!(gpus[1].vendor, GpuVendor::Intel);
    }

    #[test]
    fn power_mapping() {
        let st = |ac, present, pct| PowerStatus {
            ac_online: ac,
            battery_present: present,
            battery_percent: pct,
        };
        assert_eq!(power_of(None), (None, PowerState::Unknown));
        assert_eq!(
            power_of(Some(st(Some(true), false, None))).1,
            PowerState::Ac
        );
        assert_eq!(power_of(Some(st(None, false, None))).1, PowerState::Ac);
        let (battery, power) = power_of(Some(st(Some(false), true, Some(40))));
        assert_eq!(battery, Some(BatteryInfo { percent: Some(40) }));
        assert_eq!(power, PowerState::Battery { percent: Some(40) });
        assert_eq!(
            power_of(Some(st(None, true, Some(40)))).1,
            PowerState::Unknown
        );
        assert_eq!(
            power_of(Some(st(Some(false), false, None))).1,
            PowerState::Unknown
        );
    }
}
