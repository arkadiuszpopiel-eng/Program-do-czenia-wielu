//! Testy reguł oceny.

use super::*;
use crate::snapshot::{BatteryDiag, DiskDiag, EventSummary, VolumeDiag};

fn result(kind: CheckKind, metrics: Vec<Metric>) -> CheckResult {
    CheckResult {
        kind,
        verdict: Verdict::Pass,
        metrics,
        notes: Vec::new(),
        duration_ms: 1,
    }
}

#[test]
fn disk_thresholds_follow_media() {
    assert_eq!(
        DiskClass::of(MediaKind::Ssd, Some("NVMe"), false),
        DiskClass::Nvme
    );
    assert_eq!(
        DiskClass::of(MediaKind::Ssd, Some("SATA"), false),
        DiskClass::Ssd
    );
    assert_eq!(
        DiskClass::of(MediaKind::Hdd, Some("USB"), false),
        DiskClass::External
    );
    assert_eq!(
        DiskClass::of(MediaKind::Ssd, None, true),
        DiskClass::External
    );
    let m = |w: f64, l: f64| {
        vec![
            Metric::new("seq_write_mbps", "Zapis", w, "MB/s", true),
            Metric::new("rand_read_ms", "Odczyt losowy", l, "ms", false),
        ]
    };
    let disk = || CheckKind::Disk("C:".into());
    let mut fast = result(disk(), m(1500.0, 0.2));
    rate_disk(&mut fast, DiskClass::Nvme);
    assert_eq!(fast.verdict, Verdict::Pass);
    let mut slow = result(disk(), m(200.0, 0.2));
    rate_disk(&mut slow, DiskClass::Nvme);
    assert_eq!(slow.verdict, Verdict::Warn);
    let mut same_on_hdd = result(disk(), m(200.0, 12.0));
    rate_disk(&mut same_on_hdd, DiskClass::Hdd);
    assert_eq!(same_on_hdd.verdict, Verdict::Pass);
    let mut broken = result(disk(), m(1.0, 500.0));
    rate_disk(&mut broken, DiskClass::External);
    assert_eq!(broken.verdict, Verdict::Fail);
    let mut corrupt = result(
        disk(),
        vec![Metric::new("verify_errors", "Błędy", 3.0, "szt.", false)],
    );
    rate_disk(&mut corrupt, DiskClass::Ssd);
    assert_eq!(corrupt.verdict, Verdict::Fail);
}

#[test]
fn comparison_flags_drops_and_ignores_noise() {
    let before = result(
        CheckKind::CpuSingle,
        vec![
            Metric::new("score", "Wynik", 1000.0, "pkt", true),
            Metric::new("lat", "Opóźnienie", 0.2, "ms", false),
        ],
    );
    let mut same = result(
        CheckKind::CpuSingle,
        vec![
            Metric::new("score", "Wynik", 950.0, "pkt", true),
            Metric::new("lat", "Opóźnienie", 0.6, "ms", false),
        ],
    );
    compare(&mut same, &before);
    assert_eq!(same.verdict, Verdict::Pass, "{:?}", same.notes);
    let mut throttled = result(
        CheckKind::CpuSingle,
        vec![Metric::new("score", "Wynik", 600.0, "pkt", true)],
    );
    compare(&mut throttled, &before);
    assert_eq!(throttled.verdict, Verdict::Warn);
    assert!(throttled.notes[0].contains("spadek o 40%"));
    let mut other = result(
        CheckKind::Memory,
        vec![Metric::new("score", "Wynik", 1.0, "pkt", true)],
    );
    compare(&mut other, &before);
    assert_eq!(
        other.verdict,
        Verdict::Pass,
        "inny test nie jest porównywany"
    );
}

#[test]
fn integrity_reads_health_space_events_battery() {
    let mut s = Snapshot::default();
    s.memory.total_mb = 16_000;
    s.memory.available_mb = 8_000;
    let ok = integrity(&s);
    assert_eq!(ok.verdict, Verdict::Pass);
    assert_eq!(ok.notes, vec!["Bez zastrzeżeń.".to_owned()]);
    s.disks.push(DiskDiag {
        model: "Dysk USB".into(),
        health: Health::Warning,
        wear_pct: Some(75),
        ..DiskDiag::default()
    });
    s.volumes.push(VolumeDiag {
        letter: "E:".into(),
        size_mb: 1_000_000,
        free_mb: 80_000,
        ..VolumeDiag::default()
    });
    s.events = EventSummary {
        window_days: 7,
        counts: vec![(EventKind::Disk, 4), (EventKind::Usb, 2)],
        recent: Vec::new(),
    };
    s.battery = Some(BatteryDiag {
        design_mwh: Some(50_000),
        full_mwh: Some(25_000),
        ..BatteryDiag::default()
    });
    let r = integrity(&s);
    assert_eq!(r.verdict, Verdict::Warn, "same ostrzeżenia: {:?}", r.notes);
    let text = r.notes.join(" ");
    assert!(text.contains("ostrzeżenie kondycji") && text.contains("zużycie 75%"));
    assert!(text.contains("E: — wolne 8.0%") && text.contains("4 błędów dysku"));
    assert!(text.contains("Bateria straciła 50%"));
    assert_eq!(r.metric("disk_events"), Some(4.0));
}

#[test]
fn check_keys_roundtrip() {
    for k in [
        CheckKind::CpuSingle,
        CheckKind::CpuMulti,
        CheckKind::Memory,
        CheckKind::Disk("E:".into()),
        CheckKind::Network,
        CheckKind::Integrity,
    ] {
        assert_eq!(CheckKind::from_key(&k.key()), Some(k));
    }
    assert_eq!(
        CheckKind::from_key("disk:e:"),
        Some(CheckKind::Disk("E:".into()))
    );
    assert_eq!(CheckKind::from_key("disk:..\\"), None);
    let run = crate::CheckRun::new(
        "t".into(),
        vec![
            CheckResult::skipped(CheckKind::Network, "brak sieci"),
            result(CheckKind::Memory, Vec::new()),
        ],
    );
    assert_eq!(run.overall, Verdict::Pass);
}
