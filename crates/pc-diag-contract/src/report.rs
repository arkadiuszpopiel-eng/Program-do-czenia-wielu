//! Raport diagnostyki w Markdown (zapis do pliku, wklejenie serwisantowi, odpowiedź agentki).
//! Bez danych osobistych: brak adresów IP/MAC, pełnych numerów seryjnych i ścieżek użytkownika.

use std::fmt::Write as _;

use crate::checks::{CheckRun, Verdict};
use crate::snapshot::{Health, MediaKind, Snapshot};

fn gb(mb: u64) -> String {
    format!("{:.1} GB", mb as f64 / 1024.0)
}

fn opt<T: std::fmt::Display>(v: Option<T>, unit: &str) -> String {
    v.map_or_else(|| "—".to_owned(), |v| format!("{v}{unit}"))
}

/// Słowo oceny.
pub fn verdict_word(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "OK",
        Verdict::Warn => "UWAGA",
        Verdict::Fail => "PROBLEM",
        Verdict::Skipped => "pominięto",
    }
}

fn health_word(h: Health) -> &'static str {
    match h {
        Health::Healthy => "sprawny",
        Health::Warning => "ostrzeżenie",
        Health::Unhealthy => "awaria",
        Health::Unknown => "nieznany",
    }
}

/// Raport: parametry i (opcjonalnie) ostatnie testy.
pub fn markdown(s: &Snapshot, run: Option<&CheckRun>) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Diagnostyka komputera\n\nZapis parametrów: {}\n", s.at);
    let m = &s.machine;
    let _ = writeln!(
        o,
        "## System\n\n- {} ({})\n- Komputer: {}\n- BIOS/UEFI: {}\n- Czas pracy: {:.1} h\n",
        m.os,
        m.os_version,
        opt(m.model.as_deref(), ""),
        opt(m.bios.as_deref(), ""),
        m.uptime_s as f64 / 3600.0
    );
    let c = &s.cpu;
    let _ = writeln!(
        o,
        "## Procesor\n\n- {} — {} rdzeni / {} wątków\n- Taktowanie: bazowe {}, bieżące {}\n- Obciążenie: {}\n- Pamięć podręczna L2/L3: {} / {}\n",
        c.name,
        c.cores,
        c.threads,
        opt(c.base_mhz, " MHz"),
        opt(c.current_mhz, " MHz"),
        opt(c.load_pct.map(|l| format!("{l:.0}")), " %"),
        opt(c.l2_kb, " KB"),
        opt(c.l3_kb, " KB")
    );
    let mem = &s.memory;
    let _ = writeln!(
        o,
        "## Pamięć\n\n- {} łącznie, wolne {} (zajęte {:.0}%)\n- Plik stronicowania: {} (użyte {})",
        gb(mem.total_mb),
        gb(mem.available_mb),
        mem.used_pct(),
        opt(mem.pagefile_mb.map(gb), ""),
        opt(mem.pagefile_used_mb.map(gb), "")
    );
    for mm in &mem.modules {
        let _ = writeln!(
            o,
            "- Moduł {}: {} {} {} {}",
            opt(mm.slot.as_deref(), ""),
            gb(mm.capacity_mb),
            opt(mm.speed_mhz, " MHz"),
            opt(mm.manufacturer.as_deref(), ""),
            opt(mm.part.as_deref(), "")
        );
    }
    let _ = writeln!(o, "\n## Dyski\n");
    for d in &s.disks {
        let media = match d.media {
            MediaKind::Ssd => "SSD",
            MediaKind::Hdd => "HDD",
            MediaKind::Unknown => "nośnik nieznany",
        };
        let _ = writeln!(
            o,
            "- **{}** ({media}, {}, {}) — kondycja: {}; zużycie {}; temperatura {}; godziny pracy {}; błędy odczytu/zapisu {}/{}; woluminy: {}",
            d.model,
            opt(d.bus.as_deref(), ""),
            gb(d.size_mb),
            health_word(d.health),
            opt(d.wear_pct, " %"),
            opt(d.temperature_c, " °C"),
            opt(d.power_on_hours, " h"),
            opt(d.read_errors, ""),
            opt(d.write_errors, ""),
            if d.volumes.is_empty() {
                "—".into()
            } else {
                d.volumes.join(", ")
            }
        );
    }
    for v in &s.volumes {
        let _ = writeln!(
            o,
            "- {} {} ({}) — wolne {} z {} ({:.1}%)",
            v.letter,
            opt(v.label.as_deref(), ""),
            opt(v.file_system.as_deref(), ""),
            gb(v.free_mb),
            gb(v.size_mb),
            v.free_pct()
        );
    }
    let _ = writeln!(o, "\n## Karty graficzne\n");
    for g in &s.gpus {
        let _ = writeln!(
            o,
            "- {} — pamięć {}, sterownik {} ({}), stan {}",
            g.name,
            opt(g.vram_mb.map(gb), ""),
            opt(g.driver_version.as_deref(), ""),
            opt(g.driver_date.as_deref(), ""),
            opt(g.status.as_deref(), "")
        );
    }
    let _ = writeln!(o, "\n## Sieć\n");
    for n in &s.network {
        let _ = writeln!(
            o,
            "- {} ({}) — {}, {}",
            n.name,
            n.kind,
            if n.up { "połączona" } else { "rozłączona" },
            opt(n.link_mbps, " Mb/s")
        );
    }
    if let Some(b) = &s.battery {
        let _ = writeln!(
            o,
            "\n## Bateria\n\n- Naładowanie {}, zasilacz {}, zużycie {}, cykle {}",
            opt(b.charge_pct, " %"),
            opt(
                b.on_ac.map(|a| if a { "podłączony" } else { "odłączony" }),
                ""
            ),
            opt(b.wear_pct().map(|w| format!("{w:.0}")), " %"),
            opt(b.cycles, "")
        );
    }
    if !s.thermal.is_empty() {
        let _ = writeln!(o, "\n## Temperatury\n");
        for t in &s.thermal {
            let _ = writeln!(o, "- {}: {:.0} °C", t.name, t.celsius);
        }
    }
    let _ = writeln!(o, "\n## Zdarzenia ({} dni)\n", s.events.window_days);
    for (k, n) in &s.events.counts {
        let _ = writeln!(o, "- {}: {n}", k.label());
    }
    for e in s.events.recent.iter().take(15) {
        let _ = writeln!(o, "- {} {} ({}): {}", e.at, e.source, e.id, e.message);
    }
    if let Some(run) = run {
        let _ = writeln!(
            o,
            "\n## Testy ({}) — wynik: {}\n",
            run.at,
            verdict_word(run.overall)
        );
        for r in &run.results {
            let metrics: Vec<String> = r
                .metrics
                .iter()
                .map(|m| format!("{} {:.1} {}", m.label, m.value, m.unit))
                .collect();
            let _ = writeln!(
                o,
                "- **{}** — {} ({} ms): {}. {}",
                r.kind.key(),
                verdict_word(r.verdict),
                r.duration_ms,
                metrics.join("; "),
                r.notes.join(" ")
            );
        }
    }
    if !s.gaps.is_empty() {
        let _ = writeln!(o, "\n## Nie odczytano\n");
        for g in &s.gaps {
            let _ = writeln!(o, "- {g}");
        }
    }
    o
}
