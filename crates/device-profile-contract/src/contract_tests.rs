//! Współdzielony test kontraktowy (feature `contract-tests`), uruchamiany na `-impl` i `-fake`.

use crate::{
    BASELINE_LIMITS, DeviceProfile, HwClass, MachineOverlay, PowerState, ResourceLimits,
    apply_overlay,
};

/// `current()` jest stabilne (to samo `MachineId`), a `recommend()` to reguły kontraktu
/// zastosowane do `current()` z domyślną nakładką.
pub fn current_is_stable_and_recommend_follows_rules<D: DeviceProfile>(dp: &D) {
    let a = dp.current();
    let b = dp.current();
    assert_eq!(a.machine_id, b.machine_id);
    assert_eq!(
        dp.recommend(),
        apply_overlay(&a, &MachineOverlay::default())
    );
}

/// Emulacja baseline zmniejsza limity i wymusza klasę `Baseline`; wyłączenie przywraca profil.
pub fn emulation_clamps_and_reverts<D: DeviceProfile>(dp: &D) {
    let original = dp.current();
    dp.emulate(Some(BASELINE_LIMITS))
        .unwrap_or_else(|e| panic!("{e}"));
    let emulated = dp.current();
    assert_eq!(emulated.machine_id, original.machine_id);
    assert!(emulated.cpu.logical_cores <= BASELINE_LIMITS.cpu_threads);
    assert!(emulated.cpu.physical_cores <= BASELINE_LIMITS.cpu_cores);
    assert!(emulated.ram_mb <= BASELINE_LIMITS.ram_mb);
    assert!(
        emulated
            .gpus
            .iter()
            .all(|g| g.vram_mb <= BASELINE_LIMITS.vram_mb)
    );
    assert_eq!(dp.recommend().class, HwClass::Baseline);
    let zero = ResourceLimits {
        vram_mb: 0,
        ..BASELINE_LIMITS
    };
    assert!(dp.emulate(Some(zero)).is_err());
    dp.emulate(None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(dp.current().emulation, None);
    assert_eq!(dp.current().ram_mb, original.ram_mb);
}

/// Stan zasilania jest spójny z profilem.
pub fn power_state_is_consistent<D: DeviceProfile>(dp: &D) {
    let power = dp.power_state();
    assert!(matches!(
        power,
        PowerState::Ac | PowerState::Battery { .. } | PowerState::Unknown
    ));
    assert_eq!(dp.current().power, power);
    let _ = dp.fullscreen_active();
    assert!(dp.refresh().is_ok());
}

/// Cały zestaw; `factory` daje świeżą instancję.
pub fn run_all<D, F>(factory: F)
where
    D: DeviceProfile,
    F: Fn() -> D,
{
    current_is_stable_and_recommend_follows_rules(&factory());
    emulation_clamps_and_reverts(&factory());
    power_state_is_consistent(&factory());
}
