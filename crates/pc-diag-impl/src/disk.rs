//! Test dysku na pliku tymczasowym Alfy (`.alfa-diag-<pid>.tmp` w katalogu testu woluminu),
//! usuwanym zawsze — także po błędzie i przerwaniu. Na Windows zapis i odczyt omijają pamięć
//! podręczną systemu (`FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`, bufory wyrównane do
//! 4 KB), więc wynik mierzy nośnik, nie RAM. Każdy blok ma sumę kontrolną — różnica przy
//! odczycie oznacza uszkodzone dane (nośnik, kabel, mostek USB).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pc_diag_contract::{CheckKind, CheckResult, Metric, Verdict};

/// Rozmiar bloku (wielokrotność sektora 4 KB).
const BLOCK: usize = 4 * 1024 * 1024;
/// Wyrównanie buforów.
const ALIGN: usize = 4096;
/// Liczba odczytów losowych 4 KB.
const RANDOM_READS: usize = 400;

/// Parametry testu dysku.
#[derive(Debug, Clone)]
pub struct DiskPlan {
    /// Litera woluminu (`C:`).
    pub letter: String,
    /// Katalog na plik testowy (istniejący albo tworzony; usuwany, gdy pusty po teście).
    pub dir: PathBuf,
    /// Rozmiar pliku (MB, wielokrotność 4).
    pub size_mb: u64,
    /// Limit czasu całego testu.
    pub limit: Duration,
}

/// Bufor wyrównany do 4 KB (wycinek większego wektora).
struct Aligned {
    raw: Vec<u8>,
    off: usize,
    len: usize,
}

impl Aligned {
    fn new(len: usize) -> Self {
        let raw = vec![0u8; len + ALIGN];
        let off = (ALIGN - (raw.as_ptr() as usize % ALIGN)) % ALIGN;
        Self { raw, off, len }
    }
    fn bytes(&mut self) -> &mut [u8] {
        &mut self.raw[self.off..self.off + self.len]
    }
}

fn open(path: &Path, write: bool) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    if write {
        o.write(true).create_new(true);
    } else {
        o.read(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH
        o.custom_flags(0x2000_0000 | 0x8000_0000);
    }
    o.open(path)
}

/// Treść bloku `n` (deterministyczna) i jej suma.
fn fill(buf: &mut [u8], n: u64) -> u64 {
    let mut x = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for chunk in buf.chunks_exact_mut(8) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        chunk.copy_from_slice(&x.to_le_bytes());
    }
    checksum(buf)
}

fn checksum(buf: &[u8]) -> u64 {
    buf.chunks_exact(8).fold(0xcbf2_9ce4_8422_2325u64, |h, c| {
        let mut w = [0u8; 8];
        w.copy_from_slice(c);
        (h ^ u64::from_le_bytes(w)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Usuwa plik i (pusty) katalog testu przy wyjściu.
struct Cleanup<'a> {
    file: PathBuf,
    dir: &'a Path,
    created_dir: bool,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        if self.created_dir {
            let _ = std::fs::remove_dir(self.dir);
        }
    }
}

fn mbps(bytes: u64, t: Duration) -> f64 {
    bytes as f64 / (1024.0 * 1024.0) / t.as_secs_f64().max(1e-6)
}

/// Test dysku: zapis sekwencyjny, odczyt z weryfikacją, odczyt losowy 4 KB.
pub fn disk(plan: &DiskPlan, cancel: &AtomicBool) -> CheckResult {
    let kind = CheckKind::Disk(plan.letter.clone());
    let start = Instant::now();
    let created_dir = !plan.dir.exists();
    if let Err(e) = std::fs::create_dir_all(&plan.dir) {
        return CheckResult::skipped(kind, format!("Nie można utworzyć katalogu testu: {e}."));
    }
    let file = plan
        .dir
        .join(format!(".alfa-diag-{}.tmp", std::process::id()));
    let _cleanup = Cleanup {
        file: file.clone(),
        dir: &plan.dir,
        created_dir,
    };
    let blocks = (plan.size_mb * 1024 * 1024) as usize / BLOCK;
    let mut buf = Aligned::new(BLOCK);
    let mut sums = Vec::with_capacity(blocks);
    let mut notes = Vec::new();
    // Zapis.
    let t = Instant::now();
    let mut written = 0u64;
    let write = (|| -> std::io::Result<()> {
        let mut f = open(&file, true)?;
        for n in 0..blocks {
            if cancel.load(Ordering::Relaxed) || start.elapsed() > plan.limit {
                break;
            }
            sums.push(fill(buf.bytes(), n as u64));
            f.write_all(buf.bytes())?;
            written += BLOCK as u64;
        }
        f.sync_all()
    })();
    let write_time = t.elapsed();
    if let Err(e) = write {
        return CheckResult {
            kind,
            verdict: Verdict::Fail,
            metrics: Vec::new(),
            notes: vec![format!("Zapis pliku testowego nie powiódł się: {e}.")],
            duration_ms: start.elapsed().as_millis() as u64,
        };
    }
    if written < (blocks * BLOCK) as u64 {
        notes.push(format!(
            "Zapis przerwany po {} MB (limit czasu {} s) — dysk bardzo wolny albo zawiesza się.",
            written / 1024 / 1024,
            plan.limit.as_secs()
        ));
    }
    // Odczyt z weryfikacją.
    let t = Instant::now();
    let mut read = 0u64;
    let mut bad = 0u64;
    let read_result = (|| -> std::io::Result<()> {
        let mut f = open(&file, false)?;
        for expected in &sums {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            f.read_exact(buf.bytes())?;
            if checksum(buf.bytes()) != *expected {
                bad += 1;
            }
            read += BLOCK as u64;
        }
        Ok(())
    })();
    let read_time = t.elapsed();
    // Odczyt losowy 4 KB.
    let mut small = Aligned::new(ALIGN);
    let mut latencies = Vec::with_capacity(RANDOM_READS);
    let random = (|| -> std::io::Result<()> {
        let mut f = open(&file, false)?;
        let pages = (written / ALIGN as u64).max(1);
        let mut x = 0x1234_5678_9ABC_DEF1u64;
        for _ in 0..RANDOM_READS {
            if cancel.load(Ordering::Relaxed) || start.elapsed() > plan.limit * 2 {
                break;
            }
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let at = (x % pages) * ALIGN as u64;
            let t = Instant::now();
            f.seek(SeekFrom::Start(at))?;
            f.read_exact(small.bytes())?;
            latencies.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        Ok(())
    })();
    let mut metrics = vec![
        Metric::new(
            "seq_write_mbps",
            "Zapis sekwencyjny",
            mbps(written, write_time),
            "MB/s",
            true,
        ),
        Metric::new(
            "seq_read_mbps",
            "Odczyt sekwencyjny",
            mbps(read, read_time),
            "MB/s",
            true,
        ),
        Metric::new(
            "verify_errors",
            "Bloki z błędną sumą",
            bad as f64,
            "szt.",
            false,
        ),
        Metric::new(
            "tested_mb",
            "Rozmiar próby",
            (written / 1024 / 1024) as f64,
            "MB",
            true,
        ),
    ];
    if !latencies.is_empty() {
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        latencies.sort_by(f64::total_cmp);
        let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
        metrics.push(Metric::new(
            "rand_read_ms",
            "Odczyt losowy 4 KB (średnio)",
            avg,
            "ms",
            false,
        ));
        metrics.push(Metric::new(
            "rand_read_p99_ms",
            "Odczyt losowy 4 KB (99%)",
            p99,
            "ms",
            false,
        ));
        metrics.push(Metric::new(
            "iops",
            "Operacje 4 KB na sekundę",
            1000.0 / avg.max(1e-6),
            "IOPS",
            true,
        ));
    }
    let mut verdict = Verdict::Pass;
    for (what, r) in [("Odczyt", read_result), ("Odczyt losowy", random)] {
        if let Err(e) = r {
            verdict = Verdict::Fail;
            notes.push(format!("{what} pliku testowego nie powiódł się: {e}."));
        }
    }
    if !cfg!(windows) {
        notes.push("Poza Windows odczyt może korzystać z pamięci podręcznej systemu.".into());
    }
    CheckResult {
        kind,
        verdict,
        metrics,
        notes,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_test_measures_verifies_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("diag");
        let plan = DiskPlan {
            letter: "T:".into(),
            dir: sub.clone(),
            size_mb: 16,
            limit: Duration::from_secs(60),
        };
        let r = disk(&plan, &AtomicBool::new(false));
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.notes);
        assert_eq!(r.metric("verify_errors"), Some(0.0));
        assert_eq!(r.metric("tested_mb"), Some(16.0));
        assert!(r.metric("seq_write_mbps").unwrap() > 0.0);
        assert!(r.metric("rand_read_ms").is_some());
        assert!(!sub.exists(), "plik i pusty katalog usunięte");
    }

    #[test]
    fn checksum_detects_change() {
        let mut a = vec![0u8; 4096];
        let s = fill(&mut a, 3);
        assert_eq!(checksum(&a), s);
        a[100] ^= 1;
        assert_ne!(checksum(&a), s);
    }
}
