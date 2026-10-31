//! Testy parsera na odpowiedziach w kształcie PowerShell (listy jednoelementowe jako obiekty).

use super::*;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

const FULL: &str = r#"{
 "gaps": [{"part": "thermal", "error": "Odmowa dostępu"}],
 "system": {"caption": "Microsoft Windows 11 Pro", "version": "10.0.26200", "build": "26200",
   "boot": "2026-10-15T09:00:00.0000000Z", "total_kb": 33554432, "free_kb": 16777216,
   "maker": "Firma", "model": "Laptop 15", "bios": "1.20"},
 "cpu": {"name": " AMD Ryzen 7 ", "cores": 8, "threads": 16, "max": 3200, "cur": 2900, "load": 7,
   "l2": 8192, "l3": 16384, "virt": true},
 "modules": {"slot": "DIMM 0", "cap": 17179869184, "speed": 5600, "maker": "SK", "part": "X "},
 "pagefile": {"size": 4096, "used": 120},
 "disks": [
   {"model": "NVMe SSD", "serial": "4F2A", "media": "SSD", "bus": "NVMe", "size": 1000204886016,
    "health": "Healthy", "op": "OK", "wear": 2, "temp": 40, "hours": 1800, "rerr": 0, "werr": 0,
    "letters": "C:"},
   {"number": 2, "model": "USB Disk", "serial": null, "media": "Unspecified", "bus": "USB", "size": 2000398934016,
    "health": "Warning", "op": "OK", "wear": null, "temp": 0, "hours": null, "rerr": null, "werr": null,
    "letters": ["E:", "F:"]}],
 "volumes": [{"letter": "c:", "label": "OS", "fs": "NTFS", "size": 1000000000000, "free": 400000000000, "system": true},
             {"letter": "E:", "label": null, "fs": "NTFS", "size": 2000000000000, "free": 100000000000, "system": false}],
 "gpus": [{"name": "GeForce RTX", "drv": "32.0.15", "date": "2026-08-01", "ram": 4293918720, "reg": 8589934592, "status": "OK"},
          {"name": "Radeon iGPU", "drv": "31.0", "date": null, "ram": 536870912, "reg": null, "status": "OK"}],
 "net": {"name": "Wi-Fi", "desc": "Intel Wi-Fi 6E", "up": true, "bps": 866700000, "media": "Native 802.11"},
 "battery": {"charge": 80, "status": 2, "design": 80000, "full": 60000, "cycles": 0},
 "thermal": [{"name": "TZ00", "c": 47.5}, {"name": "TZ01", "c": -273.2}],
 "events": [
   {"at": "2026-10-14T08:00:00Z", "provider": "Ntfs", "id": 140, "msg": "Nie można opróżnić"},
   {"at": "2026-10-14T07:00:00Z", "provider": "disk", "id": 51, "msg": "Błąd urządzenia \\Device\\Harddisk2\\DR19."},
   {"at": "2026-10-14T07:00:00Z", "provider": "disk", "id": 51, "msg": "Błąd urządzenia \\Device\\Harddisk2\\DR19."},
   {"at": "2026-10-14T09:00:00Z", "provider": "UASPStor", "id": 129, "msg": "reset"},
   {"at": "2026-10-13T09:00:00Z", "provider": "Microsoft-Windows-Kernel-Power", "id": 41, "msg": "x"},
   {"at": "2026-10-12T09:00:00Z", "provider": "Application Error", "id": 1000, "msg": "app.exe"}]
}"#;

struct Fixed(Snapshot);

impl pc_diag_contract::DiagProbe for Fixed {
    fn snapshot(&self) -> Result<Snapshot, pc_diag_contract::DiagError> {
        Ok(self.0.clone())
    }
}

#[test]
fn parses_full_probe_output() {
    let s = parse(FULL, now()).unwrap();
    assert_eq!(s.machine.uptime_s, 3 * 3600);
    assert_eq!(s.machine.model.as_deref(), Some("Firma Laptop 15"));
    assert_eq!(s.cpu.name, "AMD Ryzen 7");
    assert_eq!((s.cpu.cores, s.cpu.threads), (8, 16));
    assert_eq!(s.memory.total_mb, 32_768);
    assert_eq!(s.memory.modules.len(), 1, "pojedynczy moduł jako obiekt");
    assert_eq!(s.memory.modules[0].capacity_mb, 16_384);
    assert_eq!(s.memory.pagefile_mb, Some(4096));
    assert_eq!(s.disks.len(), 2);
    assert_eq!(s.disks[0].media, MediaKind::Ssd);
    assert_eq!(s.disks[0].volumes, vec!["C:".to_owned()]);
    assert_eq!(s.disks[0].serial_tail.as_deref(), Some("…4F2A"));
    assert!(s.disks[1].removable && s.disks[1].health == Health::Warning);
    assert_eq!(s.disks[1].temperature_c, None, "0 °C = brak czujnika");
    assert_eq!(s.volumes[0].letter, "C:");
    assert_eq!(
        s.gpus[0].vram_mb,
        Some(8192),
        "VRAM z rejestru, nie z obciętego AdapterRAM"
    );
    assert_eq!(s.gpus[1].vram_mb, Some(512));
    assert_eq!(s.network[0].kind, "wifi");
    assert_eq!(s.network[0].link_mbps, Some(866));
    let b = s.battery.as_ref().unwrap();
    assert_eq!(b.on_ac, Some(true));
    assert_eq!(b.cycles, None);
    assert_eq!(b.wear_pct(), Some(25.0));
    assert_eq!(s.thermal.len(), 1, "czujnik bez odczytu pominięty");
    assert_eq!(s.events.count(EventKind::Disk), 3);
    assert_eq!(s.disks[1].event_errors, 2, "błędy przypisane do dysku 2");
    assert!(
        s.events.recent.iter().any(|e| e.message.ends_with("(×2)")),
        "powtórzenia zwinięte"
    );
    assert_eq!(s.events.count(EventKind::Usb), 1);
    assert_eq!(s.events.count(EventKind::UnexpectedShutdown), 1);
    assert_eq!(s.events.count(EventKind::AppCrash), 1);
    assert_eq!(s.events.recent[0].source, "UASPStor", "najnowsze pierwsze");
    assert_eq!(s.gaps, vec!["temperatury (Odmowa dostępu)".to_owned()]);
    pc_diag_contract::contract_tests::snapshot_is_consistent(&Fixed(s));
}

#[test]
fn tolerates_missing_parts_and_bom() {
    let s = parse(
        "\u{feff}{\"gaps\": null, \"cpu\": {\"name\": \"X\", \"cores\": 4}}",
        now(),
    )
    .unwrap();
    assert_eq!(s.cpu.threads, 4);
    assert!(s.disks.is_empty() && s.battery.is_none() && s.events.recent.is_empty());
    assert!(parse("Błąd PowerShell", now()).is_err());
}

#[test]
fn classifies_events() {
    assert_eq!(classify("disk", 153), EventKind::Disk);
    assert_eq!(classify("Microsoft-Windows-Ntfs", 98), EventKind::Disk);
    assert_eq!(classify("EventLog", 6008), EventKind::UnexpectedShutdown);
    assert_eq!(
        classify("Microsoft-Windows-WER-SystemErrorReporting", 1001),
        EventKind::Bugcheck
    );
    assert_eq!(
        classify("Microsoft-Windows-WHEA-Logger", 17),
        EventKind::Hardware
    );
    assert_eq!(classify("USBSTOR", 1), EventKind::Usb);
    assert_eq!(classify("EventLog", 6005), EventKind::Other);
    assert_eq!(disk_number(r"Błąd \Device\Harddisk12\DR3"), Some(12));
    assert_eq!(
        disk_number("ponowiona dla adresu 0x0 dysku 2 (nazwa PDO"),
        Some(2)
    );
    assert_eq!(disk_number("brak"), None);
}

#[test]
fn network_kind_from_adapter_type_and_name() {
    // Win32_NetworkAdapter (31.10): `AdapterType` bywa „Ethernet 802.3” także dla Wi-Fi i Bluetooth.
    assert_eq!(
        net_kind("Ethernet 802.3", "Intel(R) Wi-Fi 6E AX211 160MHz"),
        "wifi"
    );
    assert_eq!(
        net_kind("Ethernet 802.3", "Realtek PCIe GbE Family Controller"),
        "ethernet"
    );
    assert_eq!(
        net_kind("Ethernet 802.3", "Bluetooth Device (Personal Area Network)"),
        "other"
    );
    assert_eq!(net_kind("", "Karta nieznana"), "other");
}
