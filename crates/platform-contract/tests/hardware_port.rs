//! `HardwarePort` da się używać jako obiektu; metody z domyślną implementacją nie wymagają atrapy.

#![allow(clippy::unwrap_used)]

use platform_contract::{
    AudioEndpoint, CpuSummary, GpuAdapter, HardwarePort, OsSummary, PlatformError, PowerStatus,
};

struct Nothing;

impl HardwarePort for Nothing {
    fn os(&self) -> Result<OsSummary, PlatformError> {
        Err(PlatformError::Unsupported("os".into()))
    }
    fn cpu(&self) -> Result<CpuSummary, PlatformError> {
        Err(PlatformError::Unsupported("cpu".into()))
    }
    fn memory_total_mb(&self) -> Result<u64, PlatformError> {
        Ok(0)
    }
    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
        Ok(Vec::new())
    }
    fn npu(&self) -> Result<Option<String>, PlatformError> {
        Ok(None)
    }
    fn power_status(&self) -> Result<PowerStatus, PlatformError> {
        Ok(PowerStatus {
            ac_online: None,
            battery_present: false,
            battery_percent: None,
        })
    }
    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
        Ok(Vec::new())
    }
    fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
        Ok(None)
    }
}

#[test]
fn port_is_object_safe() {
    let port: Box<dyn HardwarePort> = Box::new(Nothing);
    assert!(
        matches!(port.gpu_usage(), Err(PlatformError::Unsupported(_))),
        "domyślnie bez liczników"
    );
    assert!(port.cpu().is_err());
    assert_eq!(port.gpus().unwrap(), Vec::new());
    assert!(!port.power_status().unwrap().battery_present);
}
