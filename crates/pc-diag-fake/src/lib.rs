//! Atrapa sondy parametrów `pc-diag`: laptop (8 rdzeni / 16 wątków, 32 GB, NVMe 1 TB i dysk USB
//! z ostrzeżeniem kondycji), zdarzenia dysku i USB z ostatnich dni. Test może podmienić zapis
//! albo wymusić błąd odczytu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, PoisonError};

use pc_diag_contract::{
    BatteryDiag, CpuDiag, DiagError, DiagProbe, DiskDiag, EventCount, EventKind, EventLine,
    EventSummary, GpuDiag, Health, MachineDiag, MediaKind, MemDiag, MemModule, NetDiag, Snapshot,
    ThermalDiag, VolumeDiag,
};

/// Przykładowy zapis parametrów.
pub fn sample() -> Snapshot {
    Snapshot {
        at: "2026-10-15T10:00:00Z".into(),
        machine: MachineDiag {
            os: "Windows 11 Pro".into(),
            os_version: "10.0.26200".into(),
            model: Some("Przykładowy Laptop 15".into()),
            bios: Some("1.2.3".into()),
            uptime_s: 3 * 3600,
        },
        cpu: CpuDiag {
            name: "Przykładowy CPU 8C".into(),
            cores: 8,
            threads: 16,
            base_mhz: Some(3200),
            current_mhz: Some(2900),
            load_pct: Some(12.0),
            l2_kb: Some(8192),
            l3_kb: Some(16384),
            virtualization: Some(true),
        },
        memory: MemDiag {
            total_mb: 32_768,
            available_mb: 18_000,
            pagefile_mb: Some(4096),
            pagefile_used_mb: Some(300),
            modules: vec![
                MemModule {
                    slot: Some("DIMM 0".into()),
                    capacity_mb: 16_384,
                    speed_mhz: Some(5600),
                    manufacturer: Some("Producent".into()),
                    part: Some("M-16G".into()),
                },
                MemModule {
                    slot: Some("DIMM 1".into()),
                    capacity_mb: 16_384,
                    speed_mhz: Some(5600),
                    manufacturer: Some("Producent".into()),
                    part: Some("M-16G".into()),
                },
            ],
        },
        disks: vec![
            DiskDiag {
                number: Some(0),
                model: "NVMe 1TB".into(),
                serial_tail: Some("…4F2A".into()),
                media: MediaKind::Ssd,
                bus: Some("NVMe".into()),
                size_mb: 953_869,
                health: Health::Healthy,
                operational: Some("OK".into()),
                wear_pct: Some(3),
                temperature_c: Some(41),
                power_on_hours: Some(1830),
                read_errors: Some(0),
                write_errors: Some(0),
                removable: false,
                volumes: vec!["C:".into()],
                event_errors: 0,
            },
            DiskDiag {
                number: Some(1),
                event_errors: 3,
                model: "Dysk USB 2TB".into(),
                serial_tail: None,
                media: MediaKind::Unknown,
                bus: Some("USB".into()),
                size_mb: 1_907_729,
                health: Health::Warning,
                operational: Some("OK".into()),
                removable: true,
                volumes: vec!["E:".into()],
                ..DiskDiag::default()
            },
        ],
        volumes: vec![
            VolumeDiag {
                letter: "C:".into(),
                label: Some("System".into()),
                file_system: Some("NTFS".into()),
                size_mb: 952_000,
                free_mb: 410_000,
                system: true,
            },
            VolumeDiag {
                letter: "E:".into(),
                label: Some("Dane".into()),
                file_system: Some("NTFS".into()),
                size_mb: 1_907_000,
                free_mb: 120_000,
                system: false,
            },
        ],
        gpus: vec![GpuDiag {
            name: "Przykładowa karta 8 GB".into(),
            driver_version: Some("32.0.15.6094".into()),
            driver_date: Some("2026-08-01".into()),
            vram_mb: Some(8192),
            status: Some("OK".into()),
        }],
        network: vec![NetDiag {
            name: "Wi-Fi".into(),
            description: Some("Karta sieci bezprzewodowej".into()),
            up: true,
            link_mbps: Some(866),
            kind: "wifi".into(),
        }],
        battery: Some(BatteryDiag {
            charge_pct: Some(84),
            on_ac: Some(true),
            design_mwh: Some(80_000),
            full_mwh: Some(70_400),
            cycles: Some(212),
        }),
        thermal: vec![ThermalDiag {
            name: "Strefa CPU".into(),
            celsius: 58.0,
        }],
        events: EventSummary {
            window_days: 7,
            counts: vec![
                EventCount {
                    kind: EventKind::Disk,
                    count: 3,
                },
                EventCount {
                    kind: EventKind::Usb,
                    count: 4,
                },
            ],
            recent: vec![EventLine {
                at: "2026-10-14T08:12:00Z".into(),
                source: "Ntfs".into(),
                id: 140,
                kind: EventKind::Disk,
                message: "Nie można opróżnić danych do dziennika transakcji (E:).".into(),
            }],
        },
        gaps: Vec::new(),
    }
}

/// Atrapa sondy.
pub struct FakeProbe {
    next: Mutex<Result<Snapshot, DiagError>>,
}

impl Default for FakeProbe {
    fn default() -> Self {
        Self {
            next: Mutex::new(Ok(sample())),
        }
    }
}

impl FakeProbe {
    /// Następne odczyty zwrócą ten wynik.
    pub fn set(&self, next: Result<Snapshot, DiagError>) {
        *self.next.lock().unwrap_or_else(PoisonError::into_inner) = next;
    }
}

impl DiagProbe for FakeProbe {
    fn snapshot(&self) -> Result<Snapshot, DiagError> {
        self.next
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_passes_contract_and_rates_usb_disk() {
        let probe = FakeProbe::default();
        let s = pc_diag_contract::contract_tests::snapshot_is_consistent(&probe);
        let r = pc_diag_contract::rating::integrity(&s);
        assert_eq!(r.verdict, pc_diag_contract::Verdict::Warn);
        probe.set(Err(DiagError::Probe("x".into())));
        assert!(probe.snapshot().is_err());
    }
}
