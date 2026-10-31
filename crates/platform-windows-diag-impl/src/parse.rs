//! Przekład JSON sondy PowerShell na [`Snapshot`]. Tolerancyjny: PowerShell zamienia listę
//! jednoelementową w obiekt, a pustą w `null` ([`list`]); brakujące pola zostają `None`.
//! Zdarzenia są klasyfikowane według dostawcy i numeru.

use chrono::{DateTime, Utc};
use pc_diag_contract::{
    BatteryDiag, CpuDiag, DiskDiag, EVENT_WINDOW_DAYS, EventCount, EventKind, EventLine,
    EventSummary, GpuDiag, Health, MachineDiag, MediaKind, MemDiag, MemModule, NetDiag, Snapshot,
    ThermalDiag, VolumeDiag,
};
use serde_json::Value;

/// Najwięcej zdarzeń w podsumowaniu.
const MAX_RECENT: usize = 30;
/// Wartość `AdapterRAM` przy 4 GB i więcej (pole 32-bitowe) — niewiarygodna.
const ADAPTER_RAM_CAP: u64 = 4 * 1024 * 1024 * 1024 - 1;

/// Lista z wartości (tablica, pojedynczy obiekt albo nic).
fn list(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

fn s(v: &Value, k: &str) -> Option<String> {
    match &v[k] {
        Value::String(t) if !t.trim().is_empty() => Some(t.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn u(v: &Value, k: &str) -> Option<u64> {
    match &v[k] {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64)),
        Value::String(t) => t.trim().parse().ok(),
        _ => None,
    }
}

fn u32_of(v: &Value, k: &str) -> Option<u32> {
    u(v, k).and_then(|n| u32::try_from(n).ok())
}

const MB: u64 = 1024 * 1024;

/// Nazwy części sondy po polsku (luki w zapisie).
fn part_label(part: &str) -> &str {
    match part {
        "system" => "system",
        "cpu" => "procesor",
        "memory_modules" => "moduły pamięci",
        "pagefile" => "plik stronicowania",
        "disks" => "dyski fizyczne",
        "volumes" => "woluminy",
        "gpus" => "karty graficzne",
        "network" => "karty sieciowe",
        "battery" => "bateria",
        "thermal" => "temperatury",
        "events" => "dziennik zdarzeń",
        other => other,
    }
}

/// Rodzaj zdarzenia z dostawcy i numeru.
pub fn classify(provider: &str, id: u32) -> EventKind {
    let p = provider.to_ascii_lowercase();
    match (p.as_str(), id) {
        ("microsoft-windows-kernel-power", 41) | ("eventlog", 6008) => {
            EventKind::UnexpectedShutdown
        }
        ("microsoft-windows-wer-systemerrorreporting", 1001) => EventKind::Bugcheck,
        ("microsoft-windows-whea-logger", _) => EventKind::Hardware,
        ("application error", 1000) => EventKind::AppCrash,
        (p, _) if p.contains("uaspstor") || p.contains("usbstor") || p.contains("usbhub") => {
            EventKind::Usb
        }
        (p, _)
            if p == "disk"
                || p.contains("ntfs")
                || p.contains("stornvme")
                || p.contains("storahci")
                || p.contains("storport")
                || p == "volmgr" =>
        {
            EventKind::Disk
        }
        _ => EventKind::Other,
    }
}

fn media(text: &str) -> MediaKind {
    match text.to_ascii_lowercase().as_str() {
        "ssd" | "4" => MediaKind::Ssd,
        "hdd" | "3" => MediaKind::Hdd,
        _ => MediaKind::Unknown,
    }
}

fn health(text: &str) -> Health {
    match text.to_ascii_lowercase().as_str() {
        "healthy" | "0" => Health::Healthy,
        "warning" | "1" => Health::Warning,
        "unhealthy" | "2" => Health::Unhealthy,
        _ => Health::Unknown,
    }
}

fn net_kind(media: &str, desc: &str) -> &'static str {
    let m = format!("{media} {desc}").to_ascii_lowercase();
    if m.contains("bluetooth") {
        // Sieć przez Bluetooth: Win32_NetworkAdapter podaje ją jako „Ethernet 802.3”.
        "other"
    } else if m.contains("802.11")
        || m.contains("wireless")
        || m.contains("wi-fi")
        || m.contains("wifi")
    {
        "wifi"
    } else if m.contains("802.3") || m.contains("ethernet") {
        "ethernet"
    } else {
        "other"
    }
}

/// Numer dysku z treści zdarzenia (`\Device\Harddisk2\…`, „dysku 2”, „disk 2”).
pub fn disk_number(message: &str) -> Option<u32> {
    let lower = message.to_lowercase();
    ["harddisk", "dysku ", "disk "].iter().find_map(|key| {
        let at = lower.find(key)? + key.len();
        let digits: String = lower[at..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    })
}

/// Powtórzenia (to samo źródło, numer i treść) zwinięte do jednego wpisu z licznikiem; najwyżej
/// [`MAX_RECENT`] wpisów.
fn collapse(events: Vec<EventLine>) -> Vec<EventLine> {
    let mut out: Vec<(EventLine, u32)> = Vec::new();
    for e in events {
        match out
            .iter_mut()
            .find(|(x, _)| x.source == e.source && x.id == e.id && x.message == e.message)
        {
            Some((_, n)) => *n += 1,
            None => out.push((e, 1)),
        }
    }
    out.into_iter()
        .take(MAX_RECENT)
        .map(|(mut e, n)| {
            if n > 1 {
                e.message = format!("{} (×{n})", e.message);
            }
            e
        })
        .collect()
}

/// Zapis parametrów z JSON sondy; `now` — czas zapisu.
pub fn parse(json: &str, now: DateTime<Utc>) -> Result<Snapshot, String> {
    let v: Value = serde_json::from_str(json.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("odpowiedź sondy nie jest JSON: {e}"))?;
    let sys = &v["system"];
    let uptime_s = s(sys, "boot")
        .and_then(|b| DateTime::parse_from_rfc3339(&b).ok())
        .map_or(0, |b| {
            (now - b.with_timezone(&Utc)).num_seconds().max(0) as u64
        });
    let model = match (s(sys, "maker"), s(sys, "model")) {
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (a, b) => a.or(b),
    };
    let machine = MachineDiag {
        os: s(sys, "caption").unwrap_or_else(|| "Windows".into()),
        os_version: s(sys, "version").unwrap_or_default(),
        model,
        bios: s(sys, "bios"),
        uptime_s,
    };
    let c = &v["cpu"];
    let cores = u32_of(c, "cores").unwrap_or(0);
    let cpu = CpuDiag {
        name: s(c, "name").unwrap_or_default(),
        cores,
        threads: u32_of(c, "threads").unwrap_or(cores),
        base_mhz: u32_of(c, "max"),
        current_mhz: u32_of(c, "cur"),
        load_pct: u(c, "load").map(|l| l as f64),
        l2_kb: u32_of(c, "l2"),
        l3_kb: u32_of(c, "l3"),
        virtualization: c["virt"].as_bool(),
    };
    let pages = list(&v["pagefile"]);
    let sum = |k: &str| -> Option<u64> {
        (!pages.is_empty()).then(|| pages.iter().filter_map(|p| u(p, k)).sum())
    };
    let memory = MemDiag {
        total_mb: u(sys, "total_kb").unwrap_or(0) / 1024,
        available_mb: u(sys, "free_kb").unwrap_or(0) / 1024,
        pagefile_mb: sum("size"),
        pagefile_used_mb: sum("used"),
        modules: list(&v["modules"])
            .into_iter()
            .map(|m| MemModule {
                slot: s(m, "slot"),
                capacity_mb: u(m, "cap").unwrap_or(0) / MB,
                speed_mhz: u32_of(m, "speed").filter(|n| *n > 0),
                manufacturer: s(m, "maker"),
                part: s(m, "part"),
            })
            .collect(),
    };
    let mut disks: Vec<DiskDiag> = list(&v["disks"])
        .into_iter()
        .map(|d| {
            let bus = s(d, "bus");
            DiskDiag {
                number: u32_of(d, "number"),
                model: s(d, "model").unwrap_or_else(|| "Dysk".into()),
                serial_tail: s(d, "serial").map(|t| format!("…{t}")),
                media: media(&s(d, "media").unwrap_or_default()),
                removable: bus
                    .as_deref()
                    .is_some_and(|b| b.eq_ignore_ascii_case("usb")),
                bus,
                size_mb: u(d, "size").unwrap_or(0) / MB,
                health: health(&s(d, "health").unwrap_or_default()),
                operational: s(d, "op"),
                wear_pct: u32_of(d, "wear"),
                temperature_c: u32_of(d, "temp").filter(|t| *t > 0),
                power_on_hours: u(d, "hours"),
                read_errors: u(d, "rerr"),
                write_errors: u(d, "werr"),
                volumes: list(&d["letters"])
                    .into_iter()
                    .filter_map(|l| l.as_str().map(str::to_owned))
                    .collect(),
                event_errors: 0,
            }
        })
        .collect();
    let volumes = list(&v["volumes"])
        .into_iter()
        .filter_map(|x| {
            Some(VolumeDiag {
                letter: s(x, "letter")?.to_ascii_uppercase(),
                label: s(x, "label"),
                file_system: s(x, "fs"),
                size_mb: u(x, "size")? / MB,
                free_mb: u(x, "free").unwrap_or(0) / MB,
                system: x["system"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    let gpus = list(&v["gpus"])
        .into_iter()
        .map(|g| GpuDiag {
            name: s(g, "name").unwrap_or_default(),
            driver_version: s(g, "drv"),
            driver_date: s(g, "date"),
            vram_mb: u(g, "reg")
                .or_else(|| u(g, "ram").filter(|r| *r > 0 && *r < ADAPTER_RAM_CAP))
                .map(|b| b / MB),
            status: s(g, "status"),
        })
        .collect();
    let network = list(&v["net"])
        .into_iter()
        .map(|n| NetDiag {
            name: s(n, "name").unwrap_or_default(),
            kind: net_kind(
                &s(n, "media").unwrap_or_default(),
                &s(n, "desc").unwrap_or_default(),
            )
            .into(),
            description: s(n, "desc"),
            up: n["up"].as_bool().unwrap_or(false),
            link_mbps: u(n, "bps").filter(|b| *b > 0).map(|b| b / 1_000_000),
        })
        .collect();
    let battery = (!v["battery"].is_null()).then(|| {
        let b = &v["battery"];
        BatteryDiag {
            charge_pct: u32_of(b, "charge"),
            // Win32_Battery.BatteryStatus: 2 = zasilacz podłączony.
            on_ac: u(b, "status").map(|st| st == 2),
            design_mwh: u(b, "design").filter(|n| *n > 0),
            full_mwh: u(b, "full").filter(|n| *n > 0),
            cycles: u32_of(b, "cycles").filter(|n| *n > 0),
        }
    });
    let thermal = list(&v["thermal"])
        .into_iter()
        .filter_map(|t| {
            let c = t["c"].as_f64()?;
            // Czujniki bez odczytu podają 0 K (−273 °C) albo wartości poza zakresem.
            (c > 0.0 && c < 150.0).then(|| ThermalDiag {
                name: s(t, "name").unwrap_or_else(|| "Strefa".into()),
                celsius: c,
            })
        })
        .collect();
    let mut recent: Vec<EventLine> = list(&v["events"])
        .into_iter()
        .filter_map(|e| {
            let source = s(e, "provider")?;
            let id = u32_of(e, "id")?;
            Some(EventLine {
                at: s(e, "at").unwrap_or_default(),
                kind: classify(&source, id),
                source,
                id,
                message: s(e, "msg").unwrap_or_default(),
            })
        })
        .collect();
    recent.sort_by(|a, b| b.at.cmp(&a.at));
    for e in recent.iter().filter(|e| e.kind == EventKind::Disk) {
        if let Some(n) = disk_number(&e.message)
            && let Some(d) = disks.iter_mut().find(|d| d.number == Some(n))
        {
            d.event_errors += 1;
        }
    }
    let mut counts: Vec<EventCount> = Vec::new();
    for e in &recent {
        match counts.iter_mut().find(|c| c.kind == e.kind) {
            Some(c) => c.count += 1,
            None => counts.push(EventCount {
                kind: e.kind,
                count: 1,
            }),
        }
    }
    counts.sort_by_key(|c| c.kind);
    let recent = collapse(recent);
    let mut gaps: Vec<String> = list(&v["gaps"])
        .into_iter()
        .map(|g| {
            let part = s(g, "part").unwrap_or_default();
            let error = s(g, "error").unwrap_or_default();
            format!(
                "{} ({})",
                part_label(&part),
                error.chars().take(160).collect::<String>()
            )
        })
        .collect();
    let smart_empty = |d: &DiskDiag| {
        d.wear_pct.is_none() && d.power_on_hours.is_none() && d.read_errors.is_none()
    };
    if !disks.is_empty() && disks.iter().all(smart_empty) {
        gaps.push(
            "liczniki SMART dysków (zużycie, godziny pracy, błędy) — system podaje je tylko programom uruchomionym jako administrator; kondycja dysków jest odczytana"
                .into(),
        );
    }
    Ok(Snapshot {
        at: now.to_rfc3339(),
        machine,
        cpu,
        memory,
        disks,
        volumes,
        gpus,
        network,
        battery,
        thermal,
        events: EventSummary {
            window_days: EVENT_WINDOW_DAYS,
            counts,
            recent,
        },
        gaps,
    })
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
