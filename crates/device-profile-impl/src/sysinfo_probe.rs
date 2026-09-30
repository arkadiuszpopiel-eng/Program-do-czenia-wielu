//! Sonda sprzętu poza Windows (Linux/CI, komputery deweloperskie): CPU i RAM z `sysinfo`,
//! L3 i bateria z `/sys` (Linux), identyfikator z `/etc/machine-id`. GPU/NPU/audio: brak danych.

use std::path::Path;

use platform_contract::{
    AudioEndpoint, CpuSummary, GpuAdapter, HardwarePort, OsSummary, PlatformError, PowerStatus,
};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Sonda przez `sysinfo` (bez stanu; każde wywołanie czyta system).
#[derive(Debug, Default, Clone, Copy)]
pub struct SysinfoProbe;

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Rozmiar L3 z `/sys` (np. „32768K”) w KB.
fn linux_l3_kb() -> Option<u32> {
    let text = read_trimmed(Path::new("/sys/devices/system/cpu/cpu0/cache/index3/size"))?;
    parse_cache_size_kb(&text)
}

/// „32768K” / „32M” → KB.
pub(crate) fn parse_cache_size_kb(text: &str) -> Option<u32> {
    let text = text.trim();
    let (digits, mult) = match text.chars().last()? {
        'K' | 'k' => (&text[..text.len() - 1], 1),
        'M' | 'm' => (&text[..text.len() - 1], 1024),
        _ => (text, 1),
    };
    digits.trim().parse::<u32>().ok()?.checked_mul(mult)
}

/// Bateria i zasilacz z `/sys/class/power_supply` (Linux); brak katalogu = bez baterii.
fn linux_power() -> PowerStatus {
    let mut status = PowerStatus {
        ac_online: None,
        battery_present: false,
        battery_percent: None,
    };
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return status;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        match read_trimmed(&dir.join("type")).as_deref() {
            Some("Battery") => {
                status.battery_present = true;
                status.battery_percent = read_trimmed(&dir.join("capacity"))
                    .and_then(|c| c.parse::<u8>().ok())
                    .filter(|p| *p <= 100);
            }
            Some("Mains") => {
                status.ac_online = read_trimmed(&dir.join("online")).map(|o| o == "1");
            }
            _ => {}
        }
    }
    status
}

impl HardwarePort for SysinfoProbe {
    fn os(&self) -> Result<OsSummary, PlatformError> {
        Ok(OsSummary {
            name: System::name().unwrap_or_else(|| std::env::consts::OS.into()),
            version: System::os_version().unwrap_or_default(),
            build: None,
        })
    }

    fn cpu(&self) -> Result<CpuSummary, PlatformError> {
        let sys =
            System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()));
        let cpus = sys.cpus();
        let logical = u32::try_from(cpus.len()).unwrap_or(u32::MAX).max(1);
        let physical = System::physical_core_count()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(logical)
            .max(1);
        Ok(CpuSummary {
            model: cpus
                .first()
                .map(|c| c.brand().trim().to_owned())
                .unwrap_or_default(),
            physical_cores: physical,
            logical_cores: logical.max(physical),
            l3_cache_kb: linux_l3_kb(),
        })
    }

    fn memory_total_mb(&self) -> Result<u64, PlatformError> {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        Ok(sys.total_memory() / (1024 * 1024))
    }

    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
        Ok(Vec::new())
    }

    fn npu(&self) -> Result<Option<String>, PlatformError> {
        Ok(None)
    }

    fn power_status(&self) -> Result<PowerStatus, PlatformError> {
        Ok(linux_power())
    }

    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
        Err(PlatformError::Unsupported(
            "urządzenia audio: sonda sysinfo ich nie wylicza".into(),
        ))
    }

    fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
        Ok(["/etc/machine-id", "/var/lib/dbus/machine-id"]
            .iter()
            .find_map(|p| read_trimmed(Path::new(p))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reads_this_machine() {
        let probe = SysinfoProbe;
        let cpu = probe.cpu().unwrap();
        assert!(cpu.physical_cores >= 1 && cpu.logical_cores >= cpu.physical_cores);
        assert!(probe.memory_total_mb().unwrap() > 0);
        assert!(probe.gpus().unwrap().is_empty());
        assert!(probe.npu().unwrap().is_none());
        assert!(probe.audio_endpoints().is_err());
        assert!(!probe.os().unwrap().name.is_empty());
        let power = probe.power_status().unwrap();
        assert!(power.battery_percent.is_none_or(|p| p <= 100));
        let _ = probe.machine_seed().unwrap();
    }

    #[test]
    fn cache_size_parsing() {
        assert_eq!(parse_cache_size_kb("32768K"), Some(32_768));
        assert_eq!(parse_cache_size_kb("96M"), Some(98_304));
        assert_eq!(parse_cache_size_kb("512"), Some(512));
        assert_eq!(parse_cache_size_kb("x"), None);
        assert_eq!(parse_cache_size_kb(""), None);
    }
}
