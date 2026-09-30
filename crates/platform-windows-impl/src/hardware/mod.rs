//! `HardwarePort`: surowe dane o sprzęcie dla `device-profile` (PLAN §3.5). Na Windows: rejestr,
//! `GetLogicalProcessorInformation`, `GetPhysicallyInstalledSystemMemory`, DXGI
//! (`IDXGIFactory1::EnumAdapters1`), DXCore (NPU), `GetSystemPowerStatus`, MMDevice (wątek MTA).
//! Poza Windows każda metoda zwraca `Unsupported` (`device-profile` ma wtedy własną sondę).

#[cfg(windows)]
mod win;

use platform_contract::{
    AudioEndpoint, CpuSummary, GpuAdapter, HardwarePort, OsSummary, PlatformError, PowerStatus,
};

/// Sonda sprzętu Windows (bezstanowa; każde wywołanie czyta system od nowa).
#[derive(Debug, Default, Clone, Copy)]
pub struct WinHardware;

#[cfg(not(windows))]
fn unsupported<T>(what: &str) -> Result<T, PlatformError> {
    Err(PlatformError::Unsupported(format!(
        "{what}: sonda sprzętu platform-windows działa tylko na Windows"
    )))
}

macro_rules! per_platform {
    ($win:expr, $what:literal) => {{
        #[cfg(windows)]
        {
            $win
        }
        #[cfg(not(windows))]
        {
            unsupported($what)
        }
    }};
}

impl HardwarePort for WinHardware {
    fn os(&self) -> Result<OsSummary, PlatformError> {
        per_platform!(win::os(), "system")
    }

    fn cpu(&self) -> Result<CpuSummary, PlatformError> {
        per_platform!(win::cpu(), "procesor")
    }

    fn memory_total_mb(&self) -> Result<u64, PlatformError> {
        per_platform!(win::memory_total_mb(), "pamięć")
    }

    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
        per_platform!(win::gpus(), "GPU")
    }

    fn npu(&self) -> Result<Option<String>, PlatformError> {
        per_platform!(win::npu(), "NPU")
    }

    fn power_status(&self) -> Result<PowerStatus, PlatformError> {
        per_platform!(win::power_status(), "zasilanie")
    }

    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
        per_platform!(win::audio_endpoints(), "audio")
    }

    fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
        per_platform!(win::machine_guid(), "identyfikator maszyny")
    }
}

/// Nazwa systemu z numeru kompilacji (Windows 11 ma nadal `ProductName` „Windows 10” w rejestrze).
pub(crate) fn windows_name(build: Option<u32>) -> &'static str {
    match build {
        Some(b) if b >= 22_000 => "Windows 11",
        Some(b) if b >= 10_240 => "Windows 10",
        _ => "Windows",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_name_from_build() {
        assert_eq!(windows_name(Some(26_100)), "Windows 11");
        assert_eq!(windows_name(Some(19_045)), "Windows 10");
        assert_eq!(windows_name(None), "Windows");
        let hw = WinHardware;
        if !cfg!(windows) {
            assert!(matches!(hw.cpu(), Err(PlatformError::Unsupported(_))));
            assert!(hw.gpus().is_err() && hw.os().is_err() && hw.npu().is_err());
            assert!(hw.memory_total_mb().is_err() && hw.power_status().is_err());
            assert!(hw.audio_endpoints().is_err() && hw.machine_seed().is_err());
        }
    }
}
