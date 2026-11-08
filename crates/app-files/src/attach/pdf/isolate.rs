//! Ekstrakcja w osobnym wątku z terminem i pamięć podręczna wyników (po SHA-256 i limicie znaków).
//! Projekcja historii pyta o te same pliki w każdej turze, więc trzymamy więcej wyników niż
//! załączników w typowej rozmowie. Przekroczenie czasu (może wynikać z chwilowego obciążenia)
//! pamiętamy krótko — kolejna tura nie płaci znowu pełnego terminu, a później jest nowa próba.
//! „Zajęte” (za dużo działających wątków) nie trafia do pamięci.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use super::super::extract::Extracted;

/// Ile wyników trzymamy w pamięci podręcznej.
const CACHE_ENTRIES: usize = 64;
/// Jak długo pamiętamy przekroczenie czasu.
pub(super) const TIMEOUT_TTL: Duration = Duration::from_secs(300);
/// Najwięcej jednoczesnych wątków ekstrakcji (także tych, które przekroczyły termin).
const MAX_RUNNING: usize = 3;
/// Ile dłużej od terminu ekstrakcji czekamy na wątek.
const GRACE: Duration = Duration::from_secs(1);
/// Stos wątku ekstrakcji.
const STACK: usize = 16 * 1024 * 1024;

pub(super) const BUSY: &str = "Wyciąganie tekstu z PDF jest chwilowo niedostępne (trwają inne ekstrakcje) — spróbuj \
     ponownie za chwilę.";
const CRASHED: &str = "PDF jest uszkodzony albo niepełny — nie udało się odczytać jego struktury.";

/// Czy i jak długo pamiętać wynik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Keep {
    /// Do wyparcia przez nowsze wpisy.
    Always,
    /// Przez `TIMEOUT_TTL`.
    Briefly,
    /// Wcale.
    Never,
}

pub(super) type Key = (String, usize);

struct Entry {
    key: Key,
    value: Extracted,
    until: Option<Instant>,
}

static CACHE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
pub(super) static RUNNING: AtomicUsize = AtomicUsize::new(0);

pub(super) fn cache_get(key: &Key) -> Option<Extracted> {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    cache.retain(|e| e.until.is_none_or(|t| t > now));
    let at = cache.iter().position(|e| &e.key == key)?;
    let entry = cache.remove(at);
    let value = entry.value.clone();
    cache.push(entry);
    Some(value)
}

pub(super) fn cache_put(key: Key, value: Extracted, keep: Keep) {
    let until = match keep {
        Keep::Always => None,
        Keep::Briefly => Some(Instant::now() + TIMEOUT_TTL),
        Keep::Never => return,
    };
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    cache.retain(|e| e.key != key);
    if cache.len() >= CACHE_ENTRIES {
        cache.remove(0);
    }
    cache.push(Entry { key, value, until });
}

/// Zmniejsza licznik wątków także po panice w ekstrakcji.
struct Running(&'static AtomicUsize);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Uruchamia `work` (z terminem `time`) w osobnym wątku (licznik `running`) i czeka na wynik
/// najwyżej `time` + `GRACE`.
pub(super) fn run(
    running: &'static AtomicUsize,
    time: Duration,
    work: impl FnOnce() -> Extracted + Send + 'static,
) -> (Extracted, Keep) {
    if running.fetch_add(1, Ordering::SeqCst) >= MAX_RUNNING {
        running.fetch_sub(1, Ordering::SeqCst);
        return (Extracted::Note(BUSY.into()), Keep::Never);
    }
    let guard = Running(running);
    let (tx, rx) = mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("alfa-pdf".into())
        .stack_size(STACK)
        .spawn(move || {
            let _guard = guard;
            let _ = tx.send(work());
        });
    if spawned.is_err() {
        return (Extracted::Note(BUSY.into()), Keep::Never);
    }
    match rx.recv_timeout(time + GRACE) {
        Ok(out) => (out, Keep::Always),
        Err(mpsc::RecvTimeoutError::Timeout) => (
            Extracted::Note(format!(
                "Wyciąganie tekstu z PDF przekroczyło limit czasu ({} s) i zostało przerwane.",
                time.as_secs()
            )),
            Keep::Briefly,
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            (Extracted::Note(CRASHED.into()), Keep::Always)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_RUNNING: AtomicUsize = AtomicUsize::new(0);

    fn slow() -> Extracted {
        std::thread::sleep(GRACE * 5);
        Extracted::Note("wolno".into())
    }

    #[test]
    fn timeout_is_kept_briefly_and_busy_is_not_kept() {
        let (out, keep) = run(&TEST_RUNNING, Duration::ZERO, slow);
        assert_eq!(keep, Keep::Briefly);
        assert!(matches!(out, Extracted::Note(n) if n.contains("limit czasu")));
        let _ = run(&TEST_RUNNING, Duration::ZERO, slow);
        let _ = run(&TEST_RUNNING, Duration::ZERO, slow);
        let (out, keep) = run(&TEST_RUNNING, Duration::ZERO, slow);
        assert_eq!((out, keep), (Extracted::Note(BUSY.into()), Keep::Never));
        let quick = || Extracted::Note("szybko".into());
        let deadline = Instant::now() + Duration::from_secs(15);
        while TEST_RUNNING.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            run(&TEST_RUNNING, Duration::from_secs(5), quick),
            (Extracted::Note("szybko".into()), Keep::Always)
        );
        let crash = || -> Extracted { panic!("awaria w wątku") };
        assert_eq!(
            run(&TEST_RUNNING, Duration::from_secs(5), crash).1,
            Keep::Always
        );

        let key: Key = ("ttl-test".into(), 1);
        cache_put(key.clone(), Extracted::Note("x".into()), Keep::Never);
        assert_eq!(cache_get(&key), None);
        cache_put(key.clone(), Extracted::Note("x".into()), Keep::Briefly);
        assert_eq!(cache_get(&key), Some(Extracted::Note("x".into())));
    }
}
