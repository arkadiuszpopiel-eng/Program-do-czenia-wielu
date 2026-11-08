//! Reguły oceny (czyste funkcje — te same w `-impl` i `-fake`): progi dysków według rodzaju
//! nośnika, ocena spójności z zapisu parametrów (kondycja dysków, miejsce, zdarzenia awarii,
//! bateria, temperatury) i porównanie z poprzednim wynikiem tego samego testu (spadek
//! wydajności = ostrzeżenie, np. przegrzewanie, zużycie dysku, zły kabel USB).

use crate::checks::{CheckKind, CheckResult, Metric, Verdict};
use crate::snapshot::{EventKind, Health, MediaKind, Snapshot};

/// Spadek wartości „większa lepsza” względem poprzedniego pomiaru, od którego jest ostrzeżenie.
pub const DROP_WARN: f64 = 0.30;
/// Wzrost wartości „mniejsza lepsza”, od którego jest ostrzeżenie.
pub const RISE_WARN: f64 = 0.50;

/// Rodzaj dysku do progów: NVMe, SSD SATA, HDD, USB / nieznany.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskClass {
    /// SSD na NVMe.
    Nvme,
    /// SSD na SATA.
    Ssd,
    /// Dysk talerzowy.
    Hdd,
    /// USB, wymienny albo nieznany.
    External,
}

impl DiskClass {
    /// Z rodzaju nośnika, magistrali i wymienności.
    pub fn of(media: MediaKind, bus: Option<&str>, removable: bool) -> Self {
        let bus = bus.unwrap_or_default().to_ascii_lowercase();
        if removable || bus.contains("usb") {
            return Self::External;
        }
        match media {
            _ if bus.contains("nvme") => Self::Nvme,
            MediaKind::Ssd => Self::Ssd,
            MediaKind::Hdd => Self::Hdd,
            MediaKind::Unknown => Self::External,
        }
    }

    /// Progi (ostrzeżenie, błąd): zapis sekwencyjny MB/s (poniżej) i opóźnienie odczytu 4 KB ms
    /// (powyżej).
    pub fn thresholds(self) -> ((f64, f64), (f64, f64)) {
        match self {
            Self::Nvme => ((300.0, 100.0), (2.0, 10.0)),
            Self::Ssd => ((150.0, 50.0), (3.0, 15.0)),
            Self::Hdd => ((40.0, 15.0), (30.0, 80.0)),
            Self::External => ((20.0, 5.0), (40.0, 150.0)),
        }
    }
}

fn worse(v: &mut Verdict, to: Verdict) {
    if to > *v && to != Verdict::Skipped {
        *v = to;
    }
}

/// Ocena testu dysku z progów rodzaju dysku.
pub fn rate_disk(result: &mut CheckResult, class: DiskClass) {
    let ((w_warn, w_fail), (l_warn, l_fail)) = class.thresholds();
    if let Some(w) = result.metric("seq_write_mbps") {
        if w < w_fail {
            worse(&mut result.verdict, Verdict::Fail);
            result.notes.push(format!(
                "Zapis {w:.0} MB/s — bardzo wolno jak na ten rodzaj dysku."
            ));
        } else if w < w_warn {
            worse(&mut result.verdict, Verdict::Warn);
            result.notes.push(format!(
                "Zapis {w:.0} MB/s — wolniej niż typowo (próg {w_warn:.0})."
            ));
        }
    }
    if let Some(l) = result.metric("rand_read_ms") {
        if l > l_fail {
            worse(&mut result.verdict, Verdict::Fail);
            result.notes.push(format!(
                "Odczyt losowy {l:.1} ms — dysk odpowiada bardzo wolno."
            ));
        } else if l > l_warn {
            worse(&mut result.verdict, Verdict::Warn);
            result
                .notes
                .push(format!("Odczyt losowy {l:.1} ms — wolniej niż typowo."));
        }
    }
    if result.metric("verify_errors").is_some_and(|e| e > 0.0) {
        worse(&mut result.verdict, Verdict::Fail);
        result
            .notes
            .push("Odczytane dane różnią się od zapisanych — możliwe uszkodzenie nośnika albo połączenia.".into());
    }
}

/// Porównanie z poprzednim wynikiem tego samego testu (dopisuje uwagi, pogarsza ocenę).
pub fn compare(result: &mut CheckResult, previous: &CheckResult) {
    if previous.kind != result.kind || previous.verdict == Verdict::Skipped {
        return;
    }
    let mut notes = Vec::new();
    for m in &result.metrics {
        let Some(before) = previous.metric(&m.key) else {
            continue;
        };
        if before <= 0.0 {
            continue;
        }
        let change = (m.value - before) / before;
        if m.higher_is_better && change <= -DROP_WARN {
            notes.push(format!(
                "{}: {:.1} {} — spadek o {:.0}% od poprzedniego pomiaru ({:.1}).",
                m.label,
                m.value,
                m.unit,
                -change * 100.0,
                before
            ));
        } else if !m.higher_is_better && change >= RISE_WARN && is_meaningful(m, before) {
            notes.push(format!(
                "{}: {:.1} {} — wzrost o {:.0}% od poprzedniego pomiaru ({:.1}).",
                m.label,
                m.value,
                m.unit,
                change * 100.0,
                before
            ));
        }
    }
    if !notes.is_empty() {
        worse(&mut result.verdict, Verdict::Warn);
        result.notes.extend(notes);
    }
}

/// Bardzo małe opóźnienia (ułamki milisekundy) skaczą procentowo bez znaczenia.
fn is_meaningful(m: &Metric, before: f64) -> bool {
    !(m.unit == "ms" && m.value < 1.0 && before < 1.0)
}

/// Spójność: ocena ostatniego zapisu parametrów (bez obciążania sprzętu).
pub fn integrity(s: &Snapshot) -> CheckResult {
    let mut r = CheckResult {
        kind: CheckKind::Integrity,
        verdict: Verdict::Pass,
        metrics: Vec::new(),
        notes: Vec::new(),
        duration_ms: 0,
    };
    let flag = |r: &mut CheckResult, v: Verdict, note: String| {
        worse(&mut r.verdict, v);
        r.notes.push(note);
    };
    for d in &s.disks {
        match d.health {
            Health::Unhealthy => flag(
                &mut r,
                Verdict::Fail,
                format!(
                    "Dysk {}: system zgłasza awarię — zrób kopię danych.",
                    d.model
                ),
            ),
            Health::Warning => flag(
                &mut r,
                Verdict::Warn,
                format!("Dysk {}: system zgłasza ostrzeżenie kondycji.", d.model),
            ),
            _ => {}
        }
        if let Some(w) = d.wear_pct {
            if w >= 90 {
                flag(
                    &mut r,
                    Verdict::Fail,
                    format!("Dysk {}: zużycie {w}% — bliski końca żywotności.", d.model),
                );
            } else if w >= 70 {
                flag(
                    &mut r,
                    Verdict::Warn,
                    format!("Dysk {}: zużycie {w}%.", d.model),
                );
            }
        }
        if let Some(t) = d.temperature_c.filter(|t| *t >= 70) {
            flag(
                &mut r,
                Verdict::Warn,
                format!("Dysk {}: temperatura {t} °C.", d.model),
            );
        }
        let errors = d.read_errors.unwrap_or(0) + d.write_errors.unwrap_or(0);
        if errors > 0 {
            flag(
                &mut r,
                Verdict::Warn,
                format!(
                    "Dysk {}: {errors} błędów odczytu/zapisu w liczniku SMART.",
                    d.model
                ),
            );
        }
    }
    for v in &s.volumes {
        let pct = v.free_pct();
        if pct < 5.0 || (v.system && v.free_mb < 5 * 1024) {
            flag(
                &mut r,
                Verdict::Fail,
                format!("{} — wolne tylko {:.1}% ({} MB).", v.letter, pct, v.free_mb),
            );
        } else if pct < 10.0 {
            flag(
                &mut r,
                Verdict::Warn,
                format!("{} — wolne {:.1}% miejsca.", v.letter, pct),
            );
        }
    }
    let ev = &s.events;
    let rules: [(EventKind, u32, u32, &str); 5] = [
        (EventKind::Bugcheck, 1, 3, "niebieskich ekranów"),
        (
            EventKind::UnexpectedShutdown,
            1,
            4,
            "nieoczekiwanych wyłączeń",
        ),
        (EventKind::Hardware, 1, 5, "błędów sprzętowych (WHEA)"),
        (EventKind::Disk, 1, 10, "błędów dysku lub systemu plików"),
        (EventKind::Usb, 3, 15, "odłączeń/resetów pamięci USB"),
    ];
    for (kind, warn, fail, what) in rules {
        let n = ev.count(kind);
        if n >= fail {
            flag(
                &mut r,
                Verdict::Fail,
                format!("{n} {what} w ostatnich {} dniach.", ev.window_days),
            );
        } else if n >= warn {
            flag(
                &mut r,
                Verdict::Warn,
                format!("{n} {what} w ostatnich {} dniach.", ev.window_days),
            );
        }
    }
    let mut codes: Vec<u32> = ev
        .recent
        .iter()
        .filter(|e| e.kind == EventKind::Bugcheck)
        .filter_map(|e| crate::bugcheck::code_in(&e.message))
        .collect();
    codes.sort_unstable();
    codes.dedup();
    for code in codes {
        r.notes.push(format!(
            "Niebieski ekran {}.",
            crate::bugcheck::describe(code)
        ));
    }
    for d in s.disks.iter().filter(|d| d.event_errors > 0) {
        let place = if d.volumes.is_empty() {
            String::new()
        } else {
            format!(" ({})", d.volumes.join(", "))
        };
        let v = if d.event_errors >= 10 {
            Verdict::Fail
        } else {
            Verdict::Warn
        };
        flag(
            &mut r,
            v,
            format!(
                "Dysk {}{place}: {} błędów w dzienniku — sprawdź kabel, mostek USB i kopię danych.",
                d.model, d.event_errors
            ),
        );
    }
    if let Some(w) = s.battery.as_ref().and_then(|b| b.wear_pct()) {
        if w >= 60.0 {
            flag(
                &mut r,
                Verdict::Fail,
                format!("Bateria straciła {w:.0}% pojemności."),
            );
        } else if w >= 40.0 {
            flag(
                &mut r,
                Verdict::Warn,
                format!("Bateria straciła {w:.0}% pojemności."),
            );
        }
    }
    for t in &s.thermal {
        if t.celsius >= 90.0 {
            flag(
                &mut r,
                Verdict::Fail,
                format!("{}: {:.0} °C — przegrzewanie.", t.name, t.celsius),
            );
        } else if t.celsius >= 80.0 {
            flag(
                &mut r,
                Verdict::Warn,
                format!("{}: {:.0} °C.", t.name, t.celsius),
            );
        }
    }
    if s.memory.used_pct() >= 95.0 {
        flag(
            &mut r,
            Verdict::Warn,
            format!("Pamięć zajęta w {:.0}%.", s.memory.used_pct()),
        );
    }
    for g in &s.gpus {
        if let Some(st) = g
            .status
            .as_deref()
            .filter(|st| !st.eq_ignore_ascii_case("ok"))
        {
            flag(
                &mut r,
                Verdict::Warn,
                format!("Karta {}: stan „{st}”.", g.name),
            );
        }
    }
    for gap in &s.gaps {
        r.notes.push(format!("Nie odczytano: {gap}."));
    }
    r.metrics = vec![
        Metric::new(
            "disk_events",
            "Błędy dysku (dni)",
            f64::from(ev.count(EventKind::Disk)),
            "szt.",
            false,
        ),
        Metric::new(
            "crashes",
            "Niebieskie ekrany i wyłączenia",
            f64::from(ev.count(EventKind::Bugcheck) + ev.count(EventKind::UnexpectedShutdown)),
            "szt.",
            false,
        ),
        Metric::new(
            "memory_used_pct",
            "Zajętość pamięci",
            s.memory.used_pct(),
            "%",
            false,
        ),
    ];
    if r.notes.is_empty() {
        r.notes.push("Bez zastrzeżeń.".into());
    }
    r
}

#[cfg(test)]
#[path = "rating_tests.rs"]
mod tests;
