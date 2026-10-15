//! Testy procesora i pamięci. Procesor: stała porcja obliczeń całkowitoliczbowych (mieszanie
//! 64-bitowe) przez ustalony czas — wynik w mln operacji/s, jeden wątek i wszystkie wątki
//! (skalowanie). Pamięć: bufor do 512 MB (najwyżej ¼ wolnej pamięci) wypełniony wzorcem
//! pseudolosowym i jego negacją z pełną weryfikacją (błędy = uszkodzona pamięć) oraz
//! przepustowość kopiowania.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pc_diag_contract::{CheckKind, CheckResult, Metric, Verdict};

/// Czas jednego pomiaru procesora.
pub const CPU_WINDOW: Duration = Duration::from_millis(1_500);
/// Największy bufor testu pamięci (MB).
pub const MEMORY_MAX_MB: u64 = 512;
/// Najmniejszy sensowny bufor (MB).
pub const MEMORY_MIN_MB: u64 = 64;

/// Jedna porcja pracy: 4096 kroków mieszania (splitmix64).
fn chunk(mut x: u64) -> u64 {
    for _ in 0..4096 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= z ^ (z >> 31);
    }
    x
}

/// Porcje wykonane w oknie czasu (przerwane przez `cancel`).
fn spin(window: Duration, seed: u64, cancel: &AtomicBool) -> u64 {
    let start = Instant::now();
    let (mut n, mut x) = (0u64, seed);
    while start.elapsed() < window && !cancel.load(Ordering::Relaxed) {
        x = black_box(chunk(x));
        n += 1;
    }
    n
}

fn mops(chunks: u64, window: Duration) -> f64 {
    (chunks * 4096) as f64 / window.as_secs_f64() / 1e6
}

/// Procesor, jeden wątek.
pub fn cpu_single(window: Duration, cancel: &AtomicBool) -> CheckResult {
    let start = Instant::now();
    let n = spin(window, 1, cancel);
    let score = mops(n, start.elapsed());
    CheckResult {
        kind: CheckKind::CpuSingle,
        verdict: Verdict::Pass,
        metrics: vec![Metric::new("mops", "Wynik (jeden wątek)", score, "mln op/s", true)],
        notes: vec!["Porównywany z poprzednimi pomiarami — spadek oznacza zwykle przegrzewanie albo tryb oszczędzania energii.".into()],
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Procesor, wszystkie wątki; `single` — wynik jednego wątku do skalowania.
pub fn cpu_multi(window: Duration, single: Option<f64>, cancel: &AtomicBool) -> CheckResult {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let start = Instant::now();
    let total: u64 = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|i| s.spawn(move || spin(window, i as u64 + 7, cancel)))
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).sum()
    });
    let score = mops(total, start.elapsed());
    let mut metrics = vec![
        Metric::new("mops", "Wynik (wszystkie wątki)", score, "mln op/s", true),
        Metric::new("threads", "Wątki", threads as f64, "szt.", true),
    ];
    let mut notes = Vec::new();
    let mut verdict = Verdict::Pass;
    if let Some(one) = single.filter(|s| *s > 0.0) {
        let scaling = score / one;
        metrics.push(Metric::new("scaling", "Skalowanie", scaling, "×", true));
        // Rdzenie wydajne i energooszczędne oraz SMT: oczekiwane ~0,4–0,7 × liczba wątków.
        if scaling < threads as f64 * 0.25 {
            verdict = Verdict::Warn;
            notes.push(format!(
                "Wszystkie wątki dają tylko {scaling:.1}× wyniku jednego — procesor może być dławiony (temperatura, zasilanie) albo zajęty innymi programami."
            ));
        }
    }
    CheckResult {
        kind: CheckKind::CpuMulti,
        verdict,
        metrics,
        notes,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Rozmiar bufora pamięci (MB) dla wolnej pamięci.
pub fn memory_size_mb(available_mb: u64) -> Option<u64> {
    let size = (available_mb / 4).min(MEMORY_MAX_MB);
    (size >= MEMORY_MIN_MB).then_some(size)
}

/// Wzorzec dla indeksu (xorshift z indeksu — każdy element inny).
fn pattern(i: u64) -> u64 {
    let mut x = i.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xA5A5_A5A5_5A5A_5A5A;
    x ^= x << 13;
    x ^= x >> 7;
    x ^ (x << 17)
}

/// Pamięć: wzorzec + negacja z weryfikacją, przepustowość kopiowania.
pub fn memory(size_mb: u64, cancel: &AtomicBool) -> CheckResult {
    let start = Instant::now();
    let words = (size_mb * 1024 * 1024 / 8) as usize;
    let mut buf: Vec<u64> = Vec::new();
    if buf.try_reserve_exact(words).is_err() {
        return CheckResult::skipped(
            CheckKind::Memory,
            format!("Nie udało się zarezerwować {size_mb} MB pamięci."),
        );
    }
    buf.resize(words, 0);
    let mut errors = 0u64;
    for invert in [false, true] {
        for (i, w) in buf.iter_mut().enumerate() {
            let p = pattern(i as u64);
            *w = if invert { !p } else { p };
        }
        if cancel.load(Ordering::Relaxed) {
            return CheckResult::skipped(CheckKind::Memory, "Przerwano.");
        }
        errors += buf
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                let p = pattern(*i as u64);
                **w != if invert { !p } else { p }
            })
            .count() as u64;
    }
    let mut copy = vec![0u64; words];
    let t = Instant::now();
    let passes = 3;
    for _ in 0..passes {
        copy.copy_from_slice(black_box(&buf));
        black_box(&copy);
    }
    let gbps = (size_mb as f64 / 1024.0) * passes as f64 / t.elapsed().as_secs_f64();
    let mut r = CheckResult {
        kind: CheckKind::Memory,
        verdict: Verdict::Pass,
        metrics: vec![
            Metric::new("tested_mb", "Sprawdzono", size_mb as f64, "MB", true),
            Metric::new("errors", "Błędy wzorca", errors as f64, "szt.", false),
            Metric::new("copy_gbps", "Kopiowanie", gbps, "GB/s", true),
        ],
        notes: Vec::new(),
        duration_ms: start.elapsed().as_millis() as u64,
    };
    if errors > 0 {
        r.verdict = Verdict::Fail;
        r.notes.push(format!("{errors} słów pamięci nie zgadza się z zapisanym wzorcem — uruchom „Diagnostykę pamięci systemu Windows” i rozważ wymianę modułu."));
    } else if gbps < 2.0 {
        r.verdict = Verdict::Warn;
        r.notes.push(format!(
            "Kopiowanie {gbps:.1} GB/s — wolno; możliwy tryb jednokanałowy albo obciążenie."
        ));
    } else {
        r.notes.push(
            "Wzorzec zapisany i odczytany bez błędów (test części wolnej pamięci, nie całej)."
                .into(),
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_and_memory_tests_run() {
        let cancel = AtomicBool::new(false);
        let one = cpu_single(Duration::from_millis(100), &cancel);
        let s = one.metric("mops").unwrap();
        assert!(s > 0.0);
        let many = cpu_multi(Duration::from_millis(100), Some(s), &cancel);
        assert!(many.metric("scaling").is_some());
        let mem = memory(MEMORY_MIN_MB, &cancel);
        assert_eq!(mem.verdict, Verdict::Pass, "{:?}", mem.notes);
        assert_eq!(mem.metric("errors"), Some(0.0));
        assert_eq!(memory_size_mb(1000), Some(250));
        assert_eq!(memory_size_mb(100), None);
        assert_eq!(memory_size_mb(64_000), Some(MEMORY_MAX_MB));
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(memory(MEMORY_MIN_MB, &cancel).verdict, Verdict::Skipped);
    }
}
