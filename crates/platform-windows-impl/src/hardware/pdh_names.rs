//! Nazwy instancji liczników wydajności kart graficznych Windows („GPU Adapter Memory”,
//! „GPU Process Memory”, „GPU Engine”) i składanie odczytów w [`GpuUsage`]. Czysta logika —
//! testowana na każdej platformie; odczyt liczników jest w `gpu_win.rs`.
//!
//! Przykładowe nazwy instancji:
//! - `luid_0x00000000_0x0000F1A3_phys_0` (karta),
//! - `pid_1234_luid_0x00000000_0x0000F1A3_phys_0` (proces na karcie),
//! - `pid_1234_luid_0x00000000_0x0000F1A3_phys_0_eng_3_engtype_3D` (silnik karty w procesie).

use std::collections::BTreeMap;

use platform_contract::{GpuAdapterUsage, GpuProcessUsage, GpuUsage};

const MB: f64 = 1024.0 * 1024.0;

/// Rozłożona nazwa instancji licznika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Instance {
    /// Proces (liczniki procesów i silników).
    pub pid: Option<u32>,
    /// LUID karty (`HighPart << 32 | LowPart`).
    pub luid: u64,
    /// Numer silnika karty (liczniki silników).
    pub engine: Option<u32>,
}

fn hex(token: &str) -> Option<u32> {
    let digits = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))?;
    u32::from_str_radix(digits, 16).ok()
}

/// Nazwa instancji → proces, karta, silnik; `None` — nazwa bez LUID (np. `_Total`).
pub(crate) fn parse_instance(name: &str) -> Option<Instance> {
    let tokens: Vec<&str> = name.split('_').collect();
    let mut pid = None;
    let mut luid = None;
    let mut engine = None;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "pid" => pid = tokens.get(i + 1).and_then(|t| t.parse::<u32>().ok()),
            "luid" => {
                let high = tokens.get(i + 1).and_then(|t| hex(t))?;
                let low = tokens.get(i + 2).and_then(|t| hex(t))?;
                luid = Some((u64::from(high) << 32) | u64::from(low));
                i += 2;
            }
            "eng" => engine = tokens.get(i + 1).and_then(|t| t.parse::<u32>().ok()),
            _ => {}
        }
        i += 1;
    }
    Some(Instance {
        pid,
        luid: luid?,
        engine,
    })
}

/// Surowe odczyty liczników: (nazwa instancji, wartość). Pamięć w bajtach, obciążenie w procentach.
#[derive(Debug, Clone, Default)]
pub(crate) struct Readings {
    /// „GPU Adapter Memory(*)\Dedicated Usage”.
    pub adapter_dedicated: Vec<(String, f64)>,
    /// „GPU Adapter Memory(*)\Shared Usage”.
    pub adapter_shared: Vec<(String, f64)>,
    /// „GPU Process Memory(*)\Dedicated Usage”.
    pub process_dedicated: Vec<(String, f64)>,
    /// „GPU Engine(*)\Utilization Percentage”.
    pub engine_utilization: Vec<(String, f64)>,
}

fn mb(bytes: f64) -> u64 {
    if bytes.is_finite() && bytes > 0.0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = (bytes / MB) as u64;
        value
    } else {
        0
    }
}

/// Suma wartości na kartę (karta połączona z kilku fizycznych — `phys_0`, `phys_1` — liczona razem).
fn per_luid(rows: &[(String, f64)]) -> BTreeMap<u64, f64> {
    let mut out = BTreeMap::new();
    for (name, value) in rows {
        if let Some(inst) = parse_instance(name).filter(|i| i.pid.is_none()) {
            *out.entry(inst.luid).or_insert(0.0) += value;
        }
    }
    out
}

/// Obciążenie karty: silniki sumowane po procesach, wynik — najbardziej zajęty silnik (0–100).
fn load_per_luid(rows: &[(String, f64)]) -> BTreeMap<u64, u32> {
    let mut engines: BTreeMap<(u64, u32), f64> = BTreeMap::new();
    for (name, value) in rows {
        let Some(inst) = parse_instance(name) else {
            continue;
        };
        if value.is_finite() && *value > 0.0 {
            *engines
                .entry((inst.luid, inst.engine.unwrap_or(0)))
                .or_insert(0.0) += value;
        }
    }
    let mut out: BTreeMap<u64, u32> = BTreeMap::new();
    for ((luid, _), sum) in engines {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pct = sum.clamp(0.0, 100.0).round() as u32;
        let slot = out.entry(luid).or_insert(0);
        *slot = (*slot).max(pct);
    }
    out
}

/// Odczyt kart i procesów z surowych liczników. Karta bez odczytu obciążenia ma `load_pct = None`
/// tylko wtedy, gdy liczniki silników nie zostały odczytane wcale.
pub(crate) fn assemble(r: &Readings) -> GpuUsage {
    let dedicated = per_luid(&r.adapter_dedicated);
    let shared = per_luid(&r.adapter_shared);
    let measured_load = !r.engine_utilization.is_empty();
    let load = load_per_luid(&r.engine_utilization);
    let mut luids: Vec<u64> = dedicated.keys().chain(shared.keys()).copied().collect();
    luids.sort_unstable();
    luids.dedup();
    let adapters = luids
        .into_iter()
        .map(|luid| GpuAdapterUsage {
            luid,
            dedicated_used_mb: mb(dedicated.get(&luid).copied().unwrap_or(0.0)),
            shared_used_mb: mb(shared.get(&luid).copied().unwrap_or(0.0)),
            load_pct: measured_load.then(|| load.get(&luid).copied().unwrap_or(0)),
        })
        .collect();
    let mut per_process: BTreeMap<(u32, u64), f64> = BTreeMap::new();
    for (name, value) in &r.process_dedicated {
        if let Some(Instance {
            pid: Some(pid),
            luid,
            ..
        }) = parse_instance(name)
        {
            *per_process.entry((pid, luid)).or_insert(0.0) += value;
        }
    }
    let processes = per_process
        .into_iter()
        .map(|((pid, luid), bytes)| GpuProcessUsage {
            pid,
            luid,
            dedicated_used_mb: mb(bytes),
            name: None,
        })
        .filter(|p| p.dedicated_used_mb > 0)
        .collect();
    GpuUsage {
        adapters,
        processes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUID: u64 = 0x0000_F1A3;

    fn row(name: &str, value: f64) -> (String, f64) {
        (name.to_owned(), value)
    }

    #[test]
    fn instance_names_are_parsed() {
        assert_eq!(
            parse_instance("luid_0x00000000_0x0000F1A3_phys_0"),
            Some(Instance {
                pid: None,
                luid: LUID,
                engine: None
            })
        );
        assert_eq!(
            parse_instance("pid_4242_luid_0x00000001_0x0000F1A3_phys_0_eng_3_engtype_VideoDecode"),
            Some(Instance {
                pid: Some(4242),
                luid: (1 << 32) | LUID,
                engine: Some(3)
            })
        );
        assert_eq!(parse_instance("_Total"), None);
        assert_eq!(parse_instance("luid_0xZZ_0x1_phys_0"), None);
        assert_eq!(parse_instance("luid_0x0"), None);
    }

    #[test]
    fn usage_is_assembled_per_card_and_process() {
        let other = 0x0000_1234;
        let r = Readings {
            adapter_dedicated: vec![
                row("luid_0x00000000_0x0000F1A3_phys_0", 3.0 * MB),
                row("luid_0x00000000_0x0000F1A3_phys_1", 1.0 * MB),
                row("luid_0x00000000_0x00001234_phys_0", 0.0),
                row("_Total", 99.0 * MB),
            ],
            adapter_shared: vec![row("luid_0x00000000_0x00001234_phys_0", 512.0 * MB)],
            process_dedicated: vec![
                row("pid_10_luid_0x00000000_0x0000F1A3_phys_0", 2.5 * MB),
                row("pid_11_luid_0x00000000_0x0000F1A3_phys_0", 0.0),
                row("pid_10_luid_0x00000000_0x00001234_phys_0", 1.0 * MB),
            ],
            engine_utilization: vec![
                row(
                    "pid_10_luid_0x00000000_0x0000F1A3_phys_0_eng_0_engtype_3D",
                    30.0,
                ),
                row(
                    "pid_11_luid_0x00000000_0x0000F1A3_phys_0_eng_0_engtype_3D",
                    25.4,
                ),
                row(
                    "pid_10_luid_0x00000000_0x0000F1A3_phys_0_eng_1_engtype_Compute",
                    70.0,
                ),
                row(
                    "pid_12_luid_0x00000000_0x0000F1A3_phys_0_eng_1_engtype_Compute",
                    50.0,
                ),
                row(
                    "pid_10_luid_0x00000000_0x0000F1A3_phys_0_eng_2_engtype_Copy",
                    f64::NAN,
                ),
            ],
        };
        let usage = assemble(&r);
        assert_eq!(
            usage.adapters,
            vec![
                GpuAdapterUsage {
                    luid: other,
                    dedicated_used_mb: 0,
                    shared_used_mb: 512,
                    load_pct: Some(0),
                },
                GpuAdapterUsage {
                    luid: LUID,
                    dedicated_used_mb: 4,
                    shared_used_mb: 0,
                    // Silnik obliczeń: 70 + 50 → obcięte do 100.
                    load_pct: Some(100),
                },
            ]
        );
        assert_eq!(
            usage.processes,
            vec![
                GpuProcessUsage {
                    pid: 10,
                    luid: other,
                    dedicated_used_mb: 1,
                    name: None,
                },
                GpuProcessUsage {
                    pid: 10,
                    luid: LUID,
                    dedicated_used_mb: 2,
                    name: None,
                },
            ]
        );
    }

    #[test]
    fn missing_engine_counters_give_no_load() {
        let r = Readings {
            adapter_dedicated: vec![row("luid_0x00000000_0x0000F1A3_phys_0", 8.0 * MB)],
            ..Readings::default()
        };
        let usage = assemble(&r);
        assert_eq!(usage.adapters.len(), 1);
        assert_eq!(usage.adapters[0].load_pct, None);
        assert_eq!(usage.adapters[0].dedicated_used_mb, 8);
        assert!(usage.processes.is_empty());
    }
}
